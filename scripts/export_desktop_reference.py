#!/usr/bin/env python3
"""Export the shared Gen 3 type chart from Kotlin source as a versioned JSON dataset."""

import argparse
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NAMES_PATH = ROOT / "app/src/main/kotlin/com/pokedaisy/app/companion/data/TypeChartUtil.kt"
CHART_PATH = ROOT / "app/src/main/kotlin/com/pokedaisy/app/companion/data/TypeChart.kt"
OUTPUT = ROOT / "desktop-rust/assets/reference/type-chart-gen3.json"


def kotlin_map(source: str, variable: str) -> str:
    match = re.search(rf"\b{re.escape(variable)}(?:\s*:[^=]+?)?\s*=\s*mapOf\((.*?)\)", source, re.S)
    if not match:
        raise ValueError(f"Kotlin map {variable!r} not found")
    return match.group(1)


def parse_type_chart(names_source: str, chart_source: str):
    names_body = kotlin_map(names_source, "typeNames")
    chart_body = kotlin_map(chart_source, "typeEffectiveness")
    names = [(int(key), value) for key, value in re.findall(r'(\d+)\s+to\s+"([^"\\]*)"', names_body)]
    if not names or len({key for key, _ in names}) != len(names):
        raise ValueError("type names are empty or contain duplicate IDs")
    known_ids = {key for key, _ in names}
    matchups = []
    seen = set()
    for raw_key, raw_percent in re.findall(r"(\d+)\s+to\s+(\d+)", chart_body):
        key, percent = int(raw_key), int(raw_percent)
        attack, defense = divmod(key, 100)
        if attack not in known_ids or defense not in known_ids:
            raise ValueError(f"type chart pair {key} references an unknown type")
        if percent not in (0, 50, 100, 200, 400):
            raise ValueError(f"unexpected type multiplier {percent}")
        if key in seen:
            raise ValueError(f"duplicate type chart pair {key}")
        seen.add(key)
        if percent != 100:
            matchups.append({"attack": attack, "defense": defense, "percent": percent})
    return (
        [{"id": key, "name": name} for key, name in sorted(names)],
        sorted(matchups, key=lambda pair: (pair["attack"], pair["defense"])),
    )


def render_dataset() -> str:
    names_source = NAMES_PATH.read_text(encoding="utf-8")
    chart_source = CHART_PATH.read_text(encoding="utf-8")
    types, matchups = parse_type_chart(names_source, chart_source)
    digest = hashlib.sha256((names_source + "\0" + chart_source).encode("utf-8")).hexdigest()
    dataset = {
        "schema_version": 1,
        "dataset_id": "pokemon-gen3-type-chart",
        "generation": 3,
        "source_files": [
            NAMES_PATH.relative_to(ROOT).as_posix(),
            CHART_PATH.relative_to(ROOT).as_posix(),
        ],
        "source_sha256": digest,
        "license": "GPL-3.0-only",
        "default_percent": 100,
        "types": types,
        "matchups": matchups,
    }
    return json.dumps(dataset, ensure_ascii=False, indent=2) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    generated = render_dataset()
    if args.check:
        actual = OUTPUT.read_text(encoding="utf-8") if OUTPUT.exists() else ""
        if actual != generated:
            raise SystemExit("Desktop type-chart dataset is stale; run python scripts/export_desktop_reference.py")
        print("desktop reference: type chart matches Kotlin source")
        return
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(generated, encoding="utf-8", newline="\n")
    print(f"desktop reference: wrote {OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
