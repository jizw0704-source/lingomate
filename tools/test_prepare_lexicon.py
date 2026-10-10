"""Check independent provenance, Pinyin conversion and fail-closed inputs."""

import json
import tempfile
import unittest
from pathlib import Path

import prepare_lexicon as builder


def fixture(*lines):
    header = f"# CC-CEDICT\n#! license={builder.LICENSE_URL}\n#! entries={len(lines)}\n"
    return (header + "\n".join(lines) + "\n").encode()


class LexiconTests(unittest.TestCase):
    def test_readings_glosses_and_line_provenance(self):
        source = fixture(
            "綠 绿 [lu:4] /green/CL:個|个[ge4]/",
            "銀行 银行 [yin2 hang2] /bank/",
            "銀行 银行 [yin2 hang2] /bank/financial institution/",
            "花兒 花儿 [hua1 r5] /flower/",
            "行 行 [Xing2] /surname Xing/",
            "行 行 [xing2] /to walk/",
            "A股 A股 [A gu3] /A shares/",
            "罕 罕 [xx5] /unknown/",
        )
        files, stats = builder.convert(source, {"银行": 12345})
        self.assertIn("绿\tlv\t10\n", files["dict.tsv"].decode())
        self.assertIn("花儿\thua er\t10\n", files["dict.tsv"].decode())
        self.assertIn("银行\tyin hang\t12355\n", files["dict.tsv"].decode())
        self.assertIn("行\txing\t10\n", files["dict.tsv"].decode())
        self.assertIn(
            "银行\tbank\tfinancial institution\n", files["glossary-en.tsv"].decode()
        )
        self.assertNotIn("CL:", files["glossary-en.tsv"].decode())
        self.assertIn("行\tto walk\tsurname Xing\n", files["glossary-en.tsv"].decode())
        self.assertNotIn("A股", files["dict.tsv"].decode())
        self.assertEqual(stats["filtered_source_entries"], 2)
        record = next(
            json.loads(line)
            for line in files["provenance.jsonl"].splitlines()
            if json.loads(line)["word"] == "银行"
        )
        self.assertEqual(record["source_lines"], [5, 6])

    def test_no_source_fallback_for_unlicensed_or_malformed_data(self):
        good = fixture("學習 学习 [xue2 xi2] /to learn; to study/")
        for source in (
            good.replace(b"by-sa/4.0", b"by-sa/3.0"),
            good.replace(b"entries=1", b"entries=2"),
            good + b"malformed entry\n",
        ):
            with self.subTest(source=source), self.assertRaises(ValueError):
                builder.convert(source, {})
        with self.assertRaises(ValueError):
            builder.convert(good, {"不存在": 100})

    def test_pinned_checksum_rejects_wrong_source_without_overwriting_it(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "source.u8"
            source.write_bytes(b"wrong snapshot")
            with self.assertRaises(ValueError):
                builder.verified_source({"source_sha256": "0" * 64}, source)
            self.assertEqual(source.read_bytes(), b"wrong snapshot")

    def test_packaged_old_tables_and_missing_notices_are_rejected(self):
        files = {
            name: name.encode()
            for name in (
                "dict.tsv",
                "glossary-en.tsv",
                "LEXICON-NOTICE.md",
                "GLOSSARY-NOTICE.md",
            )
        }
        with tempfile.TemporaryDirectory() as directory:
            package = Path(directory)
            for name, content in files.items():
                (package / name).write_bytes(content)
            builder.verify_package(package, files)
            (package / "dict.tsv").write_bytes(b"legacy data")
            with self.assertRaises(ValueError):
                builder.verify_package(package, files)
            (package / "dict.tsv").write_bytes(files["dict.tsv"])
            (package / "GLOSSARY-NOTICE.md").unlink()
            with self.assertRaises(ValueError):
                builder.verify_package(package, files)

    def test_deterministic_lf_outputs_without_optional_corpus(self):
        source = fixture("學習 学习 [xue2 xi2] /to learn; to study/")
        a, _ = builder.convert(source, {})
        b, _ = builder.convert(source.replace(b"\n", b"\r\n"), {})
        self.assertEqual(a, b)
        self.assertIn("学习\txue xi\t10\n", a["dict.tsv"].decode())
        self.assertNotIn(b"\r", a["dict.tsv"])


if __name__ == "__main__":
    unittest.main()
