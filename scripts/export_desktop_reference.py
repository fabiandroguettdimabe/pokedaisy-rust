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
GEN1_LEGACY = ROOT / "app/src/main/kotlin/com/pokedaisy/app/companion/data/TypeChartYellowLegacy.kt"
GEN2_NAMES = ROOT / "app/src/main/kotlin/com/pokedaisy/app/companion/data/TypeChartCrystal.kt"
GEN6_NAMES = ROOT / "app/src/main/kotlin/com/pokedaisy/app/companion/data/TypeChartGlazed.kt"
ROWE_NAMES = ROOT / "app/src/main/kotlin/com/pokedaisy/app/companion/data/TypeChartRowe.kt"
HACK_CHARTS = ROOT / "app/src/main/kotlin/com/pokedaisy/app/companion/data"
SPECIES_SOURCES = {
    "yellow": ("SpeciesNamesYellow.kt", "speciesNamesYellow", "SpeciesTypesYellow.kt", "speciesTypeDataYellow"),
    "yellow-legacy": ("SpeciesNamesYellow.kt", "speciesNamesYellow", "SpeciesTypesYellow.kt", "speciesTypeDataYellow"),
    "crystal": ("SpeciesNamesCrystal.kt", "speciesNamesCrystal", "SpeciesTypesCrystal.kt", "speciesTypeDataCrystal"),
    "glazed": ("SpeciesNamesGlazed.kt", "speciesNamesGlazed", "SpeciesTypesGlazed.kt", "speciesTypeDataGlazed"),
    "rowe": ("SpeciesNamesRowe.kt", "speciesNamesRowe", "SpeciesTypesRowe.kt", "speciesTypeDataRowe"),
    "wishes": ("SpeciesNamesWishes.kt", "speciesNamesWishes", "SpeciesTypesWishes.kt", "speciesTypeDataWishes"),
    "team-rocket": ("SpeciesNamesTeamRocketV3.kt", "speciesNamesTeamRocketV3", "SpeciesTypesTeamRocketV3.kt", "speciesTypeDataTeamRocketV3"),
    "soul-gold": ("SpeciesNamesSoulGold.kt", "speciesNamesSoulGold", "SpeciesTypesSoulGold.kt", "speciesTypeDataSoulGold"),
    "rogue": ("SpeciesNamesRogue.kt", "speciesNamesRogue", "SpeciesTypesRogue.kt", "speciesTypeDataRogue"),
    "quetzal": ("SpeciesNamesQuetzal.kt", "speciesNamesQuetzal", "SpeciesTypesQuetzal.kt", "speciesTypeDataQuetzal"),
    "lazarus": ("SpeciesNamesLazarus.kt", "speciesNamesLazarus", "SpeciesTypesLazarus.kt", "speciesTypeDataLazarus"),
    "imperium": ("SpeciesNamesImperium.kt", "speciesNamesImperium", "SpeciesTypesImperium.kt", "speciesTypeDataImperium"),
    "hns": ("SpeciesNamesHns.kt", "speciesNamesHns", "SpeciesTypesHns.kt", "speciesTypeDataHns"),
}


def kotlin_map(source: str, variable: str) -> str:
    match = re.search(rf"\b{re.escape(variable)}(?:\s*:[^=]+?)?\s*=\s*mapOf\(", source)
    if not match:
        raise ValueError(f"Kotlin map {variable!r} not found")
    start = match.end() - 1
    depth = 1
    index = start + 1
    in_string = False
    line_comment = False
    block_comment = False
    while index < len(source):
        char = source[index]
        following = source[index + 1] if index + 1 < len(source) else ""
        if line_comment:
            line_comment = char != "\n"
        elif block_comment:
            if char == "*" and following == "/":
                block_comment = False
                index += 1
        elif in_string:
            if char == "\\":
                index += 1
            elif char == '"':
                in_string = False
        elif char == "/" and following == "/":
            line_comment = True
            index += 1
        elif char == "/" and following == "*":
            block_comment = True
            index += 1
        elif char == '"':
            in_string = True
        elif char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
            if depth == 0:
                return source[start + 1:index]
        index += 1
    raise ValueError(f"Kotlin map {variable!r} is not closed")


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


