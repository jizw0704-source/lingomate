"""Self-hosted email OTP and per-user word sync; Python standard library only."""

import argparse
import hashlib
import hmac
import ipaddress
import json
import os
import re
import secrets
import smtplib
import sqlite3
import ssl
import threading
import time
import uuid
from email.message import EmailMessage
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

EMAIL = re.compile(
    r"[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+"
)
SCHEMA = """
CREATE TABLE IF NOT EXISTS users(id TEXT PRIMARY KEY, email TEXT UNIQUE NOT NULL);
CREATE TABLE IF NOT EXISTS codes(email TEXT PRIMARY KEY, digest TEXT, expires REAL, attempts INTEGER);
CREATE TABLE IF NOT EXISTS rate(bucket TEXT, at REAL);
CREATE INDEX IF NOT EXISTS rate_lookup ON rate(bucket, at);
CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY, user_id TEXT, access TEXT UNIQUE,
    refresh TEXT UNIQUE, access_until REAL, refresh_until REAL);
CREATE TABLE IF NOT EXISTS words(user_id TEXT, key TEXT, english TEXT, chinese TEXT, pos TEXT,
    uses INTEGER, mastered INTEGER, PRIMARY KEY(user_id, key));
CREATE TABLE IF NOT EXISTS events(user_id TEXT, id TEXT, payload TEXT, PRIMARY KEY(user_id, id));
"""


class APIError(Exception):
    def __init__(self, status, message):
        self.status, self.message = status, message


def require(condition, status=400, message="invalid_request"):
    if not condition:
        raise APIError(status, message)


def email_value(value):
    require(isinstance(value, str) and len(value) <= 254 and EMAIL.fullmatch(value))
    return value.lower()


def digest(value):
    return hashlib.sha256(value.encode()).hexdigest()


