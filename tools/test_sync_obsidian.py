"""Check mirroring and protection against overwriting handwritten notes."""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("sync_obsidian.py")


class SyncTests(unittest.TestCase):
    def run_sync(self, vault, *args):
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--vault", str(vault), *args],
            capture_output=True,
            text=True,
            check=False,
        )

    def test_sync_is_idempotent_and_check_matches_source(self):
        with tempfile.TemporaryDirectory() as directory:
            vault = Path(directory)
            (vault / ".obsidian").mkdir()
            self.assertEqual(self.run_sync(vault).returncode, 0)
            notes = vault / "10 项目/中英输入法"
            before = {p.name: p.read_bytes() for p in notes.glob("*.md")}
            self.assertEqual(len(before), 7)
            self.assertEqual(self.run_sync(vault).returncode, 0)
            self.assertEqual(
                before, {p.name: p.read_bytes() for p in notes.glob("*.md")}
            )
            self.assertEqual(self.run_sync(vault, "--check").returncode, 0)

    def test_manual_edits_stop_all_writes(self):
        with tempfile.TemporaryDirectory() as directory:
            vault = Path(directory)
            (vault / ".obsidian").mkdir()
            self.assertEqual(self.run_sync(vault).returncode, 0)
            notes = vault / "10 项目/中英输入法"
            overview = notes / "项目总览.md"
            overview.write_text(overview.read_text() + "\n手写记录：请保留。\n")
            before = {p.name: p.read_bytes() for p in notes.iterdir()}
            self.assertNotEqual(self.run_sync(vault).returncode, 0)
            self.assertEqual(before, {p.name: p.read_bytes() for p in notes.iterdir()})

    def test_unowned_notes_and_nonvault_are_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            vault = Path(directory)
            self.assertNotEqual(self.run_sync(vault).returncode, 0)
            (vault / ".obsidian").mkdir()
            notes = vault / "10 项目/中英输入法"
            notes.mkdir(parents=True)
            guide = notes / "使用指南.md"
            guide.write_text("原有手写指南")
            self.assertNotEqual(self.run_sync(vault).returncode, 0)
            self.assertEqual(guide.read_text(), "原有手写指南")
            self.assertFalse((notes / "项目总览.md").exists())


if __name__ == "__main__":
    unittest.main()
