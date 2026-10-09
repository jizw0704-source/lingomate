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

Linux deployment templates: deploy/bilingual-account.service and deploy/account.caddy; install/rollback steps in deploy/README.md. On the actual server verify with `systemd-analyze verify /etc/systemd/system/bilingual-account.service` and `caddy validate --config /etc/caddy/Caddyfile`; health is `curl --fail --silent --show-error https://ACCOUNT_DOMAIN/health`. Never hold the database lock while waiting for SMTP; reserve rate quota first, including failed sends, and make OTP usable only after successful delivery submission.
