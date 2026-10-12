"""真实桥接进程及重启测试；每项使用隔离临时词库，不读个人词库。"""

import json
import os
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BRIDGE = Path(
    os.environ.get(
        "LINGOMATE_TEST_BRIDGE",
        str(ROOT / "upstream/qingjian/target/release/bilingual-ime-bridge"),
    )
)
RESOURCES = os.environ.get("LINGOMATE_TEST_RESOURCES", str(ROOT))


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
            [str(BRIDGE), RESOURCES, "--memory", str(self.path)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            encoding="utf-8",
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
        return json.loads(self.path.read_text(encoding="utf-8"))["entries"]

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
        if os.name != "nt":
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
        self.assertNotIn("city", self.path.read_text(encoding="utf-8"))

    def test_abbreviation_and_apostrophe_share_canonical_word(self):
        self.confirm(self.commit("s"))
        self.restart()
        for key in ("s", "shi"):
            self.assertEqual(self.request(key)["candidates"][0]["text"], "市")
        self.confirm(self.commit("xue'xi", "学习"))
        self.restart()
        for key in ("xue'xi", "xuexi"):
            self.assertTrue(self.request(key)["candidates"][0]["personal"])

    def test_stale_preference_returns_to_corpus_order_without_deleting_word(self):
        self.confirm(self.commit())
        self.stop()
        data = json.loads(self.path.read_text(encoding="utf-8"))
        data["sequence"] += 256
        self.path.write_text(json.dumps(data), encoding="utf-8")
        self.start()
        candidates = self.request("shi")["candidates"]
        self.assertEqual(candidates[0]["text"], "是")
        self.assertTrue(next(c for c in candidates if c["text"] == "市")["personal"])
        self.assertEqual(self.entries(), data["entries"])

    def test_personal_partial_word_does_not_displace_complete_word(self):
        self.confirm(self.commit())
        first = self.request("shiqing")["candidates"][0]
        self.assertEqual(first["text"], "事情")
        self.assertEqual(first["syllables"], ["shi", "qing"])

    def test_count_bonus_is_capped(self):
        self.confirm(self.commit())
        self.stop()
        data = json.loads(self.path.read_text(encoding="utf-8"))
        data["sequence"] = 2
        data["entries"][0].update(count=4000000000, last=1)
        data["entries"].append(
            {
                "pinyin": "shi",
                "text": "是",
                "syllables": ["shi"],
                "count": 20,
                "last": 2,
            }
        )
        self.path.write_text(json.dumps(data), encoding="utf-8")
        self.start()
        self.assertEqual(self.request("shi")["candidates"][0]["text"], "是")

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

    def test_separate_compositions_learn_known_and_unknown_words_after_restart(self):
        for pinyin, character in [("gai", "钙"), ("tai", "钛"), ("kuang", "矿")]:
            self.confirm(self.commit(pinyin, character))
        self.restart()
        first = self.request("gaitaikuang")["candidates"][0]
        self.assertEqual(first["text"], "钙钛矿")
        self.assertTrue(first["personal"])
        self.request(action="cancel")
        for pinyin, character in [("gai", "钙"), ("tai", "钛"), ("nie", "镍")]:
            self.confirm(self.commit(pinyin, character))
        self.restart()
        first = self.request("gaitainie")["candidates"][0]
        self.assertEqual(first["text"], "钙钛镍")
        self.assertTrue(first["personal"])
        self.assertEqual(self.commit("gaitainie", "钙钛镍")["committed"], "钙钛镍")

    def test_separate_characters_do_not_cross_cancel_context_or_english(self):
        for boundary in ("cancel", "context", "english"):
            self.request(action="cancel")
            self.confirm(self.commit("nie", "镍"))
            if boundary == "cancel":
                self.request(action="cancel")
            elif boundary == "english":
                self.confirm(self.commit("shi", "市", english="city"))
            context = "another" if boundary == "context" else "editor"
            part = self.commit("gai", "钙", context=context)
            self.confirm(part, context=context)
            self.assertFalse(any(e["text"] == "镍钙" for e in self.entries()))

    def test_function_word_sentence_is_not_joined_into_personal_entry(self):
        for pinyin, character in [
            ("wo", "我"),
            ("xiang", "想"),
            ("qu", "去"),
            ("kan", "看"),
            ("ni", "你"),
        ]:
            self.confirm(self.commit(pinyin, character))
        self.assertFalse(any(e["text"] == "我想去看你" for e in self.entries()))
        self.assertFalse(
            any(e["text"] in ("我想", "想去", "去看", "看你") for e in self.entries())
        )

    def test_deferred_host_receipts_survive_later_queries_and_remain_fifo_single_use(
        self,
    ):
        first = self.commit("gai", "钙", deferred_output=True)
        second = self.commit("tai", "钛", deferred_output=True)
        self.assertFalse(self.path.exists())
        self.assertIn("error", self.confirm(second))
        self.confirm(first)
        self.assertIn("error", self.confirm(first))
        self.confirm(second)
        self.assertTrue(any(e["text"] == "钙钛" for e in self.entries()))
        pending = self.commit("nie", "镍", deferred_output=True)
        self.request(action="cancel")
        self.assertIn("error", self.confirm(pending))
        self.assertFalse(any(e["text"] == "钙钛镍" for e in self.entries()))

    def test_concurrent_processes_merge_choices_and_reload_other_apps(self):
        from concurrent.futures import ThreadPoolExecutor

        other = subprocess.Popen(
            [str(BRIDGE), RESOURCES, "--memory", str(self.path)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            encoding="utf-8",
        )

        def exchange(process, request):
            process.stdin.write(json.dumps(request) + "\n")
            process.stdin.flush()
            result = json.loads(process.stdout.readline())
            self.assertNotIn("error", result)
            return result

        def select_many(process, text, word):
            for _ in range(20):
                query = exchange(
                    process, {"action": "query", "input": text, "context": "app"}
                )
                candidate = next(c for c in query["candidates"] if c["text"] == word)
                commit = exchange(
                    process,
                    {
                        "action": "commit",
                        "input": text,
                        "context": "app",
                        "candidate": word,
                        "syllables": candidate["syllables"],
                    },
                )
                exchange(
                    process,
                    {
                        "action": "confirm",
                        "input": "",
                        "context": "app",
                        "learning_token": commit["learningToken"],
                    },
                )

        try:
            with ThreadPoolExecutor(max_workers=2) as pool:
                first = pool.submit(select_many, self.process, "shi", "市")
                second = pool.submit(select_many, other, "qing", "青")
                first.result(timeout=20)
                second.result(timeout=20)
            entries = self.entries()
            self.assertEqual(
                next(
                    e["count"]
                    for e in entries
                    if e["text"] == "市" and e["pinyin"] == "shi"
                ),
                20,
            )
            self.assertEqual(
                next(
                    e["count"]
                    for e in entries
                    if e["text"] == "青" and e["pinyin"] == "qing"
                ),
                20,
            )
            self.assertTrue(
                next(
                    c for c in self.request("qing")["candidates"] if c["text"] == "青"
                )["personal"]
            )
        finally:
            other.stdin.close()
            other.wait(timeout=5)
            other.stdout.close()

    def test_stale_custom_full_word_stays_a_word_instead_of_a_synthetic_tail(self):
        for pinyin, character in [("gai", "钙"), ("tai", "钛"), ("nie", "镍")]:
            self.confirm(self.commit(pinyin, character))
        self.stop()
        data = json.loads(self.path.read_text())
        data["sequence"] += 1000
        self.path.write_text(json.dumps(data), encoding="utf-8")
        self.start()
        first = self.request("gaitainie")["candidates"][0]
        self.assertEqual(first["text"], "钙钛镍")
        self.assertTrue(first["personal"])

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
