"""Expression extraction and permission gates; temporary fixtures only."""

import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import prepare_expressions as builder

FIXTURE = (
    f"#! license={builder.LICENSE_URL}\n"
    "百聞不如一見 百闻不如一见 [bai3 wen2 bu4 ru2 yi1 jian4] /seeing is believing (idiom)/\n"
    "學習 学习 [xue2 xi2] /to study/\n"
    "畫蛇添足 画蛇添足 [hua4 she2 tian1 zu2] /unnecessary addition (idiom)/\n"
).encode()
DICTIONARY = "百闻不如一见\tbai wen bu ru yi jian\t20\n学习\txue xi\t10\n".encode()


class ExpressionDataTests(unittest.TestCase):
    def setUp(self):
        self.config = json.loads(
            (builder.ROOT / "data/expressions-source.json").read_text(encoding="utf-8")
        )

    def test_idiom_only_matching_licensed_dictionary_and_supplement(self):
        catalog = json.loads(
            builder.compile_catalog(
                FIXTURE, DICTIONARY, "吃葡萄不吐葡萄皮".encode(), self.config
            )
        )
        self.assertEqual(catalog["license"], builder.LICENSE)
        entries = {e["text"]: e for e in catalog["entries"]}
        self.assertEqual(set(entries), {"百闻不如一见", "吃葡萄不吐葡萄皮"})
        self.assertEqual(entries["百闻不如一见"]["weight"], 20)
        self.assertEqual(entries["吃葡萄不吐葡萄皮"]["weight"], 1)

    def test_license_header_reading_and_unverified_supplement_rejected(self):
        for field, value in [
            ("license", "unknown"),
            ("license_url", "https://invalid.invalid"),
        ]:
            config = copy.deepcopy(self.config)
            config[field] = value
            with self.assertRaises(ValueError):
                builder.compile_catalog(
                    FIXTURE, DICTIONARY, "吃葡萄不吐葡萄皮".encode(), config
                )
        with self.assertRaises(ValueError):
            builder.compile_catalog(b"no license", DICTIONARY, b"", self.config)
        with self.assertRaises(ValueError):
            builder.compile_catalog(
                FIXTURE, DICTIONARY, b"not the requested phrase", self.config
            )
        for text, syllables in [
            ("中文", ["zhong", "wen"]),
            ("中英混合", ["zhong"]),
            ("中文ASCII", ["a"] * 7),
        ]:
            self.assertFalse(builder.valid(text, syllables))

    def test_snapshot_endpoint_size_checksum_and_cached_file_preserved(self):
        raw = "吃葡萄不吐葡萄皮".encode()
        config = copy.deepcopy(self.config["supplement"])
        config.update(
            source_bytes=len(raw), source_sha256=hashlib.sha256(raw).hexdigest()
        )
        with (
            tempfile.TemporaryDirectory() as directory,
            patch.object(builder, "ROOT", Path(directory)),
        ):
            path = (
                Path(directory) / "data/sources/wikipedia-tongue-twisters-94677104.txt"
            )
            path.parent.mkdir(parents=True)
            path.write_bytes(raw)
            self.assertEqual(builder.verified_supplement(config), raw)
            for name, value in [
                ("license", "unknown"),
                ("snapshot_url", "https://invalid.invalid"),
                ("source_bytes", 1),
                ("source_sha256", "0" * 64),
            ]:
                invalid = copy.deepcopy(config)
                invalid[name] = value
                with self.assertRaises(ValueError):
                    builder.verified_supplement(invalid)
                self.assertEqual(path.read_bytes(), raw)


if __name__ == "__main__":
    unittest.main()
