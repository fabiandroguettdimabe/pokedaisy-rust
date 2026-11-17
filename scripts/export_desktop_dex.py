#!/usr/bin/env python3
"""Export the existing National Dex names as an attributed offline Desktop dataset."""

import hashlib
import json
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
SOURCE = ROOT / "app/src/main/kotlin/com/pokedaisy/app/companion/data/SpeciesNamesNationalDex.kt"
OUTPUT = ROOT / "desktop-rust/ui/src/data/pokedex-gen3.json"


def parse_species_names(source: str, max_dex: int = 386) -> list[dict]:
    if not 1 <= max_dex <= 1025:
        raise ValueError("max_dex must be between 1 and 1025")
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

    def generation(number: int) -> int:
        if number <= 151:
            return 1
        if number <= 251:
            return 2
        return 3

    return [
        {"number": number, "name": entries[number], "generation": generation(number)}
        for number in range(1, max_dex + 1)
    ]


def build_dataset(source_bytes: bytes) -> dict:
    source = source_bytes.decode("utf-8")
    species = parse_species_names(source)
    return {
        "schema_version": 1,
        "dataset_id": "pokemon-national-dex-gen3",
        "max_generation": 3,
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
