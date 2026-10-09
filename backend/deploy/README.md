# 邮箱登录服务部署包

这是可部署的 Linux/systemd 配置，尚未部署公网。需要服务器、账号服务域名和支持 TLS 的真实 SMTP 发信账号；这些信息尚未提供。配置文件中的域名和邮箱是占位符。不要把密码交给聊天或写入 Git。

## 安装

服务器需要 Python 3.11+、systemd 和 Caddy，域名 A/AAAA 记录应指向服务器，80/443 应可达。不要开放9057端口。Caddy 自动 HTTPS 的条件见[官方说明](https://caddyserver.com/docs/automatic-https)。只适合当前小规模试用；尚未做公网负载验收。

在服务器上，把项目的 `backend/` 放到一个准备目录，再以管理员身份运行下面的安装步骤。路径按服务器目录调整。不要在当前 Mac 上执行这些 Linux 命令。

```sh
install -d -m 0755 /opt/bilingual-account
install -m 0644 backend/server.py /opt/bilingual-account/server.py
install -d -m 0700 /etc/bilingual-account
install -m 0600 backend/deploy/smtp.env.example /etc/bilingual-account/smtp.env
install -m 0644 backend/deploy/bilingual-account.service /etc/systemd/system/bilingual-account.service
```

只在服务器本地编辑 `/etc/bilingual-account/smtp.env`，填入真实发信配置。将 `account.caddy` 中的域名替换为已配置 DNS 的域名，将该域名块加入服务器已有 Caddy 配置；保留其他站点，不覆盖已有配置，也不要启用请求正文/账号访问日志。若使用代理前的 CDN，限流看到的是 CDN 地址，需要另外配置受信任代理，当前模板只用于直接连接 Caddy。

```sh
python3 --version
systemd-analyze verify /etc/systemd/system/bilingual-account.service
caddy validate --config /etc/caddy/Caddyfile
systemctl daemon-reload
systemctl enable --now bilingual-account.service
systemctl reload caddy
curl --fail --silent --show-error https://你的账号域名/health
```

健康检查预期 `{"ok": true}`；它不能证明邮件送达或登录成功。服务自动重启但连续启动失败会停止重试，修正配置后用 `systemctl reset-failed bilingual-account` 和 `systemctl restart bilingual-account` 恢复。

## 接入输入法及验收

设置 → 账号与学习 → 高级设置，填写 `https://你的账号域名` 后保存。用自己控制的邮箱发送验证码、查看真实收信、填写六位码登录；不要把验证码发到聊天里。再选择英文词语、标记掌握，使用另一台设备验证同步和另一账号的数据隔离。实际收信、登录、钥匙串授权和跨设备均需要单独验收，目前未完成。

发信服务很慢时不会锁住全部账号的数据库操作；失败的发信请求也计入额度，同一邮箱再次发信至少间隔60秒。发信失败显示服务不可用，不会创建假验证码或假登录。

## 更新、备份与回退

SQLite 数据和验证码摘要密钥在 `/var/lib/bilingual-account/`（systemd DynamicUser 管理的私有状态目录）。停服务后备份整个状态目录，包含 `accounts.sqlite3` 和 `otp-key`，权限保持私有。不要只备份程序或把状态同步到 Obsidian。

更新前保留旧 `server.py`，只替换程序并重启服务，保留状态目录。失败时恢复旧程序并重启。停用用 `systemctl disable --now bilingual-account`；不要删除状态目录。备份和恢复还未在真实服务器验收。
