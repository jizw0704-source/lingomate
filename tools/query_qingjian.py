"""用原版 CLI 查询研究词表，结果保存在 evidence。"""

import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
QINGJIAN = ROOT / "upstream/qingjian"
EVIDENCE = ROOT / "evidence"


def main():
    words = json.loads((EVIDENCE / "source-inspection.json").read_text())[
        "selected_words"
    ]
    command = [
        str(QINGJIAN / "target/release/qingjian-cli"),
        "--config",
        str(ROOT / "tools/fixtures/offline-config.toml"),
        "--limit",
        "9",
        *[word["pinyin"] for word in words],
    ]
    result = subprocess.run(
        command, cwd=QINGJIAN, text=True, capture_output=True, check=True
    )
    (EVIDENCE / "qingjian-queries.txt").write_text(result.stdout)
    (EVIDENCE / "qingjian-query-startup.log").write_text(result.stderr)
    blocks = result.stdout.split("> ")[1:]
    assert len(blocks) == len(words), "查询结果数量与输入词表不一致"
    summary = []
    for word, block in zip(words, blocks, strict=True):
        assert block.splitlines()[0] == word["pinyin"]
        candidates = re.findall(r"^\s+\d+\.\s+(\S+)", block, flags=re.MULTILINE)
        summary.append(
            {
                **word,
                "first_candidate": candidates[0] if candidates else None,
                "expected_in_first_page": word["chinese"] in candidates,
            }
        )
    (EVIDENCE / "query-summary.json").write_text(
        json.dumps(summary, ensure_ascii=False, indent=2) + "\n"
    )
    typed = subprocess.run(
        command[:5] + ["--typing", "xuexi", "kaifa", "fangbian"],
        cwd=QINGJIAN,
        text=True,
        capture_output=True,
        check=True,
    )
    (EVIDENCE / "qingjian-typing.txt").write_text(typed.stdout)
    print(result.stdout[:5000])
    print(typed.stdout)


if __name__ == "__main__":
    main()
