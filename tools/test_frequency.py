"""Check permission gates, fixed snapshots and exact corpus matching."""

import gzip
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

import frequency
import prepare_lexicon as builder
from test_prepare_lexicon import fixture


class FrequencyTests(unittest.TestCase):
    def test_window_exact_surface_and_no_pos_tag_import(self):
        raw = "学习\t1979,100,4\t1980,20,2\t2019,30,3\t2020,80,4\n学习_VERB\t2019,999,9\n陌生词\t2019,888,8\n市\t2000,10,1\n"
        counts, stats = frequency.count_words(
            gzip.compress(raw.encode()), {"学习", "市"}, 1980, 2019
        )
        self.assertEqual(counts, {"学习": 50, "市": 10})
        self.assertEqual(
            stats, {"source_rows": 4, "observed_words": 2, "match_total": 60}
        )

    def test_invalid_observations_and_empty_matches_fail_closed(self):
        for raw in (
            "市\t2000,1\n",
            "市\t2000,-1,0\n",
            "市\t2000,1,2\n",
            "市\t2000,2,1\t2000,2,1\n",
            "市\t2000,2,1\n市\t2000,2,1\n",
            "陌生\t2000,2,1\n",
        ):
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                frequency.count_words(gzip.compress(raw.encode()), {"市"}, 1980, 2019)
        with self.assertRaises(ValueError):
            frequency.count_words(b"", {"市"}, 2020, 2019)

    def test_permission_endpoint_size_and_checksum_are_all_required(self):
        config = json.loads(
            (frequency.ROOT / "data/frequency-source.json").read_text(encoding="utf-8")
        )
        content = gzip.compress("市\t2000,2,1\n".encode())
        config.update(
            source_bytes=len(content), source_sha256=hashlib.sha256(content).hexdigest()
        )
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "source.gz"
            path.write_bytes(content)
            self.assertEqual(frequency.verified_source(config, path), content)
            for field, wrong in (
                ("license", "unknown"),
                ("terms_url", "https://example.com"),
                ("snapshot_url", "https://example.com/data.gz"),
                ("source_bytes", 1),
                ("source_sha256", "0" * 64),
            ):
                with self.subTest(field=field), self.assertRaises(ValueError):
                    frequency.verified_source({**config, field: wrong}, path)
                self.assertEqual(path.read_bytes(), content)

    def test_frequency_scaling_reading_penalty_and_separate_independent_prior(self):
        source = fixture(
            "是 是 [shi4] /to be/",
            "市 市 [shi4] /city/",
            "石 石 [Shi2] /surname Shi/",
            "石 石 [shi2] /stone/",
            "罕 罕 [han3] /rare/",
        )
        files, _ = builder.convert(
            source, {"市": 10000, "罕": 7}, {"是": 1000, "市": 100, "石": 50}
        )
        table = files["dict.tsv"].decode()
        self.assertIn("是\tshi\t1000000\n", table)
        self.assertIn("市\tshi\t110000\n", table)
        self.assertIn("石\tshi\t50000\n", table)
        self.assertIn("罕\than\t17\n", table)
        records = [json.loads(line) for line in files["provenance.jsonl"].splitlines()]
        self.assertEqual(
            next(r for r in records if r["word"] == "市")["frequency_match_count"], 100
        )
        self.assertEqual(
            files,
            builder.convert(
                source, {"市": 10000, "罕": 7}, {"是": 1000, "市": 100, "石": 50}
            )[0],
        )
        names, _ = builder.convert(
            fixture("施 施 [Shi1] /surname Shi/"), {}, {"施": 50}
        )
        self.assertIn("施\tshi\t250000\n", names["dict.tsv"].decode())


if __name__ == "__main__":
    unittest.main()
