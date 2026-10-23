"""Evaluation comparison gates; no real bridge, host or personal vocabulary."""

import copy
import unittest

from evaluate_candidates import compare


class CompareTests(unittest.TestCase):
    def setUp(self):
        self.before = {
            "fixtures_sha256": "synthetic-fixture",
            "results": [
                {
                    "id": "sample",
                    "input": "shiyan",
                    "accepted": ["试验"],
                    "category": "learning",
                    "rank": 2,
                }
            ],
        }

    def test_improvement_and_missing_are_compared(self):
        after = copy.deepcopy(self.before)
        after["results"][0]["rank"] = 1
        self.assertEqual(compare(self.before, after), [])
        after["results"][0]["rank"] = None
        self.assertEqual(compare(self.before, after), ["sample"])

    def test_cannot_compare_changed_expectations_or_learning_seed(self):
        after = copy.deepcopy(self.before)
        after["fixtures_sha256"] = "different-learning-seed"
        with self.assertRaises(ValueError):
            compare(self.before, after)
        after = copy.deepcopy(self.before)
        after["results"][0]["accepted"] = ["实验"]
        with self.assertRaises(ValueError):
            compare(self.before, after)
        after = copy.deepcopy(self.before)
        after["results"] = []
        with self.assertRaises(ValueError):
            compare(self.before, after)


if __name__ == "__main__":
    unittest.main()
