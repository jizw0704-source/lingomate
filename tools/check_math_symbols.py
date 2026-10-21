"""Validate the independently curated math catalog and its complete coverage table."""

import argparse
import json
import re
import unicodedata
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CATEGORIES = {
    "四则与运算",
    "比较与关系",
    "乘方与根式",
    "比例与百分数",
    "常用分数",
    "微积分与无穷",
    "集合与数系",
    "逻辑与推理",
    "几何与角度",
    "统计与概率",
    "括号与定界",
    "数学省略记号",
    "上下标",
    "常用函数记号",
}
# Independent acceptance requirements; omission of a category or an everyday alias fails the build.
REQUIRED = {
    "jia": "+",
    "jiahao": "+",
    "jiashang": "+",
    "duishu": "log",
    "ziranduishu": "ln",
    "zhengxian": "sin",
    "jiankuohao": "⟨⟩",
    "kaigenhao": "√",
    "baifenbi": "%",
    "jian": "−",
    "jianhao": "−",
    "cheng": "×",
    "chenghao": "×",
    "chu": "÷",
    "chuhao": "÷",
    "dengyu": "=",
    "denghao": "=",
    "dayu": ">",
    "dayuhao": ">",
    "xiaoyu": "<",
    "xiaoyuhao": "<",
    "budengyu": "≠",
    "dayudengyu": "≥",
    "xiaoyudengyu": "≤",
    "yuedengyu": "≈",
    "zhengfu": "±",
    "pingfang": "²",
    "lifang": "³",
    "genhao": "√",
    "baifenhao": "%",
    "qianfenhao": "‰",
    "erfenzhiyi": "½",
    "wuqiongda": "∞",
    "qiuhe": "∑",
    "liancheng": "∏",
    "jifen": "∫",
    "piandao": "∂",
    "tidu": "∇",
    "shuyu": "∈",
    "bushuyu": "∉",
    "kongji": "∅",
    "bingji": "∪",
    "jiaoji": "∩",
    "renyi": "∀",
    "cunzai": "∃",
    "yinwei": "∵",
    "suoyi": "∴",
    "jiaodu": "°",
    "chuizhi": "⊥",
    "pingxing": "∥",
    "yuanzhoulv": "π",
    "biaozhuncha": "σ",
    "yangbenfangcha": "s²",
    "kuohao": "（）",
    "fangkuohao": "[]",
    "huakuohao": "{}",
    "xiangxiaquzheng": "⌊⌋",
}


def validate(data):
    assert data["version"] == 1 and data["license"] == "GPL-3.0-or-later"
    entries = data["entries"]
    assert 100 <= len(entries) <= 512
    assert {entry["category"] for entry in entries} == CATEGORIES
    aliases = {}
    names = set()
    for entry in entries:
        assert entry["name"] and entry["name"] not in names, entry["name"]
        names.add(entry["name"])
        assert 1 <= len(entry["aliases"]) <= 16
        assert 1 <= len(entry["outputs"]) <= 3
        assert len(set(entry["outputs"])) == len(entry["outputs"])
        for output in entry["outputs"]:
            assert output and len(output.encode("utf-16-le")) <= 64
            assert not any(
                unicodedata.category(char).startswith("C") for char in output
            )
        for alias in entry["aliases"]:
            assert re.fullmatch("[a-z]{1,96}", alias), alias
            assert alias not in aliases, alias
            aliases[alias] = entry["outputs"]
    for alias, output in REQUIRED.items():
        assert output in aliases.get(alias, []), (
            f"Missing required mapping: {alias} -> {output}"
        )
    for prefix, characters in [("shangbiao", "⁰¹²³⁴⁵⁶⁷⁸⁹"), ("xiabiao", "₀₁₂₃₄₅₆₇₈₉")]:
        for pinyin, char in zip(
            ["ling", "yi", "er", "san", "si", "wu", "liu", "qi", "ba", "jiu"],
            characters,
            strict=True,
        ):
            assert aliases[prefix + pinyin] == [char]
    return aliases


def document(data):
    aliases = validate(data)
    lines = [
        "# 常用数学符号覆盖清单",
        "",
        f"Mac 0.12.16 / build 38：{len(CATEGORIES)} 类、{len(data['entries'])} 项、{len(aliases)} 个完整关键词／别称。",
        "",
        "本表由项目独立整理的 data/math-symbols.json 再生成，按项目 GPL-3.0-or-later 许可提供；不包含第三方字体或表情图像。",
        "文字字符与名称参考核对：[NIST 常用数学记号](https://dlmf.nist.gov/front/introduction)、[Unicode 数学字符分类](https://www.unicode.org/Public/math/revision-15/MathClassEx-15.html)。没有从这些页面批量导入语料或图片。",
        "",
        "只在中文模式按完整关键词匹配；普通候选的前两项及相对顺序保留，再补充最多三项符号。回车仍确认英文字母，Shift 英文直输不匹配。选择的符号按原样输出，不写入中文词库或英文学习记录。",
        "",
        "表内括号对直接插入两个括号，光标留在末尾，不自动移入括号；需要分别输入时可用左／右括号关键词。符号是纯文本，不是公式编辑器；上下标和分数字符仅覆盖表内常用形式。数学记号在不同领域可能有约定差异，按候选主动选择。",
        "",
        "测试要求涵盖所有别称、重复／冲突、每项≤3个输出、必备基础符号、0–9上下标、原候选排序、首屏索引、分页、失效选择、开关及记忆隔离；真实键盘验收另记。",
        "",
    ]
    for category in dict.fromkeys(entry["category"] for entry in data["entries"]):
        lines += [
            f"## {category}",
            "",
            "| 名称 | 完整拼音／别称 | 候选 |",
            "| --- | --- | --- |",
        ]
        for entry in data["entries"]:
            if entry["category"] != category:
                continue
            outputs = "、".join(
                "`" + s.replace("|", "&#124;") + "`" for s in entry["outputs"]
            )
            lines.append(
                f"| {entry['name']} | {' / '.join(entry['aliases'])} | {outputs} |"
            )
        lines.append("")
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--write-doc", action="store_true")
    args = parser.parse_args()
    data = json.loads((ROOT / "data/math-symbols.json").read_text(encoding="utf-8"))
    aliases = validate(data)
    doc = ROOT / "docs/MATH_SYMBOLS.md"
    expected = document(data)
    if args.write_doc:
        doc.write_text(expected, encoding="utf-8")
    else:
        assert doc.read_text(encoding="utf-8") == expected, (
            "Math coverage table stale; run --write-doc"
        )
    counts = Counter(entry["category"] for entry in data["entries"])
    print(
        f"Verified math mappings: {len(counts)} categories, {len(data['entries'])} entries, {len(aliases)} aliases."
    )


if __name__ == "__main__":
    main()