def parse_species_reference(names_source: str, names_variable: str, types_source: str, types_variable: str, known_type_ids: set[int], strict_type_ids: bool = True):
    names_body = kotlin_map(names_source, names_variable)
    types_body = kotlin_map(types_source, types_variable)
    names = {int(number): json.loads(raw_name) for number, raw_name in re.findall(r'(\d+)\s+to\s+("(?:\\.|[^"\\])*")', names_body)}
    raw_types = re.findall(r"(\d+)\s+to\s+SpeciesTypes\(\s*(\d+)\s*,\s*(\d+)\s*\)", types_body)
    types = {int(number): (int(first), int(second)) for number, first, second in raw_types}
    if len(names) != len(re.findall(r'(\d+)\s+to\s+("(?:\\.|[^"\\])*")', names_body)):
        raise ValueError(f"duplicate species ID in {names_variable}")
    if len(types) != len(raw_types):
        raise ValueError(f"duplicate species ID in {types_variable}")
    entries = []
    for number in sorted(names.keys() & types.keys()):
        name = names[number]
        pair = types[number]
        if not name or any(ord(char) < 32 for char in name):
            raise ValueError(f"invalid species name at game species ID {number}")
        if any(type_id not in known_type_ids for type_id in pair):
            if not any(char.isalnum() for char in name):
                continue  # Some game tables reserve unnamed internal slots with sentinel type bytes.
            if strict_type_ids:
                raise ValueError(f"species {number} references an unknown type in {types_variable}")
            pair = ()  # The species name is still useful, but its unsupported type bytes stay unknown.
        entries.append({"number": number, "name": name, "types": list(dict.fromkeys(pair))})
    if not entries:
        raise ValueError(f"no joined species name/type rows for {names_variable}")
    return entries


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
        (1, "yellow-legacy", GEN1_LEGACY, "typeNamesYellowLegacy", "typeEffectivenessYellowLegacy"),
        (2, "crystal", GEN2_NAMES, "typeNamesCrystal", "typeEffectivenessCrystal"),
        (6, "glazed", GEN6_NAMES, "typeNamesGlazed", "typeEffectivenessGlazed"),
        (6, "rowe", ROWE_NAMES, "typeNamesRowe", "typeEffectivenessRowe"),
        (6, "wishes", HACK_CHARTS / "TypeChartWishes.kt", "typeNamesWishes", "typeEffectivenessWishes"),
        (6, "team-rocket", HACK_CHARTS / "TypeChartTeamRocketV3.kt", "typeNamesTeamRocketV3", "typeEffectivenessTeamRocketV3"),
        (6, "soul-gold", HACK_CHARTS / "TypeChartSoulGold.kt", "typeNamesSoulGold", "typeEffectivenessSoulGold"),
        (6, "rogue", HACK_CHARTS / "TypeChartRogue.kt", "typeNamesRogue", "typeEffectivenessRogue"),
        (6, "quetzal", HACK_CHARTS / "TypeChartQuetzal.kt", "typeNamesQuetzal", "typeEffectivenessQuetzal"),
        (6, "lazarus", HACK_CHARTS / "TypeChartLazarus.kt", "typeNamesLazarus", "typeEffectivenessLazarus"),
        (6, "imperium", HACK_CHARTS / "TypeChartImperium.kt", "typeNamesImperium", "typeEffectivenessImperium"),
        (6, "hns", HACK_CHARTS / "TypeChartHns.kt", "typeNamesHns", "typeEffectivenessHns"),
    ]
    datasets = []
    for generation, game, source_path, names_var, chart_var in definitions:
        source = source_path.read_text(encoding="utf-8")
        types, matchups = parse_type_chart(source, source, names_var, chart_var)
        species_names_path, species_names_var, species_types_path, species_types_var = SPECIES_SOURCES[game]
        species_names_path = HACK_CHARTS / species_names_path
        species_types_path = HACK_CHARTS / species_types_path
        names_source = species_names_path.read_text(encoding="utf-8")
        species_types_source = species_types_path.read_text(encoding="utf-8")
        species = parse_species_reference(names_source, species_names_var, species_types_source, species_types_var, {t["id"] for t in types}, strict_type_ids=False)
        dataset = {
            "dataset_id": f"pokemon-gen{generation}-{game}-type-chart",
            "generation": generation,
            "reference_game": {
                "yellow": "Pokémon Yellow",
                "yellow-legacy": "Pokémon Yellow Legacy v1.0.10",
                "crystal": "Pokémon Crystal",
                "glazed": "Pokémon Glazed 9.2.0",
                "rowe": "Pokémon R.O.W.E. v2.1.9.1 Experimental",
                "wishes": "Pokémon Wishes of Tomorrow v1.2.6",
                "team-rocket": "Pokémon Team Rocket Edition 2026 (ES) v3.0.9",
                "soul-gold": "Pokémon SoulGold v1.1.4",
                "rogue": "Emerald Rogue v2.2.1-EX",
                "quetzal": "Pokémon Quetzal English Alpha 9 v0",
                "lazarus": "Pokémon Lazarus v2.0",
                "imperium": "Pokémon Emerald Imperium v1.3.1",
                "hns": "Pokémon Heart and Soul v2.0.6",
            }[game],
            "source_files": [source_path.relative_to(ROOT).as_posix()],
            "source_sha256": hashlib.sha256(source.encode("utf-8")).hexdigest(),
            "license": "GPL-3.0-only",
            "default_percent": 100,
            "types": types,
            "matchups": matchups,
            "species_count": len(species),
            "species": species,
            "species_source_files": [species_names_path.relative_to(ROOT).as_posix(), species_types_path.relative_to(ROOT).as_posix()],
            "species_source_sha256": hashlib.sha256((names_source + "\0" + species_types_source).encode("utf-8")).hexdigest(),
        }
        datasets.append(dataset)

    classic_names_source = NAMES_PATH.read_text(encoding="utf-8")
    classic_chart_source = CHART_PATH.read_text(encoding="utf-8")
    classic_types, classic_matchups = parse_type_chart(classic_names_source, classic_chart_source)
    classic_hash = hashlib.sha256((classic_names_source + "\0" + classic_chart_source).encode("utf-8")).hexdigest()
    for generation in (4, 5):
        datasets.append({
            "dataset_id": f"pokemon-gen{generation}-standard-type-chart",
            "generation": generation,
            "reference_game": f"Estándar retail · Generación {generation}",
            "source_files": [NAMES_PATH.relative_to(ROOT).as_posix(), CHART_PATH.relative_to(ROOT).as_posix()],
            "source_sha256": classic_hash,
            "license": "GPL-3.0-only",
            "default_percent": 100,
            "types": classic_types,
            "matchups": classic_matchups,
        })

    modern_source = GEN6_NAMES.read_text(encoding="utf-8")
    modern_types, modern_matchups = parse_type_chart(
        modern_source, modern_source, "typeNamesGlazed", "typeEffectivenessGlazed"
    )
    modern_hash = hashlib.sha256(modern_source.encode("utf-8")).hexdigest()
    for generation in (6, 7, 8, 9):
        datasets.append({
            "dataset_id": f"pokemon-gen{generation}-standard-type-chart",
            "generation": generation,
            "reference_game": f"Estándar retail · Generación {generation} (Fairy)",
            "source_files": [GEN6_NAMES.relative_to(ROOT).as_posix()],
            "source_sha256": modern_hash,
            "verification_source_url": "https://sg.portal-pokemon.com/game/type-chart/",
            "license": "GPL-3.0-only",
            "default_percent": 100,
            "types": modern_types,
            "matchups": modern_matchups,
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
