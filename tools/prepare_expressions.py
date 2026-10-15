"""Reproduce a separately attributed local fixed-expression completion catalog."""

import argparse
import hashlib
import json
import re
import urllib.request
from pathlib import Path

import prepare_lexicon

ROOT = Path(__file__).resolve().parents[1]
LICENSE = "CC-BY-SA-4.0"
LICENSE_URL = "https://creativecommons.org/licenses/by-sa/4.0/"
SNAPSHOT = (
    "https://zh.wikipedia.org/w/index.php?title=%E7%BB%95%E5%8F%A3%E4%BB%A4"
    "&oldid=94677104&action=raw"
)
MAX_BYTES = 2 * 1024 * 1024


def valid(text, syllables):
    return (
        4 <= len(text) <= 16
        and all(0x3400 <= ord(c) <= 0x9FFF for c in text)
        and len(text) == len(syllables)
        and all(re.fullmatch("[a-z]{1,6}", s) for s in syllables)
    )


def verified_supplement(config):
    if config["snapshot_url"] != SNAPSHOT or config["license"] != LICENSE:
        raise ValueError("Unrecognized fixed-expression source or license")
    path = ROOT / "data/sources/wikipedia-tongue-twisters-94677104.txt"
    if path.exists():
        if path.stat().st_size > MAX_BYTES:
            raise ValueError("Supplement exceeds size bound")
        content = path.read_bytes()
    else:
        request = urllib.request.Request(
            SNAPSHOT, headers={"User-Agent": "LingoMate-data-builder/1.0"}
        )
        with urllib.request.urlopen(request, timeout=30) as response:
            if response.url != SNAPSHOT:
                raise ValueError("Unexpected supplement download destination")
            content = response.read(MAX_BYTES + 1)
    if (
        len(content) > MAX_BYTES
        or len(content) != config["source_bytes"]
        or hashlib.sha256(content).hexdigest() != config["source_sha256"]
    ):
        raise ValueError("Supplement checksum mismatch; existing files preserved")
    if not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
    return content


def compile_catalog(cedict, dictionary, supplement, config):
    if config["license"] != LICENSE or config["license_url"] != LICENSE_URL:
        raise ValueError("Unrecognized expression catalog license")
    if f"#! license={LICENSE_URL}" not in cedict.decode():
        raise ValueError("CC-CEDICT license header required")
    weights = {}
    for line in dictionary.decode().splitlines():
        if not line or line.startswith("#"):
            continue
        text, reading, raw_weight = line.split("\t")
        weight = int(raw_weight)
        if not 1 <= weight <= 1_000_000:
            raise ValueError("Invalid dictionary weight")
        weights[(text, tuple(reading.split()))] = weight
    records = {}
    for line in cedict.decode().splitlines():
        match = prepare_lexicon.LINE.fullmatch(line)
        if not match or "(idiom)" not in match[4]:
            continue
        text = match[2]
        reading = prepare_lexicon.normalize_pinyin(match[3])
        syllables = reading.split() if reading else []
        key = (text, tuple(syllables))
        if valid(text, syllables) and key in weights:
            records[key] = {
                "text": text,
                "syllables": syllables,
                "weight": weights[key],
            }
    supplemental = config["supplement"]
    if supplemental["license"] != LICENSE:
        raise ValueError("Supplement license differs")
    original = supplement.decode()
    for entry in supplemental["entries"]:
        text, syllables = entry["text"], entry["syllables"]
        if not valid(text, syllables) or text not in original:
            raise ValueError("Supplement entry not in verified snapshot")
        records.setdefault(
            (text, tuple(syllables)),
            {"text": text, "syllables": syllables, "weight": 1},
        )
    if not 1 <= len(records) <= 10_000:
        raise ValueError("Expression catalog count exceeds bounds")
    catalog = {
        "version": 1,
        "license": LICENSE,
        "entries": sorted(records.values(), key=lambda e: (e["text"], e["syllables"])),
    }
    encoded = (json.dumps(catalog, ensure_ascii=False, indent=2) + "\n").encode()
    if len(encoded) > MAX_BYTES:
        raise ValueError("Expression catalog exceeds size bound")
    return encoded


def build():
    config = json.loads(
        (ROOT / "data/expressions-source.json").read_text(encoding="utf-8")
    )
    source = json.loads((ROOT / "data/source.json").read_text(encoding="utf-8"))
    cedict = prepare_lexicon.verified_source(source)
    supplement = verified_supplement(config["supplement"])
    dictionary = (ROOT / "data/generated/dict.tsv").read_bytes()
    manifest = json.loads(
        (ROOT / "data/generated/data-manifest.json").read_text(encoding="utf-8")
    )
    if hashlib.sha256(dictionary).hexdigest() != manifest["files"]["dict.tsv"]:
        raise ValueError("Generated dictionary checksum mismatch")
    catalog = compile_catalog(cedict, dictionary, supplement, config)
    notice = (
        "# Fixed expressions — CC BY-SA 4.0\n\n"
        "CC-CEDICT idiom-tagged entries are extracted from the pinned dictionary.\n"
        "Only Han-only 4-16 character entries and their matching readings are used.\n"
        "Weights reuse the verified dictionary; no new corpus frequency is invented.\n\n"
        + (ROOT / "docs/licenses/CC-CEDICT-NOTICE.txt").read_text(encoding="utf-8")
        + "\n\nSupplement: Chinese Wikipedia contributors, 绕口令.\n"
        + config["supplement"]["official_url"]
        + "\nContributor history: "
        + config["supplement"]["history_url"]
        + "\nChanges: first-clause extraction; independently encoded tone-free Pinyin; baseline weight 1.\n"
        + "Both source and adapted text are offered under CC BY-SA 4.0.\n\n"
        + "Source metadata and modifications:\n"
        + json.dumps(
            {"cedict": source, "expressions": config}, ensure_ascii=False, indent=2
        )
        + "\n\nExisting frequency attribution:\n"
        + (ROOT / "docs/licenses/GOOGLE-BOOKS-NGRAM-NOTICE.txt").read_text(
            encoding="utf-8"
        )
        + "\n\nFull license:\n"
        + (ROOT / "docs/licenses/CC-BY-SA-4.0.txt").read_text(encoding="utf-8")
    ).encode()
    return {"fixed-expressions.json": catalog, "EXPRESSIONS-NOTICE.md": notice}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--package", type=Path)
    args = parser.parse_args()
    files = build()
    destination = ROOT / "data/generated"
    for name, content in files.items():
        target = destination / name
        if args.check:
            if not target.is_file() or target.read_bytes() != content:
                raise SystemExit(f"Outdated expression data: {name}")
        else:
            destination.mkdir(parents=True, exist_ok=True)
            temporary = target.with_suffix(target.suffix + ".tmp")
            temporary.write_bytes(content)
            temporary.replace(target)
        if args.package and (args.package / name).read_bytes() != content:
            raise SystemExit(f"Packaged expression data differs: {name}")
    count = len(json.loads(files["fixed-expressions.json"])["entries"])
    print(
        f"Verified fixed expressions: {count}; CC-CEDICT + attributed Wikipedia; CC BY-SA 4.0"
    )


if __name__ == "__main__":
    main()
