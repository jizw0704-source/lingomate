"""Isolated protocol tests; no email delivery, personal records or real secrets."""

import json
import tempfile
import threading
import unittest
import uuid
from urllib.error import HTTPError
from urllib.request import Request, urlopen

from server import APIError, Handler, Server, Store


class AccountTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.now = 1000000
        self.mail = {}
        self.store = Store(self.directory.name, self.mail.__setitem__, lambda: self.now)

    def tearDown(self):
        self.store.close()
        self.directory.cleanup()

    def login(self, email="one@example.invalid"):
        self.store.send_code(email, "test")
        return self.store.verify(email, self.mail[email])

    def event(self, kind="study", mastered=False):
        return {
            "id": str(uuid.uuid4()),
            "english": "learn",
            "chinese": "学习",
            "pos": "v.",
            "kind": kind,
            "mastered": mastered,
        }

    def fails(self, status, action):
        with self.assertRaises(APIError) as error:
            action()
        self.assertEqual(error.exception.status, status)

    def test_code_replay_expiry_and_attempts(self):
        self.store.send_code("one@example.invalid", "test")
        code = self.mail["one@example.invalid"]
        for _ in range(5):
            wrong = "999999" if code != "999999" else "000000"
            self.fails(
                403, lambda wrong=wrong: self.store.verify("one@example.invalid", wrong)
            )
        self.fails(403, lambda: self.store.verify("one@example.invalid", code))
        self.now += 61
        self.store.send_code("one@example.invalid", "test")
        self.now += 601
        self.fails(
            403,
            lambda: self.store.verify(
                "one@example.invalid", self.mail["one@example.invalid"]
            ),
        )
        tokens = self.login("two@example.invalid")
        self.fails(
            403,
            lambda: self.store.verify(
                "two@example.invalid", self.mail["two@example.invalid"]
            ),
        )
        self.assertNotIn(
            tokens["access_token"],
            str(self.store.db.execute("SELECT * FROM sessions").fetchall()),
        )

    def test_rate_and_mail_failure(self):
        self.store.send_code("one@example.invalid", "test")
        self.fails(429, lambda: self.store.send_code("one@example.invalid", "test"))
        self.store.sender = lambda *_: (_ for _ in ()).throw(
            RuntimeError("private-mail-detail")
        )
        self.fails(503, lambda: self.store.send_code("other@example.invalid", "test"))
        self.assertIsNone(
            self.store.db.execute(
                "SELECT * FROM codes WHERE email=?", ("other@example.invalid",)
            ).fetchone()
        )
        self.fails(429, lambda: self.store.send_code("other@example.invalid", "test"))

    def test_refresh_rotation_expiry_logout(self):
        tokens = self.login()
        self.now += 901
        self.fails(401, lambda: self.store.words(tokens["access_token"]))
        renewed = self.store.refresh(tokens["refresh_token"])
        self.fails(401, lambda: self.store.refresh(tokens["refresh_token"]))
        self.assertEqual(renewed["user"], tokens["user"])
        self.store.logout(renewed["access_token"], renewed["refresh_token"])
        self.fails(401, lambda: self.store.refresh(renewed["refresh_token"]))
        self.fails(401, lambda: self.store.words(renewed["access_token"]))

    def test_slow_mail_does_not_block_sync_and_reserves_quota(self):
        tokens = self.login()
        sending, release = threading.Event(), threading.Event()
        result = []

        def slow_sender(email, code):
            sending.set()
            if not release.wait(3):
                raise RuntimeError("test_timeout")
            self.mail[email] = code

        def send():
            try:
                result.append(self.store.send_code("slow@example.invalid", "slow-ip"))
            except APIError as error:
                result.append(error.status)

        self.store.sender = slow_sender
        thread = threading.Thread(target=send)
        thread.start()
        try:
            self.assertTrue(sending.wait(1))
            # Query while SMTP is intentionally blocked, not after releasing it.
            self.assertEqual(self.store.words(tokens["access_token"]), [])
            self.fails(
                429, lambda: self.store.send_code("slow@example.invalid", "slow-ip")
            )
            self.fails(403, lambda: self.store.verify("slow@example.invalid", "000000"))
        finally:
            release.set()
            thread.join(4)
        self.assertFalse(thread.is_alive())
        self.assertEqual(result, [{"sent": True}])
        self.store.verify("slow@example.invalid", self.mail["slow@example.invalid"])

    def test_isolation_idempotence_and_mastery(self):
        one, two = self.login(), self.login("two@example.invalid")
        event = self.event()
        self.assertEqual(self.store.events(one["access_token"], [event]), [event["id"]])
        self.store.events(one["access_token"], [event, self.event("mastered", True)])
        self.assertEqual(self.store.words(two["access_token"]), [])
        words = self.store.words(one["access_token"])
        self.assertEqual(words[0]["uses"], 1)
        self.assertTrue(words[0]["mastered"])
        self.store.events(
            one["access_token"], [self.event(), self.event("mastered", False)]
        )
        self.assertEqual(self.store.words(one["access_token"])[0]["uses"], 2)
        self.assertFalse(self.store.words(one["access_token"])[0]["mastered"])
        self.fails(
            409,
            lambda: self.store.events(
                one["access_token"], [dict(event, chinese="朋友")]
            ),
        )

    def test_body_rejection_and_transaction_rollback(self):
        tokens = self.login()
        valid = self.event()
        invalid = dict(
            self.event(),
            chinese="我今天想学习英语。",
            english="I want to learn English today.",
        )
        self.fails(
            400, lambda: self.store.events(tokens["access_token"], [valid, invalid])
        )
        self.assertEqual(self.store.words(tokens["access_token"]), [])
        self.fails(
            400,
            lambda: self.store.events(
                tokens["access_token"], [dict(valid, user_id="other")]
            ),
        )
        self.store.events(tokens["access_token"], [valid])
        self.fails(
            409,
            lambda: self.store.events(
                tokens["access_token"], [self.event(), dict(valid, english="friend")]
            ),
        )
        self.assertEqual(self.store.words(tokens["access_token"])[0]["uses"], 1)

    def test_http_contract(self):
        server = Server(("127.0.0.1", 0), Handler)
        server.store = self.store
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        base = f"http://127.0.0.1:{server.server_port}"

        def call(path, body=None, token=None):
            headers = {"Content-Type": "application/json"}
            if token:
                headers["Authorization"] = "Bearer " + token
            request = Request(
                base + path,
                data=json.dumps(body).encode() if body is not None else None,
                headers=headers,
            )
            with urlopen(request, timeout=3) as response:
                self.assertEqual(response.headers["Cache-Control"], "no-store")
                return json.load(response)

        try:
            self.assertTrue(call("/health")["ok"])
            self.assertTrue(
                call("/v1/auth/code", {"email": "one@example.invalid"})["sent"]
            )
            tokens = call(
                "/v1/auth/verify",
                {
                    "email": "one@example.invalid",
                    "code": self.mail["one@example.invalid"],
                },
            )
            event = self.event()
            self.assertEqual(
                call("/v1/events", [event], tokens["access_token"]), [event["id"]]
            )
            self.assertEqual(
                call("/v1/words", token=tokens["access_token"])[0]["uses"], 1
            )
            with self.assertRaises(HTTPError) as error:
                call("/v1/words")
            self.assertEqual(error.exception.code, 401)
        finally:
            server.shutdown()
            server.server_close()
            thread.join()


if __name__ == "__main__":
    unittest.main()