class Store:
    def __init__(self, directory, sender, clock=time.time):
        directory = Path(directory)
        directory.mkdir(parents=True, exist_ok=True, mode=0o700)
        os.chmod(directory, 0o700)
        pepper_file = directory / "otp-key"
        if not pepper_file.exists():
            with pepper_file.open("xb") as file:
                os.chmod(pepper_file, 0o600)
                file.write(secrets.token_bytes(32))
        self.pepper = pepper_file.read_bytes()
        require(len(self.pepper) == 32, 503, "storage_unavailable")
        self.db = sqlite3.connect(
            directory / "accounts.sqlite3", check_same_thread=False
        )
        os.chmod(directory / "accounts.sqlite3", 0o600)
        self.db.row_factory = sqlite3.Row
        self.db.executescript(SCHEMA)
        self.lock = threading.RLock()
        self.sender, self.clock = sender, clock

    def close(self):
        self.db.close()

    def code_digest(self, email, code):
        return hmac.new(
            self.pepper, (email + "|" + code).encode(), hashlib.sha256
        ).hexdigest()

    def send_code(self, email, ip):
        email = email_value(email)
        with self.lock, self.db:
            now = self.clock()
            self.db.execute("DELETE FROM rate WHERE at <= ?", (now - 3600,))
            for bucket, limit in [("email:" + email, 10), ("ip:" + ip, 60)]:
                rows = self.db.execute(
                    "SELECT at FROM rate WHERE bucket=?", (bucket,)
                ).fetchall()
                require(len(rows) < limit, 429, "try_later")
                if bucket.startswith("email:"):
                    require(
                        not rows or now - max(r[0] for r in rows) >= 60,
                        429,
                        "try_later",
                    )
            code = f"{secrets.randbelow(1000000):06d}"
            # Reserve quota before network I/O, including failed attempts. A slow
            # mail server must not hold the database lock for every user's sync.
            self.db.executemany(
                "INSERT INTO rate VALUES(?,?)",
                [("email:" + email, now), ("ip:" + ip, now)],
            )
        try:
            self.sender(email, code)
        except (OSError, smtplib.SMTPException, RuntimeError):
            # Do not return SMTP exceptions, addresses, credentials or the code.
            raise APIError(503, "mail_unavailable") from None
        with self.lock, self.db:
            self.db.execute(
                "INSERT OR REPLACE INTO codes VALUES(?,?,?,0)",
                (email, self.code_digest(email, code), self.clock() + 600),
            )
        return {"sent": True}

    def tokens(self, user_id, email, session_id=None):
        now = self.clock()
        access, refresh = secrets.token_urlsafe(32), secrets.token_urlsafe(32)
        self.db.execute(
            "INSERT OR REPLACE INTO sessions VALUES(?,?,?,?,?,?)",
            (
                session_id or str(uuid.uuid4()),
                user_id,
                digest(access),
                digest(refresh),
                now + 900,
                now + 2592000,
            ),
        )
        return {
            "access_token": access,
            "refresh_token": refresh,
            "expires_in": 900,
            "user": {"id": user_id, "email": email},
        }

    def verify(self, email, code):
        email = email_value(email)
        require(isinstance(code, str) and re.fullmatch(r"[0-9]{6}", code))
        # Failed attempts must commit even when verification returns an error.
        with self.lock:
            row = self.db.execute(
                "SELECT * FROM codes WHERE email=?", (email,)
            ).fetchone()
            require(
                row and row["expires"] > self.clock() and row["attempts"] < 5,
                403,
                "invalid_code",
            )
            with self.db:
                self.db.execute(
                    "UPDATE codes SET attempts=attempts+1 WHERE email=?", (email,)
                )
            require(
                hmac.compare_digest(row["digest"], self.code_digest(email, code)),
                403,
                "invalid_code",
            )
            with self.db:
                user = self.db.execute(
                    "SELECT * FROM users WHERE email=?", (email,)
                ).fetchone()
                user_id = user["id"] if user else str(uuid.uuid4())
                if not user:
                    self.db.execute("INSERT INTO users VALUES(?,?)", (user_id, email))
                self.db.execute("DELETE FROM codes WHERE email=?", (email,))
                return self.tokens(user_id, email)

    def refresh(self, token):
        require(isinstance(token, str) and 1 <= len(token) <= 256, 401, "signed_out")
        with self.lock, self.db:
            row = self.db.execute(
                "SELECT sessions.*,users.email FROM sessions JOIN users ON users.id=sessions.user_id WHERE refresh=?",
                (digest(token),),
            ).fetchone()
            require(row and row["refresh_until"] > self.clock(), 401, "signed_out")
            return self.tokens(row["user_id"], row["email"], row["id"])

    def owner(self, token):
        require(isinstance(token, str) and 1 <= len(token) <= 256, 401, "signed_out")
        row = self.db.execute(
            "SELECT * FROM sessions WHERE access=?", (digest(token),)
        ).fetchone()
        require(row and row["access_until"] > self.clock(), 401, "signed_out")
        return row["user_id"]

    def logout(self, token, refresh):
        require(isinstance(token, str) and isinstance(refresh, str), 401, "signed_out")
        with self.lock, self.db:
            # An expired access token can revoke only its own paired refresh session.
            self.db.execute(
                "DELETE FROM sessions WHERE access=? AND refresh=?",
                (digest(token), digest(refresh)),
            )
        return {"signed_out": True}

    @staticmethod
    def event_value(event):
        require(
            isinstance(event, dict)
            and set(event) == {"id", "english", "chinese", "pos", "kind", "mastered"}
        )
        try:
            uuid.UUID(event["id"])
        except (ValueError, TypeError, AttributeError):
            raise APIError(400, "invalid_request") from None
        for key in ("english", "chinese", "pos", "kind"):
            require(isinstance(event[key], str))
        english, chinese, pos = event["english"], event["chinese"], event["pos"]
        require(1 <= len(english) <= 80 and all(32 <= ord(c) < 127 for c in english))
        require(
            1 <= len(chinese) <= 16 and all(0x3400 <= ord(c) <= 0x9FFF for c in chinese)
        )
        require(len(pos) <= 24 and all(ord(c) >= 32 and ord(c) != 127 for c in pos))
        require(not any("|" in v for v in (english, chinese, pos)))
        require(
            event["kind"] in ("study", "mastered")
            and isinstance(event["mastered"], bool)
        )
        # Canonical UUID prevents case aliases from counting one event twice.
        event = dict(event, id=str(uuid.UUID(event["id"])))
        return event, english.lower() + "|" + chinese + "|" + pos

    def events(self, token, values):
        require(isinstance(values, list) and len(values) <= 5000)
        parsed = [self.event_value(value) for value in values]
        with self.lock, self.db:
            owner = self.owner(token)
            ack = []
            for event, key in parsed:
                payload = json.dumps(event, ensure_ascii=False, sort_keys=True)
                previous = self.db.execute(
                    "SELECT payload FROM events WHERE user_id=? AND id=?",
                    (owner, event["id"]),
                ).fetchone()
                if previous:
                    require(previous[0] == payload, 409, "event_conflict")
                else:
                    existing = self.db.execute(
                        "SELECT uses,mastered FROM words WHERE user_id=? AND key=?",
                        (owner, key),
                    ).fetchone()
                    require(
                        existing
                        or self.db.execute(
                            "SELECT COUNT(*) FROM words WHERE user_id=?", (owner,)
                        ).fetchone()[0]
                        < 5000,
                        409,
                        "word_limit",
                    )
                    uses, mastered = (existing[0], existing[1]) if existing else (0, 0)
                    if event["kind"] == "study":
                        uses += 1
                    else:
                        mastered = int(event["mastered"])
                    self.db.execute(
                        "INSERT OR REPLACE INTO words VALUES(?,?,?,?,?,?,?)",
                        (
                            owner,
                            key,
                            event["english"],
                            event["chinese"],
                            event["pos"],
                            uses,
                            mastered,
                        ),
                    )
                    self.db.execute(
                        "INSERT INTO events VALUES(?,?,?)",
                        (owner, event["id"], payload),
                    )
                ack.append(event["id"])
            return ack

    def words(self, token):
        with self.lock:
            owner = self.owner(token)
            return [
                {
                    "english": r["english"],
                    "chinese": r["chinese"],
                    "pos": r["pos"],
                    "uses": r["uses"],
                    "mastered": bool(r["mastered"]),
                }
                for r in self.db.execute(
                    "SELECT * FROM words WHERE user_id=? ORDER BY english", (owner,)
                )
            ]


