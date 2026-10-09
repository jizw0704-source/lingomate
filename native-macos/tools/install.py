"""Install only this locally built app; keep a receipt and any prior version."""

import datetime
import json
import plistlib
import shutil
import subprocess
import tempfile
import zipfile
from pathlib import Path

NATIVE = Path(__file__).resolve().parents[1]
RESEARCH = NATIVE.parent
SOURCE = NATIVE / "build/BilingualCompanion.app"
DESTINATION = Path.home() / "Library/Input Methods/BilingualCompanion.app"
IDENTIFIER = "org.local.bilingualcompanion"
RELATIVE_BINARY = "Contents/MacOS/BilingualCompanion"
LSREGISTER = Path(
    "/System/Library/Frameworks/CoreServices.framework/Frameworks/"
    "LaunchServices.framework/Support/lsregister"
)


def register_app(path, *, remove=False):
    # Only the explicit app path; never reset the system Launch Services database.
    subprocess.run([LSREGISTER, "-u" if remove else "-f", path], check=True)


def archive_app(path, archive):
    subprocess.run(
        ["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", path, archive],
        check=True,
    )
    with zipfile.ZipFile(archive) as saved:
        if saved.testzip() is not None:
            raise RuntimeError("Backup verification failed; original app retained.")
        original = (path / "Contents/Info.plist").read_bytes()
        if saved.read(path.name + "/Contents/Info.plist") != original:
            raise RuntimeError("Backup metadata differs; original app retained.")
        if (
            saved.read(path.name + "/" + RELATIVE_BINARY)
            != (path / RELATIVE_BINARY).read_bytes()
        ):
            raise RuntimeError("Backup executable differs; original app retained.")


def ensure_stopped():
    rows = subprocess.check_output(["ps", "-axo", "command"], text=True)
    binary = str(DESTINATION / RELATIVE_BINARY)
    if any(
        line == binary or line.startswith(binary + " ") for line in rows.splitlines()
    ):
        raise SystemExit(
            "Input method is still running; stop it safely before installation."
        )


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
    DESTINATION.parent.mkdir(parents=True, exist_ok=True)
    ensure_stopped()
    stamp = datetime.datetime.now(datetime.UTC).strftime("%Y%m%dT%H%M%SZ")
    backup = None
    # Build and verify a complete replacement before touching the installed app.
    with tempfile.TemporaryDirectory(
        prefix=".bilingual-install-", dir=DESTINATION.parent
    ) as temporary:
        stage = Path(temporary) / DESTINATION.name
        rollback = Path(temporary) / "previous.app"
        subprocess.run(
            ["ditto", "--noextattr", "--norsrc", "--noacl", SOURCE, stage],
            check=True,
        )
        subprocess.run(
            ["codesign", "--verify", "--deep", "--strict", stage], check=True
        )
        if DESTINATION.exists():
            backup = (
                RESEARCH / "evidence/native-backups" / stamp / "BilingualCompanion.zip"
            )
            backup.parent.mkdir(parents=True, exist_ok=False)
            archive_app(DESTINATION, backup)
        ensure_stopped()
        try:
            if DESTINATION.exists():
                register_app(DESTINATION, remove=True)
                DESTINATION.rename(rollback)
            stage.rename(DESTINATION)
            subprocess.run(
                ["codesign", "--verify", "--deep", "--strict", DESTINATION], check=True
            )
            register_app(DESTINATION)
            # The development bundle must not compete with the installed input source.
            register_app(SOURCE, remove=True)
        except Exception:
            if DESTINATION.exists() and not stage.exists():
                subprocess.run([LSREGISTER, "-u", DESTINATION], check=False)
                shutil.rmtree(DESTINATION)
            if rollback.exists():
                rollback.rename(DESTINATION)
                register_app(DESTINATION)
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
