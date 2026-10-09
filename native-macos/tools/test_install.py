"""Exercise staged replacement/rollback using temporary bundles and fake registration."""

import json
import plistlib
import subprocess
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch

import install


class InstallationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="bilingual-install-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "build/BilingualCompanion.app"
        self.destination = self.root / "input/BilingualCompanion.app"
        for app, version in [(self.source, b"new"), (self.destination, b"old")]:
            (app / "Contents/MacOS").mkdir(parents=True)
            (app / install.RELATIVE_BINARY).write_bytes(version)
            (app / "Contents/Info.plist").write_bytes(
                plistlib.dumps({"CFBundleIdentifier": install.IDENTIFIER})
            )
        self.calls = []
        self.running = False
        self.fail_stage = False
        self.fail_registration = False
        actual_run = subprocess.run

        def run(args, **kwargs):
            self.calls.append([str(value) for value in args])
            if str(args[0]) == str(install.LSREGISTER):
                if (
                    self.fail_registration
                    and args[1] == "-f"
                    and (self.destination / install.RELATIVE_BINARY).read_bytes()
                    == b"new"
                ):
                    raise subprocess.CalledProcessError(1, args)
                return subprocess.CompletedProcess(args, 0)
            if str(args[0]) == "codesign":
                if self.fail_stage and Path(args[-1]) != self.source:
                    raise subprocess.CalledProcessError(1, args)
                return subprocess.CompletedProcess(args, 0)
            # Real ditto runs only on the synthetic files in this temporary directory.
            return actual_run(args, **kwargs)

        def output(args, **kwargs):
            if args[0] == "ps":
                return (
                    str(self.destination / install.RELATIVE_BINARY)
                    if self.running
                    else ""
                )
            return json.dumps([{"current": True, "id": "test.original.source"}])

        self.addCleanup(patch.stopall)
        for name, value in [
            ("SOURCE", self.source),
            ("DESTINATION", self.destination),
            ("RESEARCH", self.root),
        ]:
            patch.object(install, name, value).start()
        patch.object(install.subprocess, "run", side_effect=run).start()
        patch.object(install.subprocess, "check_output", side_effect=output).start()

    def installed(self):
        return (self.destination / install.RELATIVE_BINARY).read_bytes()

    def test_success_preserves_verified_archive_and_original_source(self):
        install.main()
        self.assertEqual(self.installed(), b"new")
        receipt = json.loads((self.root / "evidence/native-install.json").read_text())
        self.assertEqual(receipt["previousInputSource"], "test.original.source")
        with zipfile.ZipFile(receipt["backup"]) as backup:
            self.assertIsNone(backup.testzip())
            self.assertEqual(
                backup.read("BilingualCompanion.app/" + install.RELATIVE_BINARY), b"old"
            )
        registrations = [
            call[1:] for call in self.calls if call[0] == str(install.LSREGISTER)
        ]
        self.assertEqual(
            registrations,
            [
                ["-u", str(self.destination)],
                ["-f", str(self.destination)],
                ["-u", str(self.source)],
            ],
        )
        self.assertFalse(any(self.destination.parent.glob(".bilingual-install-*")))

    def test_running_service_is_never_overwritten(self):
        self.running = True
        with self.assertRaises(SystemExit):
            install.main()
        self.assertEqual(self.installed(), b"old")
        self.assertFalse(any(call[0] == str(install.LSREGISTER) for call in self.calls))

    def test_failed_staging_leaves_installed_app_and_registration_intact(self):
        self.fail_stage = True
        with self.assertRaises(subprocess.CalledProcessError):
            install.main()
        self.assertEqual(self.installed(), b"old")
        self.assertFalse(any(call[0] == str(install.LSREGISTER) for call in self.calls))

    def test_failed_registration_restores_original_app(self):
        self.fail_registration = True
        with self.assertRaises(subprocess.CalledProcessError):
            install.main()
        self.assertEqual(self.installed(), b"old")
        self.assertFalse((self.root / "evidence/native-install.json").exists())
        self.assertEqual(
            self.calls[-1], [str(install.LSREGISTER), "-f", str(self.destination)]
        )

    def test_unrelated_destination_is_preserved(self):
        info = self.destination / "Contents/Info.plist"
        info.write_bytes(plistlib.dumps({"CFBundleIdentifier": "test.unrelated"}))
        with self.assertRaises(SystemExit):
            install.main()
        self.assertEqual(self.installed(), b"old")
        self.assertFalse(any(call[0] == str(install.LSREGISTER) for call in self.calls))


if __name__ == "__main__":
    unittest.main()
