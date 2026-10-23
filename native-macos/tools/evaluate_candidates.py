"""Mac-only reproducible synthetic accuracy evaluation; never read personal stores."""

import argparse
import hashlib
import json
import math
import queue
import subprocess
import tempfile
import threading
import time
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def evaluate(bridge, resources, cases):
    results = []
    with tempfile.TemporaryDirectory(prefix="lingomate-accuracy-") as temporary:
        for case in cases:
            memory = Path(temporary) / "words.json"
            seed = case.get("memory")
            entries = []
            if seed:
                entries = [
                    {
                        "pinyin": seed.get("pinyin", case["input"]),
                        "text": seed["text"],
                        "syllables": seed["syllables"],
                        "count": seed["count"],
                        "last": 1,
                    }
                ]
            memory.write_text(
                json.dumps(
                    {
                        "version": 1,
                        "sequence": 1 + (seed or {}).get("age", 0),
                        "entries": entries,
                    }
                ),
                encoding="utf-8",
            )
            before = memory.read_bytes()
            started = time.perf_counter()
            with subprocess.Popen(
                [
                    str(bridge),
                    str(resources),
                    "--mac-candidates",
                    "--memory",
                    str(memory),
                ],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                text=True,
                encoding="utf-8",
            ) as process:
                responses = queue.Queue()

                def read_lines(process=process, responses=responses):
                    for line in process.stdout:
                        responses.put(line)

                reader = threading.Thread(target=read_lines, daemon=True)
                reader.start()

                def request(process=process, responses=responses, input_=case["input"]):
                    process.stdin.write(
                        json.dumps(
                            {
                                "action": "query",
                                "input": input_,
                                "context": "accuracy",
                            }
                        )
                        + "\n"
                    )
                    process.stdin.flush()
                    try:
                        frame = json.loads(responses.get(timeout=10))
                    except queue.Empty:
                        process.kill()
                        process.wait(timeout=5)
                        raise TimeoutError("Evaluation query timed out") from None
                    if "error" in frame:
                        raise RuntimeError(frame["error"])
                    return frame

                frame = request()
                elapsed = (time.perf_counter() - started) * 1000
                request()  # Separate first-use indexing/startup from steady queries.
                warm = []
                for _ in range(5):
                    tick = time.perf_counter()
                    repeated = request()
                    warm.append((time.perf_counter() - tick) * 1000)
                    if repeated["candidates"] != frame["candidates"]:
                        raise AssertionError("Query-only repetition changed candidates")
                process.stdin.close()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
                    raise
                reader.join(timeout=5)
                if process.returncode:
                    raise RuntimeError(f"Bridge failed for {case['id']}")
            words = [item["text"] for item in frame["candidates"]]
            rank = next(
                (i + 1 for i, word in enumerate(words) if word in case["accepted"]),
                None,
            )
            if memory.read_bytes() != before:
                raise AssertionError("Query changed temporary memory")
            results.append(
                {
                    "id": case["id"],
                    "category": case["category"],
                    "input": case["input"],
                    "accepted": case["accepted"],
                    "rank": rank,
                    "top5": words[:5],
                    "elapsed_ms": round(elapsed, 3),
                    "warm_ms": [round(value, 3) for value in warm],
                }
            )
    groups = defaultdict(list)
    for result in results:
        groups[result["category"]].append(result)

    def summary(rows):
        return {
            "cases": len(rows),
            "top1": sum(r["rank"] == 1 for r in rows),
            "top5": sum(r["rank"] is not None and r["rank"] <= 5 for r in rows),
            "recall": sum(r["rank"] is not None for r in rows),
            "p95_warm_query_ms": sorted(
                value for row in rows for value in row["warm_ms"]
            )[math.ceil(len(rows) * 5 * 0.95) - 1],
            "p95_cold_process_ms": sorted(r["elapsed_ms"] for r in rows)[
                math.ceil(len(rows) * 0.95) - 1
            ],
        }

    return {
        "fixtures_sha256": hashlib.sha256(
            json.dumps(cases, ensure_ascii=False, sort_keys=True).encode("utf-8")
        ).hexdigest(),
        "summary": summary(results),
        "categories": {k: summary(v) for k, v in groups.items()},
        "results": results,
    }


def compare(baseline, report):
    if baseline.get("fixtures_sha256") != report.get("fixtures_sha256"):
        raise ValueError("Evaluation fixtures differ")
    before = {row["id"]: row for row in baseline["results"]}
    after = {row["id"]: row for row in report["results"]}
    if before.keys() != after.keys():
        raise ValueError("Evaluation case IDs differ")
    regressions = []
    for key, row in after.items():
        old = before[key]
        if any(old[field] != row[field] for field in ("input", "accepted", "category")):
            raise ValueError("Evaluation fixture changed")
        if (row["rank"] or 65) > (old["rank"] or 65):
            regressions.append(key)
    return regressions


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bridge", type=Path, required=True)
    parser.add_argument("--resources", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--baseline", type=Path)
    args = parser.parse_args()
    cases = json.loads(
        (ROOT / "native-macos/evaluation/cases.json").read_text(encoding="utf-8")
    )["cases"]
    report = evaluate(args.bridge.resolve(), args.resources.resolve(), cases)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(report["summary"], ensure_ascii=False))
    if args.baseline:
        regressions = compare(
            json.loads(args.baseline.read_text(encoding="utf-8")), report
        )
        if regressions:
            raise SystemExit("Candidate rank regressions: " + ", ".join(regressions))


if __name__ == "__main__":
    main()
