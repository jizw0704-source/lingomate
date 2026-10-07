"""只监听本机，提供静态原型与青简引擎接口；不记录输入正文。"""

import argparse
import atexit
import json
import selectors
import signal
import subprocess
import threading
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RESEARCH = ROOT.parent


class Bridge:
    def __init__(self):
        self.process = subprocess.Popen(
            [
                str(RESEARCH / "upstream/qingjian/target/release/bilingual-ime-bridge"),
                str(RESEARCH),
            ],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self.lock = threading.Lock()
        atexit.register(self.close)
        self.request({"action": "query", "input": ""})

    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
            self.process.wait(timeout=5)

    def request(self, payload):
        with self.lock:
            if self.process.poll() is not None:
                raise RuntimeError("本地引擎已停止，请重新启动原型。")
            self.process.stdin.write(json.dumps(payload, ensure_ascii=False) + "\n")
            self.process.stdin.flush()
            with selectors.DefaultSelector() as selector:
                selector.register(self.process.stdout, selectors.EVENT_READ)
                if not selector.select(timeout=10):
                    self.close()
                    raise RuntimeError("本地引擎响应超时，请重新启动原型。")
            line = self.process.stdout.readline()
            if not line:
                raise RuntimeError("本地引擎响应失败。")
            return json.loads(line)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=9037)
    args = parser.parse_args()
    if not (ROOT / "dist/index.html").exists():
        parser.error("请先运行 pnpm build")
    bridge = Bridge()

    class Handler(SimpleHTTPRequestHandler):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, directory=str(ROOT / "dist"), **kwargs)

        def log_message(self, _format, *args):
            # 不保存请求日志或输入正文。
            pass

        def send_json(self, payload, status=200):
            content = json.dumps(payload, ensure_ascii=False).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Cache-Control", "no-store")
            self.send_header("Content-Length", str(len(content)))
            self.end_headers()
            self.wfile.write(content)

        def do_GET(self):
            if self.path == "/api/health":
                self.send_json(
                    {
                        "engine": "qingjian",
                        "local": True,
                        "ready": bridge.process.poll() is None,
                    }
                )
            else:
                super().do_GET()

        def do_POST(self):
            action = {"/api/query": "query", "/api/commit": "commit"}.get(self.path)
            if action is None:
                self.send_json({"error": "接口不存在"}, 404)
                return
            origin = self.headers.get("Origin")
            allowed = {
                f"http://127.0.0.1:{args.port}",
                f"http://localhost:{args.port}",
                "http://127.0.0.1:5173",
            }
            if origin and origin not in allowed:
                self.send_json({"error": "仅接受本地原型请求"}, 403)
                return
            try:
                length = int(self.headers.get("Content-Length", "0"))
                if not 0 < length <= 16384:
                    raise ValueError("请求大小不正确")
                payload = json.loads(self.rfile.read(length))
                if not isinstance(payload, dict) or not isinstance(
                    payload.get("input"), str
                ):
                    raise TypeError("请求格式不正确")
                result = bridge.request({**payload, "action": action})
                self.send_json(result, 400 if "error" in result else 200)
            except (ValueError, TypeError, UnicodeError) as error:
                self.send_json({"error": str(error)}, 400)
            except (RuntimeError, BrokenPipeError) as error:
                self.send_json({"error": str(error)}, 503)

    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    print(f"原型已启动：http://127.0.0.1:{args.port}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        signal.signal(signal.SIGINT, signal.SIG_IGN)
    finally:
        server.server_close()
        bridge.close()


if __name__ == "__main__":
    main()
