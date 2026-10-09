"""Mirror project documents into an existing vault, preserving manual changes."""

import argparse
import hashlib
import json
import re
from pathlib import Path

PROJECT = Path(__file__).resolve().parents[1]
MARKER = "<!-- bilingual-ime-managed-v1 -->"
REPOSITORY = "https://github.com/jizw0704-source/lingomate"
DOCUMENTS = {
    "使用指南.md": "native-macos/README.md",
    "开发进展.md": "docs/PROGRESS.md",
    "验证记录.md": "native-macos/QA.md",
    "安装登记与重载.md": "native-macos/INSTALLATION.md",
    "历史调研.md": "RESEARCH.md",
    "第三方来源.md": "docs/THIRD_PARTY.md",
}
LINKS = {
    "README.md": "项目总览",
    "RESEARCH.md": "历史调研",
    "native-macos/README.md": "使用指南",
    "native-macos/QA.md": "验证记录",
    "native-macos/INSTALLATION.md": "安装登记与重载",
    "docs/PROGRESS.md": "开发进展",
    "docs/THIRD_PARTY.md": "第三方来源",
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def mirrored(source):
    path = PROJECT / source

    def link(match):
        label, target = match.groups()
        if re.match(r"[a-zA-Z]+:", target) or target.startswith("#"):
            return match.group(0)
        local = (path.parent / target.split("#")[0]).resolve()
        try:
            relative = local.relative_to(PROJECT).as_posix()
        except ValueError:
            return match.group(0)
        if relative in LINKS:
            return f"[[{LINKS[relative]}|{label}]]"
        return f"[{label}]({REPOSITORY}/blob/main/{relative})"

    text = re.sub(r"\[([^\]]+)\]\(([^)]+)\)", link, path.read_text())
    return f"{MARKER}\n\n{text}".encode()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--vault", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    vault = args.vault.expanduser().resolve()
    if not (vault / ".obsidian").is_dir():
        parser.error("Target must be an existing Obsidian vault.")
    destination = vault / "10 项目/中英输入法"
    manifest = destination / ".bilingual-ime-sync.json"
    previous = json.loads(manifest.read_text()) if manifest.exists() else {}
    files = {name: mirrored(source) for name, source in DOCUMENTS.items()}
    overview = f"""{MARKER}

# 灵果 · LingoMate · 项目总览

原生版本：0.11.7。更新：2026-10-09。

中文拼音输入时同时选择中英文，支持词语多译法展开、本机整句翻译、候选分页、本地选词记忆、中英文标点和Shift英文直输切换。

0.11.7增加中文夹英文：空格选中文、回车输出原样字母并保留大小写、Shift＋空格选译文；展开译法时回车也输出原样字母，提交后继续中文拼音。无组合时回车交给宿主换行/发送。原样字母不记录为学习单词。仓库命名统一为lingomate，源码处于早期预览阶段，暂不发布正式安装包。

0.11.6将中文软件显示名称统一为“灵果”，英文为“LingoMate”。原生输入源、窗口/提示、网页原型和登录邮件标题同步；内部标识与数据位置不变，不迁移或清空词库及配置。已安装本机新版，系统回读名称“灵果”，签名与构建/安装哈希通过；新版实体键盘及跨应用稳定性仍待用户试用。

0.11.5增加正常服务启动互斥，重复启动不再运行引擎/争用连接，退出或崩溃自动释放。安装流程整理本项目开发包和安装包登记。原生空白文稿实体键盘对照正常，仅切换应用未恢复Codex；用户随后确认重启Codex后恢复使用。项目记录也收到微信、钉钉各自重启后恢复反馈。具体根因、逐项输入验收及长期/反复切换稳定性仍待验证，不把重启恢复等同于永久修复。

2026-10-09微信/钉钉连接排查：苹果拼音在微信同一输入框正常，实验输入法无按键到达；25份旧备份已校验压缩保留并注销旧副本，当前仅正式安装路径登记。微信和钉钉本轮实体键盘验收仍待回复。详见[[安装登记与重载]]。

0.11.4取消候选旁的“记／记忆”标记与专用留白，保留本机选词记忆和优先排序。

0.11.3短词候选改为五项横排，中文在上、主要英文在下；顶部集中页码、翻页、译法与齿轮设置，其他译法按需展开。长句保留换行，44pt按钮和原快捷键保留。已更新本机试用包，实体键盘与宿主焦点仍待本版试用。

0.11.2补充每次会话激活的ABC字母布局覆盖和无文字诊断计数；用户在0.11.2实际键盘重试后确认中文空格提交与Shift切换后的英文直输均正常；未提供宿主名称，长期/跨应用稳定性与故障根因仍待验证。

0.11增加统一设置：候选窗或系统菜单 → 设置，在同一窗口管理登录与学习、翻译、输入和外观；账号服务仍待部署，输入选项按本次运行生效，外观保存。

0.10.1修正正常输入服务的禁止窗口模式，保留不抢焦点的候选面板；加入不记录文字的会话/事件/窗口状态和隔离窗口检查。已安装重载，2026-10-08用户在钉钉重试后确认“现在可以了”，此次无响应问题恢复；中英文分别提交、切换、标点及跨应用完整流程仍待逐项验证。

0.10增加可选MiniMax在线翻译：输入源菜单 → AI 翻译设置，选国内/国际预设，自己填对应密钥并启用；默认本机，只发当前较长中文候选。密钥在系统钥匙串，不镜像。真实API速度/质量及Keychain授权待用户配置验收。

0.9进一步压缩候选窗并加入学习入口、自建邮箱验证码账号和单词学习页。按当前决定先准备部署说明，尚未部署公网或连接发信邮箱，不能真实在线登录；不使用假账号。登录后只同步所选词表英文、对应中文、词性、次数和手动掌握状态，整句、拼音和个人词库不上传。

0.8新增白色、黑色和跟随系统，候选窗齿轮 → 外观或输入源菜单选择，本机记住选择；保留正在输入的拼音和候选状态。

0.7整理候选窗层次，中英文分列；更多译法和整句译文紧随当前候选，长句换行，窄面板上下排列，超高内容滚动。

- [GitHub 项目仓库]({REPOSITORY})
- [[使用指南]]：账号与学习记录、自建服务部署说明、外观切换、英文直输、标点切换、选词记忆、翻页、中文/英文提交、模型准备和开发命令。
- [[开发进展]]：已实现功能、验证范围及下一步试用。
- [[验证记录]]：通过的检查与真实键盘等待验收项。
- [[历史调研]]：开源来源、固定提交和第一步离线研究。
- [[第三方来源]]：GPL许可、上游数据来源和正式分发待核对项。

每页5项，PageUp/PageDown或减号/等号翻页，数字1–5选择当前页；空格中文，Shift＋空格英文。

确认选过的中文词下次优先显示，不额外标注；词库只保存在本机，不镜像到本知识库或GitHub。

源码以GitHub为主，这些页面是本轮按需镜像，没有后台同步。手写笔记请放在单独文件；脚本检测到镜像被修改时会停止并保留它。
"""
    files["项目总览.md"] = overview.encode()
    for name, desired in files.items():
        existing = destination / name
        if not existing.exists():
            if args.check:
                raise SystemExit(f"Missing mirror: {name}")
            continue
        data = existing.read_bytes()
        recorded = previous.get(name)
        if MARKER.encode() not in data or (
            digest(data) != recorded and data != desired
        ):
            raise SystemExit(f"Manual changes preserved; refusing to overwrite: {name}")
        if args.check and data != desired:
            raise SystemExit(f"Outdated mirror: {name}")
    if args.check:
        print(f"Verified {len(files)} project notes; source and vault match.")
        return
    destination.mkdir(parents=True, exist_ok=True)
    for name, data in files.items():
        target = destination / name
        temporary = target.with_suffix(".md.tmp")
        temporary.write_bytes(data)
        temporary.replace(target)
    manifest.write_text(
        json.dumps({name: digest(data) for name, data in files.items()}, indent=2)
        + "\n"
    )
    print(f"Synced {len(files)} project notes: {destination}")


if __name__ == "__main__":
    main()
