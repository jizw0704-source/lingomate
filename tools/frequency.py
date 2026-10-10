"""Reproduce word counts from the official, checksum-pinned Google Ngram export."""

import gzip
import hashlib
import io
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MAX_BYTES = 80 * 1024 * 1024
TERMS = "LicenseRef-Google-Books-Ngram"


def verified_source(config, override=None):
    if (
        config["license"] != TERMS
        or config["terms_url"] != "https://books.google.com/ngrams/info"
    ):
        raise ValueError("Unrecognized frequency permission statement")
    expected = "https://storage.googleapis.com/books/ngrams/books/20200217/chi_sim/1-00000-of-00001.gz"
    if config["snapshot_url"] != expected:
        raise ValueError("Frequency source must be the pinned official export")
    path = override or ROOT / "data/sources/google-chi-sim-20200217.gz"
    if path.exists():
        if path.stat().st_size > MAX_BYTES:
            raise ValueError("Frequency source exceeds size bound")
        content = path.read_bytes()
    else:
        if override:
            raise ValueError("Explicit frequency source does not exist")
        request = urllib.request.Request(
            expected, headers={"User-Agent": "LingoMate-data-builder"}
        )
        with urllib.request.urlopen(request, timeout=90) as response:
            if response.url != expected:
                raise ValueError("Unexpected frequency download destination")
            content = response.read(MAX_BYTES + 1)
    if len(content) != config["source_bytes"] or len(content) > MAX_BYTES:
        raise ValueError("Pinned frequency size mismatch")
    if hashlib.sha256(content).hexdigest() != config["source_sha256"]:
        raise ValueError("Pinned frequency checksum mismatch; existing files preserved")
    if not override and not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
    return content


def count_words(content, words, year_start, year_end):
    if not 1800 <= year_start <= year_end <= 2019:
        raise ValueError("Invalid frequency publication-year range")
    counts = {}
    rows = 0
    with gzip.GzipFile(fileobj=io.BytesIO(content)) as archive:
        for raw in archive:
            rows += 1
            fields = raw.decode("utf-8").rstrip("\r\n").split("\t")
            word = fields[0]
            # POS tags, spaces, numbers and words outside CC-CEDICT are not imported.
            if word not in words:
                continue
            count = 0
            seen = set()
            for field in fields[1:]:
                parts = field.split(",")
                if len(parts) != 3:
                    raise ValueError("Malformed frequency observation")
                year, matches, volumes = map(int, parts)
                if year in seen or matches < 0 or volumes < 0 or volumes > matches:
                    raise ValueError("Invalid or repeated frequency observation")
                seen.add(year)
                if year_start <= year <= year_end:
                    count += matches
            if count:
                if word in counts:
                    raise ValueError("Duplicate frequency word")
                counts[word] = count
    if not counts:
        raise ValueError("No frequency observations matched the dictionary")
    return counts, {
        "source_rows": rows,
        "observed_words": len(counts),
        "match_total": sum(counts.values()),
    }
