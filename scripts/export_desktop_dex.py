#!/usr/bin/env python3
"""Export the existing National Dex names as an attributed offline Desktop dataset."""

import hashlib
import json
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
SOURCE = ROOT / "app/src/main/kotlin/com/pokedaisy/app/companion/data/SpeciesNamesNationalDex.kt"
OUTPUT = ROOT / "desktop-rust/ui/src/data/pokedex-gen9.json"
GENERATION_ENDS = (151, 251, 386, 493, 649, 721, 809, 905, 1025)


def parse_species_names(source: str, max_dex: int = 386) -> list[dict]:
    if not 1 <= max_dex <= GENERATION_ENDS[-1]:
        raise ValueError(f"max_dex must be between 1 and {GENERATION_ENDS[-1]}")
    entries = {}
    for match in re.finditer(r'(\d+)\s+to\s+("(?:\\.|[^"\\])*")', source):
        number = int(match.group(1))
        if number > max_dex:
            continue
        if number in entries:
            raise ValueError(f"duplicate National Dex number {number}")
        name = json.loads(match.group(2))
        if not name or any(ord(char) < 32 for char in name):
            raise ValueError(f"invalid species name at National Dex number {number}")
        entries[number] = name

    expected = set(range(1, max_dex + 1))
    missing = expected - entries.keys()
    if missing:
        raise ValueError(f"missing National Dex numbers: {sorted(missing)[:8]}")

    return [
        {
            "number": number,
            "name": entries[number],
            "generation": next(i for i, last in enumerate(GENERATION_ENDS, 1) if number <= last),
        }
        for number in range(1, max_dex + 1)
    ]


def build_dataset(source_bytes: bytes) -> dict:
    source = source_bytes.decode("utf-8")
    species = parse_species_names(source, max_dex=GENERATION_ENDS[-1])
    generations = []
    first = 1
    for number, last in enumerate(GENERATION_ENDS, 1):
        generations.append({"generation": number, "first": first, "last": last})
        first = last + 1
    return {
        "schema_version": 1,
        "dataset_id": "pokemon-national-dex-gen9",
        "max_generation": len(GENERATION_ENDS),
        "generations": generations,
        "species_count": len(species),
        "source_sha256": hashlib.sha256(source_bytes).hexdigest(),
        "license": "BSD-3-Clause",
        "attribution": "PokeAPI contributors",
        "species": species,
    }


def main() -> None:
    dataset = build_dataset(SOURCE.read_bytes())
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(dataset, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {OUTPUT.relative_to(ROOT)}: {dataset['species_count']} species, {dataset['license']}")


if __name__ == "__main__":
    main()
