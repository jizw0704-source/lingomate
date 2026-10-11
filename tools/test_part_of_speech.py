"""Verify conservative tagging, exact output preservation and source guards."""

import hashlib
import io
import tarfile
import tempfile
import unittest
from pathlib import Path

import part_of_speech as pos


class POSTests(unittest.TestCase):
    def test_contextual_infinitive_and_exact_lexical_categories(self):
        index = {
            "study": {"n.", "v."},
            "learn": {"v."},
            "research": {"n.", "v."},
            "beautiful": {"adj."},
            "quickly": {"adv."},
        }
        for text, expected in [
            ("study", "n./v."),
            ("to study", "v."),
            ("to learn; to study", "v."),
            ("study; research", "n./v."),
            ("beautiful", "adj."),
            ("quickly", "adv."),
            ("unknown", ""),
            ("to beautiful", ""),
            ("study; unknown", ""),
            ("a study", ""),
            ("beautiful; quickly", ""),
        ]:
            self.assertEqual(pos.label(text, index), expected, text)

    def test_comments_preserve_original_glossary_and_unmatched_output(self):
        glossary = b"# fixture\nword\tstudy\tunmatched phrase\n"
        result, stats = pos.annotate(
            glossary, {"study": {"n.", "v."}}, b"fixture license\n"
        )
        self.assertTrue(result.startswith(glossary))
        self.assertIn(b"# POS\tstudy\tn./v.\n", result)
        self.assertNotIn(b"# POS\tunmatched phrase", result)
        self.assertEqual(stats["tagged_senses"], 1)

    def test_source_permission_and_checksum_are_required(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory) / "archive.gz"
            file.write_bytes(b"bad snapshot")
            config = {
                "license": "WordNet",
                "snapshot_url": pos.URL,
                "source_bytes": 12,
                "source_sha256": "0" * 64,
            }
            with self.assertRaises(ValueError):
                pos.verified_source(config, file)
            self.assertEqual(file.read_bytes(), b"bad snapshot")
            config["source_sha256"] = hashlib.sha256(file.read_bytes()).hexdigest()
            config["license"] = "unknown"
            with self.assertRaises(ValueError):
                pos.verified_source(config, file)

    def test_fixed_members_license_and_phrase_normalization(self):
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode="w:gz") as archive:
            rows = {
                "LICENSE": "preserved permission",
                "dict/index.noun": "study n 1 0 1 0 00000001\n",
                "dict/index.verb": "study v 1 0 1 0 00000001\nlook_up v 1 0 1 0 00000002\n",
                "dict/index.adj": "beautiful a 1 0 1 0 00000001\n",
                "dict/index.adv": "quickly r 1 0 1 0 00000001\n",
            }
            for name, content in rows.items():
                data = content.encode()
                member = tarfile.TarInfo("WordNet-3.0/" + name)
                member.size = len(data)
                archive.addfile(member, io.BytesIO(data))
        index = pos.read_index(buffer.getvalue(), b"preserved permission")
        self.assertEqual(pos.label("to look up", index), "v.")
        self.assertEqual(pos.label("study", index), "n./v.")
        with self.assertRaises(ValueError):
            pos.read_index(buffer.getvalue(), b"wrong license")


if __name__ == "__main__":
    unittest.main()
