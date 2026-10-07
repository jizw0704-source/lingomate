"""记录研究快照和译词数据，并执行上游离线 Lua 测试。"""

import json
import shutil
import subprocess
from pathlib import Path

from lupa import LuaRuntime

ROOT = Path(__file__).resolve().parents[1]
QINGJIAN = ROOT / "upstream/qingjian"
RIME = ROOT / "upstream/rime-translate"
EVIDENCE = ROOT / "evidence"
WORDS = [
    ("学习", "xuexi"),
    ("喜欢", "xihuan"),
    ("方便", "fangbian"),
    ("苹果", "pingguo"),
    ("开发", "kaifa"),
    ("银行", "yinhang"),
    ("行", "xing"),
    ("工作", "gongzuo"),
    ("问题", "wenti"),
    ("明天", "mingtian"),
    ("今天", "jintian"),
    ("你好", "nihao"),
    ("朋友", "pengyou"),
    ("可以", "keyi"),
    ("需要", "xuyao"),
    ("知道", "zhidao"),
    ("研究", "yanjiu"),
    ("材料", "cailiao"),
    ("电池", "dianchi"),
    ("会议", "huiyi"),
    ("计划", "jihua"),
    ("设计", "sheji"),
    ("重要", "zhongyao"),
    ("时间", "shijian"),
    ("了解", "liaojie"),
    ("决定", "jueding"),
    ("免费", "mianfei"),
    ("支持", "zhichi"),
    ("选择", "xuanze"),
    ("谢谢", "xiexie"),
]


def snapshot(repo):
    return (
        subprocess.check_output(
            ["git", "log", "-1", "--format=%H%n%aI%n%s"], cwd=repo, text=True
        )
        .strip()
        .splitlines()
    )


def main():
    glossary = {}
    for raw in (QINGJIAN / "assets/glossary/glossary-en.tsv").read_text().splitlines():
        if raw.strip() and not raw.startswith("#"):
            word, *senses = raw.split("\t")
            glossary[word] = senses
    source = (RIME / "rime/rime_translate.lua").read_text()
    tests = (RIME / "tests/test_filter.lua").read_text()
    lua_results = []
    for fixture, layout, flag in [
        ("home", "vertical", ""),
        ("home2", "horizontal", ""),
        ("home3", "vertical", "home3"),
    ]:
        lua = LuaRuntime(unpack_returned_tuples=True)
        lines = []
        lua.globals().print = lambda *parts, _lines=lines: _lines.append(
            " ".join(map(str, parts))
        )
        lua.globals().research_layout = layout
        lua.globals().research_fixture = flag
        lua.execute("""
            local original = os.getenv
            os.getenv = function(name)
                if name == 'EXPECT_LAYOUT' then return research_layout end
                if name == 'FIXTURE' then
                    if research_fixture ~= '' then return research_fixture end
                    return nil
                end
                return original(name)
            end
        """)
        copied_fixture = EVIDENCE / "lua-fixtures" / fixture
        shutil.copytree(
            RIME / "tests/fixtures" / fixture, copied_fixture, dirs_exist_ok=True
        )
        fixture_path = str(copied_fixture)
        original_line = 'local HOME = os.getenv("HOME") or ""'
        assert source.count(original_line) == 1
        # 仅替换模块内的测试数据路径，不改进程 HOME 或上游文件。
        isolated_source = source.replace(
            original_line, "local HOME = " + json.dumps(fixture_path)
        )
        lua.globals().research_source = isolated_source
        lua.execute(
            "package.preload['rime_translate'] = function() return assert(load(research_source))() end"
        )
        lua.execute(tests)
        assert not any(line.startswith("FAIL") for line in lines), lines
        lua_results.append(
            {"fixture": fixture, "lua_version": lua.eval("_VERSION"), "checks": lines}
        )

    result = {
        "qingjian_snapshot": snapshot(QINGJIAN),
        "rime_translate_snapshot": snapshot(RIME),
        "glossary_entries": len(glossary),
        "glossary_entries_with_two_senses": sum(len(s) == 2 for s in glossary.values()),
        "glossary_entries_with_over_two_senses": sum(
            len(s) > 2 for s in glossary.values()
        ),
        "selected_words": [
            {"chinese": word, "pinyin": pinyin, "senses": glossary.get(word, [])}
            for word, pinyin in WORDS
        ],
        "upstream_lua_tests": lua_results,
        "lua_test_isolation": "上游测试使用 Lupa 内置 Lua；只将模块内 HOME 路径替换为 evidence 下复制的 fixture，未修改进程 HOME 或上游文件。",
    }
    (EVIDENCE / "source-inspection.json").write_text(
        json.dumps(result, ensure_ascii=False, indent=2) + "\n"
    )
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
