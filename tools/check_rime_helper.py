"""编译并验证上游 Swift helper，运行目录隔离在研究目录。"""

import json
import os
import socket
import subprocess
import time
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / "evidence"
RIME = ROOT / "upstream/rime-translate"


def main():
    isolated = EVIDENCE / "helper-fixture"
    (isolated / "Library/Rime").mkdir(parents=True, exist_ok=True)
    original = (RIME / "helper/main.swift").read_text()
    app_path = "FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]"
    home_path = "FileManager.default.homeDirectoryForCurrentUser"
    assert original.count(app_path) == original.count(home_path) == 1
    source = original.replace(
        app_path, "URL(fileURLWithPath: " + json.dumps(str(isolated)) + ")"
    )
    source = source.replace(
        home_path, "URL(fileURLWithPath: " + json.dumps(str(isolated)) + ")"
    )
    derived = EVIDENCE / "rime-helper-isolated.swift"
    derived.write_text(source)
    binary = EVIDENCE / "rime-helper-isolated"
    env = {**os.environ, "DEVELOPER_DIR": "/Library/Developer/CommandLineTools"}
    subprocess.run(
        [
            "/Library/Developer/CommandLineTools/usr/bin/swiftc",
            "-O",
            "-target",
            "arm64-apple-macosx13.0",
            "-sdk",
            "/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk",
            str(derived),
            "-lsqlite3",
            "-o",
            str(binary),
        ],
        env=env,
        check=True,
    )
    config = isolated / "offline-config.json"
    config.write_text("{}\n")
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    log = (EVIDENCE / "rime-helper-runtime.log").open("w")
    process = subprocess.Popen(
        [
            str(binary),
            "--db",
            str(EVIDENCE / "rime-sample.db"),
            "--config",
            str(config),
            "--port",
            str(port),
        ],
        stdout=log,
        stderr=subprocess.STDOUT,
    )
    base = f"http://127.0.0.1:{port}"

    def get(path):
        with urllib.request.urlopen(base + path, timeout=3) as response:
            return json.load(response)

    try:
        for _ in range(50):
            try:
                health = get("/health")
                break
            except OSError:
                if process.poll() is not None:
                    raise RuntimeError("helper 提前退出")
                time.sleep(0.1)
        else:
            raise RuntimeError("helper 未就绪")
        assert health == {"status": "ok", "dict": True, "ai": False}, health
        results = {"health": health}
        for word, expected in [
            ("苹果", "apple"),
            ("电脑", "computer"),
            ("果实", "fruit"),
        ]:
            result = get("/lookup?q=" + urllib.parse.quote(word))
            assert result["source"] == "dict" and expected in result["en"].split("|"), (
                result
            )
            results[word] = result
        missing = get("/lookup?q=" + urllib.parse.quote("测试未知词"))
        assert missing["en"] == "" and missing["source"] == "pending", missing
        results["unknown"] = missing
        results["scope"] = (
            "上游 helper 仅两处目录解析替换为研究路径；样例数据库 22 词条，未启用 AI，未安装系统服务。"
        )
        (EVIDENCE / "rime-helper-results.json").write_text(
            json.dumps(results, ensure_ascii=False, indent=2) + "\n"
        )
        print(json.dumps(results, ensure_ascii=False, indent=2))
    finally:
        process.terminate()
        process.wait(timeout=5)
        log.close()


if __name__ == "__main__":
    main()
