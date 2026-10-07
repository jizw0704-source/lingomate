"""Install only this locally built app; keep a receipt and any prior version."""

import datetime
import json
import plistlib
import shutil
import subprocess
from pathlib import Path

NATIVE = Path(__file__).resolve().parents[1]
RESEARCH = NATIVE.parent
SOURCE = NATIVE / "build/BilingualCompanion.app"
DESTINATION = Path.home() / "Library/Input Methods/BilingualCompanion.app"
IDENTIFIER = "org.local.bilingualcompanion"
RELATIVE_BINARY = "Contents/MacOS/BilingualCompanion"


def bundle_id(path):
    with (path / "Contents/Info.plist").open("rb") as stream:
        return plistlib.load(stream).get("CFBundleIdentifier")


def main():
    if bundle_id(SOURCE) != IDENTIFIER:
        raise SystemExit("Unexpected source app; refusing installation.")
    subprocess.run(["codesign", "--verify", "--deep", "--strict", SOURCE], check=True)
    if DESTINATION.exists() and bundle_id(DESTINATION) != IDENTIFIER:
        raise SystemExit("Unrelated destination app; refusing to overwrite.")
    sources = json.loads(
        subprocess.check_output([SOURCE / RELATIVE_BINARY, "--sources"], text=True)
    )
    previous = next((item["id"] for item in sources if item["current"]), None)
    stamp = datetime.datetime.now(datetime.UTC).strftime("%Y%m%dT%H%M%SZ")
    backup = None
    if DESTINATION.exists():
        backup = (
            RESEARCH
            / "evidence/native-backups"
            / stamp
            / (DESTINATION.name + ".disabled")
        )
        backup.parent.mkdir(parents=True, exist_ok=False)
        shutil.move(DESTINATION, backup)
    DESTINATION.parent.mkdir(parents=True, exist_ok=True)
    try:
        subprocess.run(
            ["ditto", "--noextattr", "--norsrc", "--noacl", SOURCE, DESTINATION],
            check=True,
        )
        subprocess.run(
            ["codesign", "--verify", "--deep", "--strict", DESTINATION], check=True
        )
    except Exception:
        if DESTINATION.exists():
            failed = RESEARCH / "evidence/native-failed" / stamp / DESTINATION.name
            failed.parent.mkdir(parents=True, exist_ok=True)
            shutil.move(DESTINATION, failed)
        if backup:
            shutil.move(backup, DESTINATION)
        raise
    receipt = {
        "installedAt": stamp,
        "destination": str(DESTINATION),
        "previousInputSource": previous,
        "backup": str(backup) if backup else None,
        "sourceSelected": False,
    }
    receipt_path = RESEARCH / "evidence/native-install.json"
    receipt_path.parent.mkdir(parents=True, exist_ok=True)
    receipt_path.write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + "\n")
    print(f"已安装：{DESTINATION}")
    print("安装未切换输入源；下一步运行已安装程序的 --register。")


if __name__ == "__main__":
    main()
