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
    "Windows使用指南.md": "native-windows/README.md",
    "Windows验证记录.md": "native-windows/QA.md",
    "使用指南.md": "native-macos/README.md",
    "开发进展.md": "docs/PROGRESS.md",
    "验证记录.md": "native-macos/QA.md",
    "安装登记与重载.md": "native-macos/INSTALLATION.md",
    "历史调研.md": "RESEARCH.md",
    "第三方来源.md": "docs/THIRD_PARTY.md",
}
LINKS = {
    "native-windows/README.md": "Windows使用指南",
    "native-windows/QA.md": "Windows验证记录",
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
    files["项目总览.md"] = mirrored("README.md")
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
