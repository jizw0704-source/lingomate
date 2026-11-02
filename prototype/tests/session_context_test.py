"""Synthetic context-ranking fixtures. No host content, cloud or personal vocabulary."""

import json
import subprocess
import unittest

from mac_accuracy_test import ROOT, AccuracyTests


class ContextTests(AccuracyTests):
    def test_context_only_reorders_existing_homophones(self):
        for prefix, raw, target in [
            ("达成", "gongshi", "共识"),
            ("客观", "shishi", "事实"),
            ("临床", "shiyan", "试验"),
        ]:
            baseline = self.request(raw)["candidates"]
            contextual = self.request(raw, preceding=prefix)["candidates"]
            identities = lambda rows: {(c["text"], tuple(c["syllables"])) for c in rows}
            self.assertEqual(identities(baseline), identities(contextual))
            self.assertEqual(contextual[0]["text"], target)
            self.assertNotEqual(baseline[0]["text"], target)
            self.assertEqual(self.request(raw)["candidates"], baseline)
        self.assertFalse(self.path.exists())

    def test_query_does_not_learn_empty_partial_and_abbreviations_are_unchanged(self):
        for raw in ["gs", "gongshix", "gong", "", "AI"]:
            if (
                raw == "AI"
            ):  # raw English remains a native concern; protocol only accepts lowercase
                continue
            baseline = self.request(raw)["candidates"]
            self.assertEqual(
                self.request(raw, preceding="达成")["candidates"], baseline
            )
        self.assertFalse(self.path.exists())

    def test_context_does_not_override_explicit_preference_or_learn_on_cancel(self):
        payload = {
            "action": "feedback",
            "text": "实验",
            "syllables": ["shi", "yan"],
            "feedback": 1,
        }
        p = subprocess.run(
            [
                str(ROOT / "upstream/qingjian/target/release/bilingual-ime-bridge"),
                "--vocabulary",
                str(self.path),
            ],
            input=json.dumps(payload) + "\n",
            text=True,
            encoding="utf-8",
            capture_output=True,
            check=True,
        )
        self.assertNotIn("error", json.loads(p.stdout))
        before = self.path.with_suffix(".feedback.json").read_bytes()
        self.assertEqual(
            self.request("shiyan", preceding="临床")["candidates"][0]["text"], "实验"
        )
        self.request(action="cancel")
        self.assertFalse(self.path.exists())
        self.assertEqual(self.path.with_suffix(".feedback.json").read_bytes(), before)

    def test_contextual_candidate_commits_through_real_engine(self):
        raw = "shiyan"
        first = self.request(raw, preceding="临床")["candidates"][0]
        committed = self.request(
            raw, "commit", candidate=first["text"], syllables=first["syllables"]
        )
        self.assertEqual(committed["committed"], "试验")
        self.assertEqual(committed["input"], "")
        self.assertFalse(self.path.exists())
        self.request(action="confirm", learning_token=committed["learningToken"])
        saved = json.loads(self.path.read_text(encoding="utf-8"))
        self.assertEqual(saved["entries"][0]["text"], "试验")
        self.assertNotIn("临床", self.path.read_text(encoding="utf-8"))
        self.stop()
        self.start()
        # No context was serialized; query ranking is determined by ordinary confirmed memory.
        self.assertEqual(self.request("")["candidates"], [])

    def test_invalid_prefix_is_rejected_without_echo_or_learning(self):
        for prefix in ["临床" * 17, "secret", "临床\n"]:
            self.process.stdin.write(
                json.dumps(
                    {
                        "action": "query",
                        "input": "shiyan",
                        "context": "accuracy",
                        "preceding": prefix,
                    }
                )
                + "\n"
            )
            self.process.stdin.flush()
            result = json.loads(self.responses.get(timeout=10))
            self.assertIn("error", result)
            self.assertNotIn(prefix, json.dumps(result, ensure_ascii=False))
        self.assertFalse(self.path.exists())

    def test_legacy_defaults_ignore_context(self):
        self.stop()
        self.start(focused=False)
        baseline = self.request("shiyan")["candidates"]
        self.assertEqual(
            self.request("shiyan", preceding="临床")["candidates"], baseline
        )


if __name__ == "__main__":
    unittest.main()
