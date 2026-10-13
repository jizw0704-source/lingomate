"""Real opt-in bridge completions and receipt-confirmed temporary memory."""

import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BRIDGE = os.environ.get(
    "LINGOMATE_TEST_BRIDGE",
    str(ROOT / "upstream/qingjian/target/release/bilingual-ime-bridge"),
)
RESOURCES = os.environ.get("LINGOMATE_TEST_RESOURCES", str(ROOT))
PHRASE = "吃葡萄不吐葡萄皮"
READING = ["chi", "pu", "tao", "bu", "tu", "pu", "tao", "pi"]


class ExpressionTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.memory = Path(self.directory.name) / "words.json"
        self.process = None
        self.start()

    def tearDown(self):
        self.stop()
        self.directory.cleanup()

    def start(self, enabled=True):
        args = [BRIDGE, RESOURCES, "--memory", str(self.memory)]
        if enabled:
            args.append("--fixed-expressions")
        self.process = subprocess.Popen(
            args,
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

    def request(self, input_="", action="query", **kwargs):
        self.process.stdin.write(
            json.dumps(
                {
                    "input": input_,
                    "action": action,
                    "context": "expression-test",
                    **kwargs,
                }
            )
            + "\n"
        )
        self.process.stdin.flush()
        line = self.process.stdout.readline()
        self.assertTrue(line, "bridge exited without response")
        return json.loads(line)

    def commit(self, input_, word):
        selected = next(
            c for c in self.request(input_)["candidates"] if c["text"] == word
        )
        return self.request(
            input_, "commit", candidate=word, syllables=selected["syllables"]
        )

    def confirm(self, frame):
        return self.request(action="confirm", learning_token=frame["learningToken"])

    def test_normal_first_two_preserved_and_opt_in_only(self):
        self.stop()
        self.start(enabled=False)
        ordinary = self.request("chiputao")["candidates"]
        self.assertNotIn(PHRASE, [c["text"] for c in ordinary])
        self.stop()
        self.start()
        predicted = self.request("chiputao")["candidates"]
        self.assertEqual(predicted[:2], ordinary[:2])
        self.assertEqual(predicted[2]["text"], PHRASE)
        self.assertEqual(predicted[2]["syllables"], READING)
        self.assertLessEqual(len(predicted), 64)
        for input_ in ["chi", "chiput", "chiputaoxuexi", "ch'iputao", "cpt"]:
            self.assertNotIn(
                PHRASE, [c["text"] for c in self.request(input_)["candidates"]]
            )
        self.assertIn(
            PHRASE, [c["text"] for c in self.request("chi'pu'tao")["candidates"]]
        )

    def test_whole_output_no_learning_until_confirmation_restart_and_full_reading(self):
        frame = self.commit("chiputao", PHRASE)
        self.assertEqual(frame["committed"], PHRASE)
        self.assertEqual(frame["input"], "")
        self.assertFalse(self.memory.exists())
        self.confirm(frame)
        entries = json.loads(self.memory.read_text())["entries"]
        self.assertTrue(
            any(e["pinyin"] == "chiputao" and e["text"] == PHRASE for e in entries)
        )
        self.assertTrue(
            any(
                e["pinyin"] == "".join(READING) and e["text"] == PHRASE for e in entries
            )
        )
        self.stop()
        self.start()
        learned = next(
            c for c in self.request("chiputao")["candidates"] if c["text"] == PHRASE
        )
        self.assertTrue(learned["personal"])
        complete = self.commit("".join(READING), PHRASE)
        self.assertEqual(complete["committed"], PHRASE)
        self.assertEqual(complete["input"], "")

    def test_cancel_replay_and_forged_candidate_syllables_translation_rejected(self):
        frame = self.commit("chiputao", PHRASE)
        self.request(action="cancel")
        self.assertIn("error", self.confirm(frame))
        self.assertFalse(self.memory.exists())
        for input_, text, syllables, english in [
            ("chiputao", "随便生成的内容", READING, None),
            ("chiputao", PHRASE, READING[:3], None),
            ("chiputaoxuexi", PHRASE, READING, None),
            ("chiputao", PHRASE, READING, "unverified English"),
        ]:
            answer = self.request(
                input_, "commit", candidate=text, syllables=syllables, english=english
            )
            self.assertIn("error", answer)
            self.assertFalse(self.memory.exists())
        frame = self.commit("chiputao", PHRASE)
        self.confirm(frame)
        before = self.memory.read_bytes()
        self.assertIn("error", self.confirm(frame))
        self.assertEqual(self.memory.read_bytes(), before)

    def test_remembered_completion_promoted_only_within_completion_slots(self):
        candidates = self.request("yishi")["candidates"]
        catalog = json.loads(
            (ROOT / "data/generated/fixed-expressions.json").read_text()
        )["entries"]
        known = {
            (e["text"], tuple(e["syllables"]))
            for e in catalog
            if e["syllables"][:2] == ["yi", "shi"]
        }
        predicted = [
            c
            for c in candidates
            if (c["text"], tuple(c["syllables"])) in known
            and "".join(c["syllables"]) != "yishi"
        ]
        self.assertEqual(len(predicted), 3)
        chosen = predicted[-1]
        frame = self.commit("yishi", chosen["text"])
        self.confirm(frame)
        after = self.request("yishi")["candidates"]
        self.assertEqual(after[:2], candidates[:2])
        self.assertEqual(after[2]["text"], chosen["text"])

    def test_normal_partial_candidate_keeps_remaining_pinyin(self):
        frame = self.commit("kaifazhe", "开发")
        self.assertEqual(frame["committed"], "开发")
        self.assertEqual(frame["input"], "zhe")


if __name__ == "__main__":
    unittest.main()
