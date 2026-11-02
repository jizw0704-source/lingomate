"""Explicit recommendation feedback: isolated files, no typing history or real stores."""

import json
import subprocess
import unittest

from mac_accuracy_test import ROOT, AccuracyTests


class FeedbackTests(AccuracyTests):
    def feedback(self, word="公示", value=1):
        request = {
            "action": "feedback",
            "text": word,
            "syllables": ["gong", "shi"],
            "feedback": value,
        }
        return self.manage(request)

    def manage(self, request):
        result = subprocess.run(
            [
                str(ROOT / "upstream/qingjian/target/release/bilingual-ime-bridge"),
                "--vocabulary",
                str(self.path),
            ],
            input=json.dumps(request) + "\n",
            text=True,
            encoding="utf-8",
            capture_output=True,
            check=True,
        )
        return json.loads(result.stdout)

    def test_feedback_never_learns_and_can_be_undone_live(self):
        baseline = self.request("gongshi")["candidates"]
        target = next(c for c in baseline if c["text"] == "公示")
        self.assertTrue(target["feedbackEligible"])
        before = self.path.read_bytes() if self.path.exists() else None
        self.assertNotIn("error", self.feedback())
        self.assertEqual(self.request("gongshi")["candidates"][0]["text"], "公示")
        self.assertEqual(self.path.read_bytes() if self.path.exists() else None, before)
        rows = self.manage({"action": "list"})["words"]
        self.assertEqual(rows[0]["count"], 0)
        self.assertTrue(rows[0]["prioritized"])
        self.assertNotIn("error", self.feedback(value=-1))
        reduced = self.request("gongshi")["candidates"]
        self.assertNotEqual(reduced[0]["text"], "公示")
        self.assertTrue(any(c["text"] == "公示" for c in reduced))
        self.feedback(value=0)
        self.assertEqual(self.request("gongshi")["candidates"], baseline)
        self.assertEqual(self.path.read_bytes() if self.path.exists() else None, before)

    def test_management_reset_and_restart(self):
        self.feedback(value=-1)
        row = self.manage({"action": "list"})["words"][0]
        self.assertEqual(row["feedback"], -1)
        self.stop()
        self.start()
        self.assertEqual(
            next(
                c for c in self.request("gongshi")["candidates"] if c["text"] == "公示"
            )["feedback"],
            -1,
        )
        self.assertNotIn(
            "error",
            self.manage(
                {
                    "action": "prioritize",
                    "text": "公示",
                    "syllables": ["gong", "shi"],
                    "prioritized": False,
                }
            ),
        )
        self.assertEqual(self.manage({"action": "list"})["words"], [])

    def test_legacy_ranking_is_unchanged_and_invalid_store_preserved(self):
        self.stop()
        self.start(focused=False)
        before = self.request("gongshi")["candidates"]
        self.feedback()
        self.assertEqual(self.request("gongshi")["candidates"], before)
        path = self.path.with_suffix(".feedback.json")
        path.write_text("broken", encoding="utf-8")
        self.assertIn("error", self.feedback(value=-1))
        self.assertEqual(path.read_text(encoding="utf-8"), "broken")

    def test_invalid_requests_and_symlink_are_rejected(self):
        self.assertIn("error", self.feedback(value=2))
        self.assertIn(
            "error",
            self.manage(
                {"action": "feedback", "text": "+", "syllables": ["jia"], "feedback": 1}
            ),
        )
        self.assertFalse(self.path.exists())
        victim = self.path.parent / "victim.json"
        victim.write_text("untouched", encoding="utf-8")
        self.path.with_suffix(".feedback.json").symlink_to(victim)
        self.assertIn("error", self.feedback())
        self.assertEqual(victim.read_text(encoding="utf-8"), "untouched")


if __name__ == "__main__":
    unittest.main()
