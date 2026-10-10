"""Build CC-CEDICT release data with verified official Google Ngram weights.

Uses only Python's standard library. Never reads legacy tables or personal data.
"""

import argparse
import hashlib
import json
import os
import re
import tempfile
import urllib.request
from collections import defaultdict
from pathlib import Path

import frequency

ROOT = Path(__file__).resolve().parents[1]
LICENSE_URL = "https://creativecommons.org/licenses/by-sa/4.0/"
LINE = re.compile(r"^(\S+) (\S+) \[([^\]]+)\] /(.+)/$")
SYLLABLE = re.compile(r"^([a-zü:]+)[1-5]$", re.IGNORECASE)
MAX_BYTES = 20 * 1024 * 1024


def digest(content):
    return hashlib.sha256(content).hexdigest()


def han(character):
    number = ord(character)
    return (
        0x3400 <= number <= 0x4DBF
        or 0x4E00 <= number <= 0x9FFF
        or 0x20000 <= number <= 0x323AF
    )


def normalize_pinyin(raw):
    result = []
    for token in raw.split():
        match = SYLLABLE.fullmatch(token)
        if match is None:
            return None
        syllable = match[1].lower().replace("u:", "v").replace("ü", "v")
        if syllable == "xx" or ":" in syllable:
            return None
        # Preserve an independently typable syllable for 儿化 entries.
        result.append("er" if syllable == "r" else syllable)
    return " ".join(result) or None


def sense_fields(raw):
    """Keep whole source senses; omit references/metadata unsuitable for output."""
    fields = []
    for sense in raw.split("/"):
        sense = sense.strip()
        if (
            not sense
            or any(han(c) for c in sense)
            or any(c in sense for c in "|\t\r\n[]")
            or sense.startswith(("CL:", "variant of ", "old variant of "))
        ):
            continue
        # Semicolon-separated glosses remain one sense (do not invent POS).
        if sense not in fields:
            fields.append(sense)
    return fields


