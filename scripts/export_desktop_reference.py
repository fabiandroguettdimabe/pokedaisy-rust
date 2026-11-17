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
UI_DATASET_OUTPUT = ROOT / "desktop-rust/ui/src/data/type-charts-reference.json"
GEN1_NAMES = ROOT / "app/src/main/kotlin/com/pokedaisy/app/companion/data/TypeChartYellow.kt"
GEN2_NAMES = ROOT / "app/src/main/kotlin/com/pokedaisy/app/companion/data/TypeChartCrystal.kt"
GEN6_NAMES = ROOT / "app/src/main/kotlin/com/pokedaisy/app/companion/data/TypeChartGlazed.kt"


def kotlin_map(source: str, variable: str) -> str:
    match = re.search(rf"\b{re.escape(variable)}(?:\s*:[^=]+?)?\s*=\s*mapOf\((.*?)\)", source, re.S)
    if not match:
        raise ValueError(f"Kotlin map {variable!r} not found")
    return match.group(1)


def parse_type_chart(names_source: str, chart_source: str, names_variable="typeNames", chart_variable="typeEffectiveness"):
    names_body = kotlin_map(names_source, names_variable)
    chart_body = kotlin_map(chart_source, chart_variable)
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


def render_additional_datasets() -> str:
    definitions = [
        (1, "yellow", GEN1_NAMES, "typeNamesYellow", "typeEffectivenessYellow"),
        (2, "crystal", GEN2_NAMES, "typeNamesCrystal", "typeEffectivenessCrystal"),
        (6, "glazed", GEN6_NAMES, "typeNamesGlazed", "typeEffectivenessGlazed"),
    ]
    datasets = []
    for generation, game, source_path, names_var, chart_var in definitions:
        source = source_path.read_text(encoding="utf-8")
        types, matchups = parse_type_chart(source, source, names_var, chart_var)
        datasets.append({
            "dataset_id": f"pokemon-gen{generation}-{game}-type-chart",
            "generation": generation,
            "reference_game": {1: "Pokémon Yellow", 2: "Pokémon Crystal", 6: "Pokémon Glazed 9.2.0"}[generation],
            "source_files": [source_path.relative_to(ROOT).as_posix()],
            "source_sha256": hashlib.sha256(source.encode("utf-8")).hexdigest(),
            "license": "GPL-3.0-only",
            "default_percent": 100,
            "types": types,
            "matchups": matchups,
        })
    return json.dumps({"schema_version": 1, "datasets": datasets}, ensure_ascii=False, indent=2) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    generated = render_dataset()
    additional = render_additional_datasets()
    if args.check:
        actual = OUTPUT.read_text(encoding="utf-8") if OUTPUT.exists() else ""
        if actual != generated:
            raise SystemExit("Desktop type-chart dataset is stale; run python scripts/export_desktop_reference.py")
        ui_actual = UI_DATASET_OUTPUT.read_text(encoding="utf-8") if UI_DATASET_OUTPUT.exists() else ""
        if ui_actual != additional:
            raise SystemExit("Desktop additional type charts are stale; run python scripts/export_desktop_reference.py")
        print("desktop reference: type charts match Kotlin sources")
        return
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(generated, encoding="utf-8", newline="\n")
    UI_DATASET_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    UI_DATASET_OUTPUT.write_text(additional, encoding="utf-8", newline="\n")
    print(f"desktop reference: wrote additional versioned charts, including {UI_DATASET_OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
