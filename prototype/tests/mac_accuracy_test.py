"""Mac-only learned segmentation, real receipts and restart; synthetic temporary data."""

import json
import os
import queue
import subprocess
import tempfile
import threading
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class AccuracyTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.path = Path(self.directory.name) / "words.json"
        self.process = None
        self.start()

    def tearDown(self):
        self.stop()
        self.directory.cleanup()

    def start(self, focused=True):
        args = [
            os.environ.get(
                "LINGOMATE_TEST_BRIDGE",
                str(ROOT / "upstream/qingjian/target/release/bilingual-ime-bridge"),
            ),
            os.environ.get("LINGOMATE_TEST_RESOURCES", str(ROOT)),
            "--memory",
            str(self.path),
        ]
        if focused:
            args.append("--mac-candidates")
        self.process = subprocess.Popen(
            args,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            encoding="utf-8",
        )
        self.responses = queue.Queue()

        def read():
            for line in self.process.stdout:
                self.responses.put(line)

        self.reader = threading.Thread(target=read, daemon=True)
        self.reader.start()

    def stop(self):
        if self.process:
            self.process.stdin.close()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)
            self.reader.join(timeout=5)
            self.process.stdout.close()
            self.process = None

    def request(self, input_="", action="query", **kw):
        self.process.stdin.write(
            json.dumps(dict(input=input_, action=action, context="accuracy", **kw))
            + "\n"
        )
        self.process.stdin.flush()
        result = json.loads(self.responses.get(timeout=10))
        self.assertNotIn("error", result)
        return result

    def train(self, word, input_):
        for _ in range(2):
            candidate = next(
                c for c in self.request(input_)["candidates"] if c["text"] == word
            )
            committed = self.request(
                input_, "commit", candidate=word, syllables=candidate["syllables"]
            )
            self.request(action="confirm", learning_token=committed["learningToken"])
        self.stop()
        data = json.loads(self.path.read_text(encoding="utf-8"))
        data["sequence"] += 300  # Synthetic subsequent confirmations; no user store.
        self.path.write_text(json.dumps(data), encoding="utf-8")
        self.start()

    def test_repeated_choice_survives_age_and_restart_but_legacy_is_unchanged(self):
        self.train("试验", "shiyan")
        self.assertEqual(self.request("shiyan")["candidates"][0]["text"], "试验")
        self.stop()
        self.start(focused=False)
        self.assertEqual(self.request("shiyan")["candidates"][0]["text"], "实验")

    def test_learned_sentence_is_committable_and_only_confirm_learns(self):
        self.train("试验", "shiyan")
        before = self.path.read_bytes()
        first = self.request("womenkaishishiyan")["candidates"][0]
        self.assertEqual(first["text"], "我们开始试验")
        self.request(action="cancel")
        self.assertEqual(self.path.read_bytes(), before)
        result = self.request(
            "womenkaishishiyan",
            "commit",
            candidate=first["text"],
            syllables=first["syllables"],
        )
        self.assertEqual(result["committed"], first["text"])
        self.assertEqual(result["input"], "")
        self.assertEqual(self.path.read_bytes(), before)
        self.request(action="confirm", learning_token=result["learningToken"])
        self.stop()
        self.start()
        self.assertEqual(
            self.request("womenkaishishiyan")["candidates"][0]["text"], first["text"]
        )

    def test_apostrophe_partial_consumption_and_single_accident(self):
        self.train("试验", "shiyan")
        words = self.request("women'kaishi'shiyan")["candidates"]
        self.assertEqual(words[0]["text"], "我们开始试验")
        frame = self.request("kaifazhe")
        word = next(c for c in frame["candidates"] if c["text"] == "开发")
        result = self.request(
            "kaifazhe", "commit", candidate=word["text"], syllables=word["syllables"]
        )
        self.assertEqual(result["input"], "zhe")
        self.request(action="cancel")
        self.stop()
        data = json.loads(self.path.read_text(encoding="utf-8"))
        for entry in data["entries"]:
            entry["count"] = 1
        self.path.write_text(json.dumps(data), encoding="utf-8")
        self.start()
        self.assertEqual(self.request("shiyan")["candidates"][0]["text"], "实验")
        self.assertEqual(
            self.request("womenkaishishiyan")["candidates"][0]["text"], "我们开始实验"
        )


if __name__ == "__main__":
    unittest.main()