def convert(content, ranking, frequencies=None):
    text = content.decode("utf-8")
    if f"#! license={LICENSE_URL}" not in text or "# CC-CEDICT" not in text:
        raise ValueError("Source must explicitly carry CC-CEDICT CC BY-SA 4.0")
    slots = {}
    senses = defaultdict(list)
    name_senses = defaultdict(list)
    provenance = defaultdict(list)
    skipped = 0
    entries = 0
    header = []
    frequencies = frequencies or {}
    maximum = max(frequencies.values(), default=1)
    for number, line in enumerate(text.splitlines(), 1):
        if line.startswith("#"):
            header.append(line)
            continue
        if not line.strip():
            continue
        entries += 1
        match = LINE.fullmatch(line)
        if match is None:
            raise ValueError(f"Unrecognized source syntax at line {number}")
        _, word, raw_pinyin, raw_senses = match.groups()
        pinyin = normalize_pinyin(raw_pinyin)
        if not pinyin or not all(han(c) for c in word) or len(word) > 32:
            skipped += 1
            continue
        fields = sense_fields(raw_senses)
        # Corpus counts provide relative weights. Existing independently authored
        # everyday priors offset book-domain bias; they are not corpus counts.
        default = 1 if raw_pinyin[0].isupper() else 10
        if word in frequencies:
            weight = max(2, (frequencies[word] * 1_000_000 + maximum // 2) // maximum)
            if raw_pinyin[0].isupper():
                weight = max(1, weight // 4)
        else:
            weight = default
        weight = min(1_000_000, weight + ranking.get(word, 0))
        key = (word, pinyin)
        slots[key] = max(slots.get(key, 0), weight)
        provenance[key].append(number)
        destination = name_senses if raw_pinyin[0].isupper() else senses
        for field in fields:
            if field not in destination[word]:
                destination[word].append(field)
    # Show everyday meanings before surnames without changing source meanings.
    for word, fields in name_senses.items():
        for field in fields:
            if field not in senses[word]:
                senses[word].append(field)
    unknown = set(ranking) - {word for word, _ in slots}
    if unknown:
        raise ValueError(f"Ranking words absent from source: {sorted(unknown)}")
    declared = next(l for l in header if l.startswith("#! entries="))
    if entries != int(declared.split("=", 1)[1]):
        raise ValueError("Source entry count differs from its header")
    notice = "# CC-CEDICT / MDBG and contributors; adapted by LingoMate\n"
    notice += f"# SPDX-License-Identifier: CC-BY-SA-4.0; {LICENSE_URL}\n"
    dictionary = (
        notice
        + "# Simplified word, normalized Pinyin, scaled frequency plus independent prior weight\n"
    )
    dictionary += "".join(
        f"{word}\t{pinyin}\t{slots[word, pinyin]}\n" for word, pinyin in sorted(slots)
    )
    glossary = notice + "# Whole English senses; reference-only metadata omitted\n"
    glossary += "".join(
        word + "\t" + "\t".join(senses[word]) + "\n"
        for word in sorted(senses)
        if senses[word]
    )
    records = "".join(
        json.dumps(
            {
                "word": word,
                "pinyin": pinyin,
                "source_lines": provenance[word, pinyin],
                "frequency_match_count": frequencies.get(word, 0),
                "independent_prior": ranking.get(word, 0),
                "ranking_weight": slots[word, pinyin],
            },
            ensure_ascii=False,
            separators=(",", ":"),
        )
        + "\n"
        for word, pinyin in sorted(slots)
    )
    return {
        "dict.tsv": dictionary.encode(),
        "glossary-en.tsv": glossary.encode(),
        "provenance.jsonl": records.encode(),
    }, {
        "source_entries": entries,
        "dictionary_readings": len(slots),
        "dictionary_words": len({word for word, _ in slots}),
        "glossary_words": sum(bool(s) for s in senses.values()),
        "filtered_source_entries": skipped,
        "source_header": header,
    }


def verified_source(config, override=None):
    path = override or ROOT / "data/sources/cedict.u8"
    if path.exists():
        content = path.read_bytes()
    else:
        if override:
            raise ValueError("Explicit source path does not exist")
        request = urllib.request.Request(
            config["snapshot_url"], headers={"User-Agent": "LingoMate-data-builder"}
        )
        with urllib.request.urlopen(request, timeout=30) as response:
            if not response.url.startswith("https://raw.githubusercontent.com/"):
                raise ValueError("Unexpected source download destination")
            content = response.read(MAX_BYTES + 1)
        if len(content) > MAX_BYTES:
            raise ValueError("Source exceeds download bound")
    if digest(content) != config["source_sha256"]:
        raise ValueError("Pinned source checksum mismatch; existing files preserved")
    if not override and not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
    return content


def verify_package(package, files):
    for name in (
        "dict.tsv",
        "glossary-en.tsv",
        "LEXICON-NOTICE.md",
        "GLOSSARY-NOTICE.md",
    ):
        if (
            not (package / name).is_file()
            or (package / name).read_bytes() != files[name]
        ):
            raise ValueError(
                f"Package contains stale or unexpected data/notice: {name}"
            )


def prepare(source=None, check=False, package=None, frequency_source=None):
    config = json.loads((ROOT / "data/source.json").read_text(encoding="utf-8"))
    ranking_bytes = (ROOT / "data/ranking.json").read_bytes()
    ranking = json.loads(ranking_bytes)
    if any(type(w) is not int or not 0 < w <= 1_000_000 for w in ranking.values()):
        raise ValueError("Invalid ranking weight")
    content = verified_source(config, source)
    initial, _ = convert(content, ranking)
    words = {
        line.split("\t", 1)[0]
        for line in initial["dict.tsv"].decode().splitlines()
        if not line.startswith("#")
    }
    frequency_config = json.loads(
        (ROOT / "data/frequency-source.json").read_text(encoding="utf-8")
    )
    counts, frequency_stats = frequency.count_words(
        frequency.verified_source(frequency_config, frequency_source),
        words,
        frequency_config["year_start"],
        frequency_config["year_end"],
    )
    files, stats = convert(content, ranking, counts)
    summary = {
        "schema": 2,
        "license": "CC-BY-SA-4.0",
        "source": config,
        "frequency_source": frequency_config,
        "frequency_stats": frequency_stats,
        "ranking_sha256": digest(ranking_bytes),
        **stats,
        "files": {name: digest(value) for name, value in files.items()},
    }
    files["data-manifest.json"] = (
        json.dumps(summary, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode()
    attribution = (ROOT / "docs/licenses/CC-CEDICT-NOTICE.txt").read_bytes()
    license_text = (ROOT / "docs/licenses/CC-BY-SA-4.0.txt").read_bytes()
    frequency_notice = (
        ROOT / "docs/licenses/GOOGLE-BOOKS-NGRAM-NOTICE.txt"
    ).read_bytes()
    for name in ("LEXICON-NOTICE.md", "GLOSSARY-NOTICE.md"):
        files[name] = (
            attribution
            + b"\nOriginal source header:\n"
            + "\n".join(stats["source_header"]).encode()
            + b"\n\nBuild data manifest:\n"
            + files["data-manifest.json"]
            + b"\n\nFrequency source permission and changes:\n"
            + frequency_notice
            + b"\n\nFull license:\n"
            + license_text
        )
    if package:
        verify_package(package, files)
    output = ROOT / "data/generated"
    if check:
        for name, expected in files.items():
            if (
                not (output / name).is_file()
                or (output / name).read_bytes() != expected
            ):
                raise ValueError(f"Generated data differs: {name}")
    else:
        output.mkdir(parents=True, exist_ok=True)
        # Stage every file before replacement; all paths are project-owned.
        with tempfile.TemporaryDirectory(dir=output) as staging:
            for name, value in files.items():
                (Path(staging) / name).write_bytes(value)
            for name in files:
                os.replace(Path(staging) / name, output / name)
    print(
        f"Verified CC-CEDICT + Google Ngram: {stats['dictionary_readings']} readings, "
        f"{stats['glossary_words']} translated words; CC BY-SA 4.0"
    )
    return summary


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source", type=Path, help="Already downloaded pinned snapshot"
    )
    parser.add_argument(
        "--check", action="store_true", help="Verify without rewriting data"
    )
    parser.add_argument("--package", type=Path, help="Verify packaged data and notices")
    parser.add_argument(
        "--frequency-source",
        type=Path,
        help="Already downloaded pinned official frequency GZip",
    )
    arguments = parser.parse_args()
    try:
        prepare(
            arguments.source,
            arguments.check,
            arguments.package,
            arguments.frequency_source,
        )
    except (ValueError, OSError) as error:
        parser.exit(1, f"Data preparation failed: {error}\n")
