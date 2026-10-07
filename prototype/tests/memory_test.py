"""真实桥接进程及重启测试；每项使用隔离临时词库，不读个人词库。"""

import json
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BRIDGE = ROOT / "upstream/qingjian/target/release/bilingual-ime-bridge"


class MemoryTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.path = Path(self.directory.name) / "personal" / "words.json"
        self.process = None
        self.start()

    def tearDown(self):
        self.stop()
        self.directory.cleanup()

    def start(self):
        self.process = subprocess.Popen(
            [str(BRIDGE), str(ROOT), "--memory", str(self.path)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )

    def stop(self):
        if self.process:
            self.process.stdin.close()
            self.process.wait(timeout=5)
            self.process.stdout.close()
            self.process = None

    def restart(self):
        self.stop()
        self.start()

    def request(self, input_="", action="query", context="editor", **kwargs):
        self.process.stdin.write(
            json.dumps(
                {"input": input_, "action": action, "context": context, **kwargs}
            )
            + "\n"
        )
        self.process.stdin.flush()
        result = json.loads(self.process.stdout.readline())
        return result

    def commit(self, input_="shi", word="市", **kwargs):
        candidate = next(
            c for c in self.request(input_)["candidates"] if c["text"] == word
        )
        return self.request(
            input_, "commit", candidate=word, syllables=candidate["syllables"], **kwargs
        )

    def confirm(self, frame, **kwargs):
        self.assertIsNotNone(frame.get("learningToken"))
        return self.request(
            action="confirm", learning_token=frame["learningToken"], **kwargs
        )

    def entries(self):
        return json.loads(self.path.read_text())["entries"]

    def test_only_confirmed_selection_is_persisted_and_reordered_after_restart(self):
        baseline = self.request("shi")["candidates"]
        self.assertNotEqual(baseline[0]["text"], "市")
        frame = self.commit()
        self.assertEqual(frame["committed"], "市")
        self.assertFalse(self.path.exists())
        self.confirm(frame)
        self.restart()
        first = self.request("shi")["candidates"][0]
        self.assertEqual(first["text"], "市")
        self.assertTrue(first["personal"])
        self.assertEqual(stat.S_IMODE(self.path.stat().st_mode), 0o600)
        self.assertEqual(stat.S_IMODE(self.path.parent.stat().st_mode), 0o700)

    def test_query_cancel_unconfirmed_and_invalid_commit_do_not_learn(self):
        self.request("shi")
        self.request(action="cancel")
        bad = self.request("shi", "commit", candidate="不存在", syllables=["shi"])
        self.assertIn("error", bad)
        frame = self.commit()
        self.request(action="cancel")
        self.assertIn("error", self.confirm(frame))
        self.restart()
        self.assertFalse(self.path.exists())
        self.assertFalse(any(c["personal"] for c in self.request("shi")["candidates"]))

    def test_confirm_receipt_is_context_bound_single_use_and_invalidated_by_edit(self):
        frame = self.commit()
        self.assertIn("error", self.confirm(frame, context="other-editor"))
        self.confirm(frame)
        saved = self.path.read_bytes()
        self.assertIn("error", self.confirm(frame))
        self.assertEqual(saved, self.path.read_bytes())
        frame = self.commit()
        self.request("qing")
        self.assertIn("error", self.confirm(frame))
        self.assertEqual(saved, self.path.read_bytes())

    def test_frequency_then_recency_ranks_selected_words(self):
        self.confirm(self.commit())
        self.confirm(self.commit())
        self.confirm(self.commit(word="是"))
        self.assertEqual(self.request("shi")["candidates"][0]["text"], "市")
        self.confirm(self.commit(word="是"))
        self.restart()
        self.assertEqual(self.request("shi")["candidates"][0]["text"], "是")

    def test_new_word_from_one_composition_can_commit_after_restart(self):
        part = self.commit("shiqing")
        self.assertEqual(part["input"], "qing")
        self.confirm(part)
        self.confirm(self.commit("qing", "青"))
        self.restart()
        candidate = self.request("shiqing")["candidates"][0]
        self.assertEqual(candidate["text"], "市青")
        self.assertEqual(candidate["syllables"], ["shi", "qing"])
        self.assertTrue(candidate["personal"])
        result = self.commit("shiqing", "市青")
        self.assertEqual(result["committed"], "市青")
        self.assertEqual(result["input"], "")
        self.assertIn("error", self.commit("shiqing", "市青", english="untrusted"))

    def test_cancel_edit_or_context_change_prevents_phrase_join(self):
        for boundary in ("cancel", "edit", "context"):
            with self.subTest(boundary=boundary):
                self.confirm(self.commit("shiqing"))
                if boundary == "cancel":
                    self.request(action="cancel")
                elif boundary == "edit":
                    self.request("xuexi")
                context = "other" if boundary == "context" else "editor"
                part = self.commit("qing", "青", context=context)
                self.confirm(part, context=context)
                self.assertFalse(any(e["text"] == "市青" for e in self.entries()))
                self.request(action="cancel")

    def test_english_learns_underlying_word_but_never_an_english_phrase(self):
        self.confirm(self.commit("shiqing", english="city"))
        self.confirm(self.commit("qing", "青"))
        self.assertFalse(any(e["text"] == "市青" for e in self.entries()))
        self.assertTrue(any(e["text"] == "市" for e in self.entries()))
        # Native locally translated output consumes Chinese, then explicitly marks English.
        self.confirm(self.commit("shiqing"), chinese_output=False)
        self.confirm(self.commit("qing", "青"))
        self.assertFalse(any(e["text"] == "市青" for e in self.entries()))
        self.assertNotIn("city", self.path.read_text())

    def test_abbreviation_and_apostrophe_share_canonical_word(self):
        self.confirm(self.commit("s"))
        self.restart()
        for key in ("s", "shi"):
            self.assertEqual(self.request(key)["candidates"][0]["text"], "市")
        self.confirm(self.commit("xue'xi", "学习"))
        self.restart()
        for key in ("xue'xi", "xuexi"):
            self.assertTrue(self.request(key)["candidates"][0]["personal"])

    def test_corrupt_or_unknown_version_store_is_preserved_and_input_works(self):
        self.stop()
        self.path.parent.mkdir()
        for original in (b"broken", b'{"version":2,"sequence":0,"entries":[]}'):
            self.path.write_bytes(original)
            self.start()
            frame = self.commit()
            self.assertEqual(frame["committed"], "市")
            self.assertIsNone(frame["learningToken"])
            self.assertTrue(frame["memoryWarning"])
            self.assertEqual(self.path.read_bytes(), original)
            self.stop()

    def test_save_failure_does_not_fail_output(self):
        # Directory created after startup obstructs the target; no user data is touched.
        self.request("shi")
        self.path.parent.mkdir()
        self.path.mkdir()
        frame = self.commit()
        self.assertEqual(frame["committed"], "市")
        response = self.confirm(frame)
        self.assertTrue(response["memoryWarning"])
        self.assertTrue(self.path.is_dir())
        self.assertEqual(list(self.path.parent.glob(".memory-*.tmp")), [])


if __name__ == "__main__":
    unittest.main()