def smtp_sender():
    host = os.environ.get("IME_SMTP_HOST", "")
    sender = os.environ.get("IME_MAIL_FROM", "")
    require(
        host and sender and "\n" not in sender and "\r" not in sender,
        503,
        "smtp_not_configured",
    )
    mode = os.environ.get("IME_SMTP_MODE", "starttls")
    require(mode in ("starttls", "ssl"), 503, "smtp_not_configured")
    port = int(os.environ.get("IME_SMTP_PORT", "465" if mode == "ssl" else "587"))

    def send(email, code):
        mail = EmailMessage()
        mail["From"], mail["To"], mail["Subject"] = (
            sender,
            email,
            "灵果 · 登录验证码",
        )
        mail.set_content(
            f"你的登录验证码是 {code}，10 分钟内有效。\n如果并非你本人操作，请忽略此邮件。"
        )
        context = ssl.create_default_context()
        connection = (
            smtplib.SMTP_SSL(host, port, timeout=20, context=context)
            if mode == "ssl"
            else smtplib.SMTP(host, port, timeout=20)
        )
        with connection as client:
            if mode == "starttls":
                client.starttls(context=context)
            if os.environ.get("IME_SMTP_USER"):
                client.login(
                    os.environ["IME_SMTP_USER"], os.environ.get("IME_SMTP_PASSWORD", "")
                )
            if client.send_message(mail):
                raise RuntimeError("delivery_rejected")

    return send


class Handler(BaseHTTPRequestHandler):
    server_version = "BilingualAccount"

    def log_message(self, *_args):
        pass  # Never log OTPs, tokens, addresses or request bodies.

    def do_GET(self):
        self.handle_api()

    def do_POST(self):
        self.handle_api()

    def handle_api(self):
        try:
            body = {}
            if self.command == "POST":
                try:
                    length = int(self.headers.get("Content-Length", "0"))
                    require(0 < length <= 1_000_000)
                    require(self.headers.get_content_type() == "application/json")
                    body = json.loads(self.rfile.read(length))
                except (ValueError, UnicodeDecodeError):
                    raise APIError(400, "invalid_request") from None
            token = self.headers.get("Authorization", "").removeprefix("Bearer ")
            store = self.server.store
            ip = self.client_address[0]
            # Only a loopback reverse proxy may supply the original IP.
            if ipaddress.ip_address(ip).is_loopback:
                forwarded = (
                    self.headers.get("X-Forwarded-For", "").split(",")[0].strip()
                )
                try:
                    ip = str(ipaddress.ip_address(forwarded))
                except ValueError:
                    pass
            route = (self.command, self.path)
            if route == ("GET", "/health"):
                value = {"ok": True}
            elif route == ("POST", "/v1/events"):
                value = store.events(token, body)
            elif route == ("GET", "/v1/words"):
                value = store.words(token)
            else:
                require(isinstance(body, dict))
                if route == ("POST", "/v1/auth/code"):
                    value = store.send_code(body.get("email"), ip)
                elif route == ("POST", "/v1/auth/verify"):
                    value = store.verify(body.get("email"), body.get("code"))
                elif route == ("POST", "/v1/auth/refresh"):
                    value = store.refresh(body.get("refresh_token"))
                elif route == ("POST", "/v1/auth/logout"):
                    value = store.logout(token, body.get("refresh_token"))
                else:
                    raise APIError(404, "not_found")
            status = 200
        except APIError as error:
            status, value = error.status, {"error": error.message}
        except (OSError, sqlite3.Error, TimeoutError):
            status, value = 503, {"error": "service_unavailable"}
        data = json.dumps(value, ensure_ascii=False).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)


class Server(ThreadingHTTPServer):
    daemon_threads = True

    def get_request(self):
        connection, address = super().get_request()
        connection.settimeout(20)
        return connection, address


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=9057)
    parser.add_argument("--state", type=Path, default=Path(__file__).parent / "state")
    args = parser.parse_args()
    try:
        sender = smtp_sender()
    except APIError:
        parser.exit(
            1, "请先通过环境变量配置安全 SMTP 和发件地址，参见 backend/README.md。\n"
        )
    store = Store(args.state, sender)
    server = Server(("127.0.0.1", args.port), Handler)
    server.store = store
    print("账号服务已启动；仅监听本机，请通过 HTTPS 反向代理开放。", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
        store.close()


if __name__ == "__main__":
    main()
