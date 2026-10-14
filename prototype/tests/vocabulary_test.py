"""Explicit management against real running bridge; temporary stores only."""

import json
import os
import subprocess
import tempfile
import unittest
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BRIDGE = Path(
    os.environ.get(
        "LINGOMATE_TEST_BRIDGE",
        ROOT / "upstream/qingjian/target/release/bilingual-ime-bridge",
    )
)
RESOURCES = os.environ.get("LINGOMATE_TEST_RESOURCES", str(ROOT))


class VocabularyTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.file = Path(self.directory.name) / "words.json"
        self.preferences = self.file.with_suffix(".preferences.json")
        self.original = {
            "version": 1,
            "sequence": 1000,
            "entries": [
                {
                    "pinyin": "shi",
                    "text": "市",
                    "syllables": ["shi"],
                    "count": 2,
                    "last": 1,
                },
                {
                    "pinyin": "s",
                    "text": "市",
                    "syllables": ["shi"],
                    "count": 2,
                    "last": 1,
                },
                {
                    "pinyin": "shi",
                    "text": "是",
                    "syllables": ["shi"],
                    "count": 5,
                    "last": 2,
                },
                {
                    "pinyin": "gaitainie",
                    "text": "钙钛镍",
                    "syllables": ["gai", "tai", "nie"],
                    "count": 1,
                    "last": 3,
                },
                {
                    "pinyin": "chang",
                    "text": "长",
                    "syllables": ["chang"],
                    "count": 1,
                    "last": 4,
                },
                {
                    "pinyin": "zhang",
                    "text": "长",
                    "syllables": ["zhang"],
                    "count": 1,
                    "last": 5,
                },
            ],
        }
        self.file.write_text(json.dumps(self.original), encoding="utf-8")
        self.process = subprocess.Popen(
            [
                str(BRIDGE),
                RESOURCES,
                "--memory",
                str(self.file),
                "--mac-candidates",
                "--fixed-expressions",
            ],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )

    def tearDown(self):
        self.process.stdin.close()
        self.process.wait(timeout=5)
        self.process.stdout.close()
        self.directory.cleanup()

    def manage(self, action="list", **fields):
        result = subprocess.run(
            [str(BRIDGE), "--vocabulary", str(self.file)],
            input=json.dumps({"action": action, **fields}),
            capture_output=True,
            text=True,
            timeout=5,
            check=True,
        )
        self.assertEqual(result.stderr, "")
        return json.loads(result.stdout)

    def query(self, input_, action="query", **fields):
        self.process.stdin.write(
            json.dumps(
                {"action": action, "input": input_, "context": "isolated", **fields}
            )
            + "\n"
        )
        self.process.stdin.flush()
        result = json.loads(self.process.stdout.readline())
        self.assertNotIn("error", result)
        return result

    def test_aliases_group_by_exact_word_and_reading_without_inflating_usage(self):
        before = self.file.read_bytes()
        words = self.manage()["words"]
        self.assertEqual(len(words), 5)
        word = next(w for w in words if w["text"] == "市")
        self.assertEqual(set(word["aliases"]), {"shi", "s"})
        self.assertEqual(word["count"], 2)
        self.assertEqual(len([w for w in words if w["text"] == "长"]), 2)
        self.assertEqual(self.file.read_bytes(), before)
        self.assertFalse(self.preferences.exists())

    def test_priority_persists_and_reloads_without_engine_restart_or_count_changes(
        self,
    ):
        before = self.file.read_bytes()
        self.assertEqual(self.query("shi")["candidates"][0]["text"], "是")
        words = self.manage(
            "prioritize", text="市", syllables=["shi"], prioritized=True
        )["words"]
        self.assertTrue(next(w for w in words if w["text"] == "市")["prioritized"])
        self.assertEqual(self.file.read_bytes(), before)
        for spelling in ["shi", "s"]:
            matches = [
                c
                for c in self.query(spelling)["candidates"]
                if c["syllables"] == ["shi"]
            ]
            self.assertEqual(matches[0]["text"], "市")
        full = self.query("shiqing")["candidates"][0]
        self.assertEqual(full["text"], "事情")
        self.manage("prioritize", text="是", syllables=["shi"], prioritized=True)
        words = self.manage()["words"]
        self.assertFalse(next(w for w in words if w["text"] == "市")["prioritized"])
        self.assertEqual(self.query("shi")["candidates"][0]["text"], "是")
        self.manage("prioritize", text="是", syllables=["shi"], prioritized=False)
        self.assertFalse(any(w["prioritized"] for w in self.manage()["words"]))
        self.assertEqual(self.file.read_bytes(), before)

    def test_delete_removes_all_aliases_and_priority_next_query(self):
        self.manage("prioritize", text="市", syllables=["shi"], prioritized=True)
        self.query("shi")
        self.manage("delete", text="市", syllables=["shi"])
        self.assertFalse(
            any(e["text"] == "市" for e in json.loads(self.file.read_text())["entries"])
        )
        for key in ("s", "shi"):
            self.assertFalse(
                any(
                    c["personal"]
                    for c in self.query(key)["candidates"]
                    if c["text"] == "市"
                )
            )
        self.assertFalse(
            any(
                p["text"] == "市"
                for p in json.loads(self.preferences.read_text())["entries"]
            )
        )
        # A new successful confirmed selection may remember it again, without old priority.
        frame = self.query("shi", "commit", candidate="市", syllables=["shi"])
        self.query("", "confirm", learning_token=frame["learningToken"])
        self.assertFalse(
            next(w for w in self.manage()["words"] if w["text"] == "市")["prioritized"]
        )

    def test_delete_preserves_other_readings_and_unknown_word_disappears_from_memory(
        self,
    ):
        self.manage("delete", text="长", syllables=["chang"])
        self.assertEqual(
            [w["syllables"] for w in self.manage()["words"] if w["text"] == "长"],
            [["zhang"]],
        )
        self.assertTrue(
            any(
                c["personal"] and c["text"] == "钙钛镍"
                for c in self.query("gaitainie")["candidates"]
            )
        )
        self.manage("delete", text="钙钛镍", syllables=["gai", "tai", "nie"])
        self.assertFalse(
            any(
                c["personal"] and c["text"] == "钙钛镍"
                for c in self.query("gaitainie")["candidates"]
            )
        )

    def test_stale_invalid_and_corrupt_operations_do_not_overwrite_original(self):
        original = self.file.read_bytes()
        for request in [
            {"action": "delete", "text": "市", "syllables": ["si"]},
            {"action": "delete", "text": "../市", "syllables": ["shi"]},
            {"action": "delete", "text": "市", "syllables": ["shi"], "input": "shi"},
            {"action": "prioritize", "text": "市", "syllables": ["shi"]},
            {"action": "list", "text": "市"},
            {"action": "clear"},
        ]:
            self.assertIn("error", self.manage(**request))
            self.assertEqual(self.file.read_bytes(), original)
            self.assertFalse(self.preferences.exists())
        for bad in [b"{broken", b'{"version":2,"sequence":0,"entries":[]}']:
            self.file.write_bytes(bad)
            self.assertIn("error", self.manage())
            self.assertIn("error", self.manage("delete", text="市", syllables=["shi"]))
            self.assertEqual(self.file.read_bytes(), bad)
        self.file.write_bytes(original)
        self.preferences.write_bytes(b"{bad-priority")
        self.assertIn(
            "error",
            self.manage("prioritize", text="市", syllables=["shi"], prioritized=True),
        )
        self.assertEqual(self.file.read_bytes(), original)
        self.assertEqual(self.preferences.read_bytes(), b"{bad-priority")
        self.assertTrue(self.query("xuexi")["candidates"])

    def test_concurrent_managers_merge_and_input_still_records_unrelated_word(self):
        with ThreadPoolExecutor(max_workers=3) as pool:
            actions = [
                pool.submit(self.manage, "delete", text="长", syllables=[reading])
                for reading in ["chang", "zhang"]
            ]
            actions.append(
                pool.submit(
                    self.manage,
                    "prioritize",
                    text="市",
                    syllables=["shi"],
                    prioritized=True,
                )
            )
            for result in actions:
                self.assertNotIn("error", result.result(timeout=5))
        frame = self.query("xuexi", "commit", candidate="学习", syllables=["xue", "xi"])
        self.query("", "confirm", learning_token=frame["learningToken"])
        words = self.manage()["words"]
        self.assertFalse(any(w["text"] == "长" for w in words))
        self.assertTrue(any(w["text"] == "学习" for w in words))
        self.assertTrue(next(w for w in words if w["text"] == "市")["prioritized"])
        self.assertEqual(
            set(json.loads(self.file.read_text())), {"version", "sequence", "entries"}
        )

    def test_missing_and_symlink_stores_are_not_rebuilt(self):
        self.file.unlink()
        self.assertEqual(self.manage()["words"], [])
        self.assertFalse(self.file.exists())
        target = Path(self.directory.name) / "protected.json"
        target.write_text(json.dumps(self.original))
        self.file.symlink_to(target)
        before = target.read_bytes()
        self.assertIn("error", self.manage("delete", text="市", syllables=["shi"]))
        self.assertEqual(target.read_bytes(), before)
        self.assertTrue(self.file.is_symlink())

    @unittest.skipIf(os.name == "nt", "POSIX flock fixture")
    def test_busy_lock_is_bounded_and_cannot_lose_choices(self):
        import fcntl

        before = self.file.read_bytes()
        with self.file.with_suffix(".lock").open("w") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            self.assertIn("error", self.manage("delete", text="市", syllables=["shi"]))
        self.assertEqual(self.file.read_bytes(), before)
        self.assertNotIn("error", self.manage())


if __name__ == "__main__":
    unittest.main()
