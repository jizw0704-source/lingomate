"""Fixed independently authored ranking/consumption regressions, no personal data."""

import json
import os
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
# Ambiguous readings accept more than one reasonable common word. These are a
# small engineering regression set, not a corpus-based accuracy benchmark.
CASES = {
    "shi": ["是"],
    "shishi": ["事实"],
    "gongshi": ["公式"],
    "shiyan": ["实验", "试验"],
    "yiyi": ["意义"],
    "xian": ["先", "西安"],
    "gongsi": ["公司"],
    "jishu": ["技术"],
    "shijian": ["时间", "实践"],
    "gongzuo": ["工作"],
    "xuexi": ["学习"],
    "yuyan": ["语言"],
    "shuru": ["输入"],
    "fanyi": ["翻译"],
    "kaifa": ["开发"],
    "sheji": ["设计"],
    "xiangmu": ["项目"],
    "moxing": ["模型"],
    "shuju": ["数据"],
    "denglu": ["登录", "登陆"],
    "tongbu": ["同步"],
    "gengxin": ["更新"],
    "ceshi": ["测试"],
    "ziliao": ["资料"],
    "wenti": ["问题"],
    "zhishi": ["知识"],
    "shijie": ["世界"],
    "dianhua": ["电话"],
    "zhanghao": ["账号", "帐号"],
    "yonghu": ["用户"],
}


class RankingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.process = subprocess.Popen(
            [
                os.environ.get(
                    "LINGOMATE_TEST_BRIDGE",
                    str(ROOT / "upstream/qingjian/target/release/bilingual-ime-bridge"),
                ),
                os.environ.get("LINGOMATE_TEST_RESOURCES", str(ROOT)),
            ],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
            encoding="utf-8",
        )

    @classmethod
    def tearDownClass(cls):
        cls.process.stdin.close()
        cls.process.wait(timeout=5)
        cls.process.stdout.close()

    def query(self, input_, action="query", **kwargs):
        self.process.stdin.write(
            json.dumps({"input": input_, "action": action, **kwargs}) + "\n"
        )
        self.process.stdin.flush()
        result = json.loads(self.process.stdout.readline())
        self.assertNotIn("error", result)
        return result

    def test_common_words_are_on_first_page(self):
        for key, accepted in CASES.items():
            with self.subTest(input=key):
                words = [c["text"] for c in self.query(key)["candidates"][:5]]
                self.assertTrue(any(w in words for w in accepted), (key, words))

    def test_real_words_precede_synthetic_repeated_characters(self):
        for key, expected in (
            ("shishi", "实施"),
            ("yiyi", "意义"),
            ("gongshi", "公式"),
        ):
            self.assertEqual(self.query(key)["candidates"][0]["text"], expected)

    def test_long_sentence_and_partial_consumption_still_work(self):
        for key in (
            "wojintianxiangxuexiyingyu",
            "womenmingtianqugongsi",
            "zhegexiangmuxuyaoceshigongneng",
        ):
            with self.subTest(input=key):
                first = self.query(key)["candidates"][0]
                self.assertEqual("".join(first["syllables"]), key)
                result = self.query(
                    key, "commit", candidate=first["text"], syllables=first["syllables"]
                )
                self.assertEqual(result["input"], "")
                self.assertEqual(result["committed"], first["text"])
        candidate = next(
            c for c in self.query("kaifazhe")["candidates"] if c["text"] == "开发"
        )
        self.assertEqual(
            self.query(
                "kaifazhe", "commit", candidate="开发", syllables=candidate["syllables"]
            )["input"],
            "zhe",
        )

    def test_abbreviations_apostrophes_and_typo_paths_are_preserved(self):
        for key, expected in (
            ("xue'xi", "学习"),
            ("xx", "学习"),
            ("gs", "公司"),
            ("xueix", "学习"),
        ):
            with self.subTest(input=key):
                self.assertIn(
                    expected, [c["text"] for c in self.query(key)["candidates"]]
                )


if __name__ == "__main__":
    unittest.main()
