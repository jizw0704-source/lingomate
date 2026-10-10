"""Prepare a local Mac update ZIP and metadata, without publishing or installing."""

import argparse
import hashlib
import json
import os
import plistlib
import re
import subprocess
from pathlib import Path

NATIVE = Path(__file__).resolve().parents[1]


def prepare(app: Path, output_root: Path, notes: str) -> Path:
    if app.is_symlink() or not app.is_dir() or app.name != "BilingualCompanion.app":
        raise ValueError("Application must be a real directory.")
    info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
    if info.get("CFBundleIdentifier") != "org.local.bilingualcompanion":
        raise ValueError("Not a LingoMate application.")
    version = info["CFBundleShortVersionString"]
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version):
        raise ValueError("Invalid version.")
    if len(notes.encode()) > 8000:
        raise ValueError("Release notes must be <= 8000 UTF-8 bytes.")
    environment = {**os.environ, "DEVELOPER_DIR": "/Library/Developer/CommandLineTools"}
    subprocess.run(
        ["/usr/bin/codesign", "--verify", "--deep", "--strict", str(app)],
        env=environment,
        check=True,
    )
    architecture = subprocess.check_output(
        ["/usr/bin/lipo", "-archs", str(app / "Contents/MacOS/BilingualCompanion")],
        env=environment,
        text=True,
    ).strip()
    if architecture != "arm64":
        raise ValueError("This updater requires the Apple Silicon build.")
    # A generated file never publishes itself. An unsigned preview cannot activate updates.
    assessed = (
        subprocess.run(
            ["/usr/sbin/spctl", "--assess", "--type", "execute", str(app)],
            env=environment,
            capture_output=True,
            timeout=120,
            check=False,
        ).returncode
        == 0
    )
    output = output_root / f"release-{version}"
    output.mkdir(parents=True, exist_ok=False)
    name = f"lingomate-macos-arm64-{version}.zip"
    archive = output / name
    subprocess.run(
        [
            "/usr/bin/ditto",
            "-c",
            "-k",
            "--norsrc",
            "--noextattr",
            "--noacl",
            "--keepParent",
            str(app),
            str(archive),
        ],
        env=environment,
        check=True,
        timeout=120,
    )
    with archive.open("rb") as handle:
        digest = hashlib.file_digest(handle, "sha256").hexdigest()
    (output / (name + ".sha256")).write_text(f"{digest}  {name}\n")
    metadata = {
        "schema": 1,
        "platform": "macos",
        "arch": "arm64",
        "channel": "preview",
        "minimum_macos_major": int(info["LSMinimumSystemVersion"].split(".")[0]),
        "status": "published" if assessed else "unpublished",
        "version": version,
        "asset": name,
        "size": archive.stat().st_size,
        "sha256": digest,
        "download_url": (
            "https://github.com/jizw0704-source/lingomate/releases/download/"
            f"macos-v{version}/{name}"
        ),
        "notes": notes,
    }
    (output / "lingomate-macos-update.json").write_text(
        json.dumps(metadata, ensure_ascii=False, indent=2) + "\n"
    )
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--app", type=Path, default=NATIVE / "build/BilingualCompanion.app"
    )
    parser.add_argument("--output-root", type=Path, default=NATIVE / "build/updates")
    parser.add_argument("--notes-file", type=Path)
    args = parser.parse_args()
    notes = args.notes_file.read_text() if args.notes_file else "灵果软件更新。"
    output = prepare(args.app.absolute(), args.output_root, notes)
    print(f"Prepared local update files: {output}")
    print("Not uploaded or published. Ad-hoc builds retain unpublished status.")
    print(
        "Release signing, notarization, data rights and real-device acceptance remain required."
    )


if __name__ == "__main__":
    main()
