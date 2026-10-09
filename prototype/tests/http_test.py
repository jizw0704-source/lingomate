"""验证运行中的本地服务接口；默认端口 9037。"""

import json
import unittest
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:9037"


class HttpTests(unittest.TestCase):
    def post(self, path, payload, origin=None):
        headers = {"Content-Type": "application/json"}
        if origin:
            headers["Origin"] = origin
        request = urllib.request.Request(
            BASE + path, data=json.dumps(payload).encode(), headers=headers
        )
        try:
            with urllib.request.urlopen(request, timeout=5) as response:
                return response.status, json.load(response)
        except urllib.error.HTTPError as error:
            return error.code, json.load(error)

    def test_health_and_page(self):
        with urllib.request.urlopen(BASE + "/api/health", timeout=5) as response:
            self.assertTrue(json.load(response)["ready"])
        with urllib.request.urlopen(BASE, timeout=5) as response:
            self.assertIn("灵果", response.read().decode())

    def test_query_and_commit_third_translation(self):
        status, data = self.post("/api/query", {"input": "xuexi"})
        self.assertEqual(status, 200)
        candidate = data["candidates"][0]
        status, data = self.post(
            "/api/commit",
            {
                "input": "xuexi",
                "candidate": candidate["text"],
                "syllables": candidate["syllables"],
                "english": "learning",
            },
        )
        self.assertEqual(status, 200)
        self.assertEqual(data["committed"], "learning")
        self.assertEqual(data["input"], "")

    def test_invalid_payload_and_external_origin(self):
        for payload in ([], {"input": 12}, {"input": "中"}):
            status, data = self.post("/api/query", payload)
            self.assertEqual(status, 400)
            self.assertIn("error", data)
        self.assertEqual(
            self.post("/api/query", {"input": "xuexi"}, origin="https://example.com")[
                0
            ],
            403,
        )

    def test_unknown_route(self):
        self.assertEqual(self.post("/api/unknown", {"input": "xuexi"})[0], 404)


if __name__ == "__main__":
    unittest.main()
