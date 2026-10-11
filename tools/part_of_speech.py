"""Build display-only POS metadata from checksum-pinned Princeton WordNet 3.0."""

import hashlib
import io
import tarfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
URL = "https://wordnetcode.princeton.edu/3.0/WordNet-3.0.tar.gz"
CATEGORIES = {
    "noun": ("n", "n."),
    "verb": ("v", "v."),
    "adj": ("a", "adj."),
    "adv": ("r", "adv."),
}
ORDER = ["n.", "v.", "adj.", "adv."]


def verified_source(config, override=None):
    if config.get("license") != "WordNet" or config.get("snapshot_url") != URL:
        raise ValueError("Unexpected WordNet source or permission")
    path = override or ROOT / "data/sources/WordNet-3.0.tar.gz"
    if path.exists():
        data = path.read_bytes()
    else:
        if override:
            raise ValueError("Explicit WordNet source does not exist")
        with urllib.request.urlopen(URL, timeout=40) as response:
            if response.url != URL:
                raise ValueError("Unexpected WordNet redirect")
            data = response.read(16 * 1024 * 1024 + 1)
    if (
        len(data) != config["source_bytes"]
        or hashlib.sha256(data).hexdigest() != config["source_sha256"]
    ):
        raise ValueError("WordNet checksum or size mismatch; cached data preserved")
    if not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    return data


def read_index(data, expected_license):
    index = {}
    # Read known regular members directly; never extract archive paths.
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:

        def read(name):
            member = archive.getmember("WordNet-3.0/" + name)
            if not member.isfile() or member.size > 10 * 1024 * 1024:
                raise ValueError("Invalid WordNet member")
            return archive.extractfile(member).read()

        if read("LICENSE") != expected_license:
            raise ValueError("WordNet license differs from the preserved full notice")
        for suffix, (code, label) in CATEGORIES.items():
            for line in read("dict/index." + suffix).decode("ascii").splitlines():
                if not line or line[0].isspace():
                    continue
                fields = line.split()
                if len(fields) < 6 or fields[1] != code or not fields[2].isdigit():
                    raise ValueError("Malformed WordNet index row")
                lemma = fields[0].replace("_", " ").lower()
                index.setdefault(lemma, set()).add(label)
    return index


def label(sense, index):
    """Only whole exact matches; explicit infinitive narrows an attested verb."""
    common = None
    for part in sense.lower().split(";"):
        part = part.strip()
        if part.startswith("to "):
            categories = {"v."} if "v." in index.get(part[3:], set()) else set()
        else:
            categories = index.get(part, set())
        common = set(categories) if common is None else common & categories
        if not common:
            return ""
    return "/".join(p for p in ORDER if p in common)


def annotate(glossary, index, license_text):
    senses = {
        field
        for line in glossary.decode().splitlines()
        if line and not line.startswith("#")
        for field in line.split("\t")[1:]
    }
    metadata = {sense: label(sense, index) for sense in sorted(senses)}
    metadata = {sense: pos for sense, pos in metadata.items() if pos}
    notice = "\n# WordNet 3.0 POS metadata; exact lexical categories, not contextual analysis\n"
    notice += "".join("# " + line + "\n" for line in license_text.decode().splitlines())
    notice += "".join(f"# POS\t{sense}\t{pos}\n" for sense, pos in metadata.items())
    return glossary + notice.encode(), {
        "english_senses": len(senses),
        "tagged_senses": len(metadata),
        "lemma_count": len(index),
    }
