# Self-hosted accounts

- Python 3.11+ standard library, no production dependencies. Keep native JSON contracts in sync; preserve SQLite state and never commit state, email credentials, OTPs or tokens.
- Only email OTP is implemented. Never simulate real email delivery or online sign-in; tests inject an isolated sender and use temporary storage.
- Bind loopback behind a HTTPS reverse proxy. Keep TLS certificate verification, rate limits, single-use OTPs, hashed tokens, authenticated user scoping and event idempotence.

From repository root:

```sh
runtime/python/bin/ruff format --check backend
runtime/python/bin/ruff check backend
runtime/python/bin/python -m unittest discover -s backend -v
runtime/python/bin/python -m compileall -q backend
runtime/python/bin/python backend/server.py --port 9057
```

Run requires SMTP environment configuration; see README.md. Public deployment and real mail need separate acceptance, not a green isolated test.
