"""针对真实青简适配器验证候选、释义提交、部分拼音及拒绝无效请求。"""

import json
import subprocess
import unittest
from pathlib import Path

RESEARCH = Path(__file__).resolve().parents[2]


class EngineTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.process = subprocess.Popen(
            [
                str(RESEARCH / "upstream/qingjian/target/release/bilingual-ime-bridge"),
                str(RESEARCH),
            ],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
        )

    @classmethod
    def tearDownClass(cls):
        cls.process.stdin.close()
        cls.process.wait(timeout=5)
        cls.process.stdout.close()

    def request(self, input_, action="query", **kwargs):
        self.process.stdin.write(
            json.dumps({"input": input_, "action": action, **kwargs}) + "\n"
        )
        self.process.stdin.flush()
        return json.loads(self.process.stdout.readline())

    def candidate(self, input_, word):
        return next(c for c in self.request(input_)["candidates"] if c["text"] == word)

    def commit(self, input_, word, **kwargs):
        candidate = self.candidate(input_, word)
        return self.request(
            input_, "commit", candidate=word, syllables=candidate["syllables"], **kwargs
        )

    def test_all_thirty_sample_words_have_candidates_and_details(self):
        words = json.loads((RESEARCH / "prototype/data/samples.json").read_text())
        for word in words:
            with self.subTest(word=word["chinese"]):
                candidate = self.candidate(word["pinyin"], word["chinese"])
                self.assertTrue(candidate["hasDetails"])
                self.assertTrue(candidate["translations"])

    def test_chinese_commit(self):
        result = self.commit("xuexi", "学习")
        self.assertEqual(result["committed"], "学习")
        self.assertEqual(result["input"], "")

    def test_second_and_third_english_commit(self):
        for word in ("learn", "learning"):
            result = self.commit("xuexi", "学习", english=word)
            self.assertEqual(result["committed"], word)
            self.assertEqual(result["input"], "")

    def test_partial_commit_preserves_remaining_pinyin(self):
        for kwargs in ({}, {"english": "develop"}):
            result = self.commit("kaifazhe", "开发", **kwargs)
            self.assertEqual(result["input"], "zhe")
            self.assertTrue(result["candidates"])

    def test_unknown_translation_or_candidate_is_rejected(self):
        self.assertIn("error", self.commit("xuexi", "学习", english="fabricated"))
        self.assertIn(
            "error",
            self.request(
                "xuexi", "commit", candidate="银行", syllables=["yin", "hang"]
            ),
        )

    def test_empty_and_invalid_input(self):
        self.assertEqual(self.request("")["candidates"], [])
        self.assertIn("error", self.request("中"))
        self.assertIn("error", self.request("a" * 241))

    def test_long_sentence_composition_is_not_truncated(self):
        pinyin = (
            "woxiangxuexiyingyuyinweitakeyibangzhuwohegengduodepengyou"
            "jiaoliubingqieliaojiebutongdewenhua"
        )
        result = self.request(pinyin)
        self.assertGreater(len(pinyin), 80)
        self.assertEqual(result["input"], pinyin)
        self.assertTrue(result["candidates"])
        self.assertGreater(len(result["candidates"][0]["text"]), 20)

    def test_later_pages_preserve_candidate_and_english_validation(self):
        candidates = self.request("shi")["candidates"]
        self.assertGreater(len(candidates), 9)
        self.assertLessEqual(len(candidates), 64)
        for candidate in (candidates[5], candidates[-1]):
            result = self.request(
                "shi",
                "commit",
                candidate=candidate["text"],
                syllables=candidate["syllables"],
            )
            self.assertEqual(result["committed"], candidate["text"])
            self.assertEqual(result["input"], "")
        translated = next(c for c in candidates[5:] if c["translations"])
        english = translated["translations"][0]["word"]
        result = self.request(
            "shi",
            "commit",
            candidate=translated["text"],
            syllables=translated["syllables"],
            english=english,
        )
        self.assertEqual(result["committed"], english)
        self.assertEqual(result["input"], "")
        self.assertIn(
            "error",
            self.request(
                "shi",
                "commit",
                candidate=translated["text"],
                syllables=translated["syllables"],
                english="fabricated",
            ),
        )


if __name__ == "__main__":
    unittest.main()
