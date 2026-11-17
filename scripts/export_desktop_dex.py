#!/usr/bin/env python3
"""Export the existing National Dex names as an attributed offline Desktop dataset."""

import csv
import hashlib
import io
import json
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
POKEAPI_DATA = ROOT / "scripts/data/pokeapi"
SOURCE = ROOT / "app/src/main/kotlin/com/pokedaisy/app/companion/data/SpeciesNamesNationalDex.kt"
GEN3_SPECIES_SOURCE = ROOT / "app/src/main/kotlin/com/pokedaisy/app/companion/data/save/SpeciesDexGen.kt"
GEN3_ITEM_SOURCE = ROOT / "app/src/main/kotlin/com/pokedaisy/app/companion/data/ItemTextGame.kt"
OUTPUT = ROOT / "desktop-rust/ui/src/data/pokedex-gen9.json"
GENERATION_ENDS = (151, 251, 386, 493, 649, 721, 809, 905, 1025)
REGIONAL_DEX_FILES = (
    "pokedexes.csv",
    "pokemon_dex_numbers.csv",
    "version_group_regions.csv",
    "version_groups.csv",
    "regions.csv",
)
REGIONAL_DEX_VERSION_GROUPS = {
    "updated-johto": "heartgold-soulsilver",
    "updated-hoenn": "omega-ruby-alpha-sapphire",
    "letsgo-kanto": "lets-go-pikachu-lets-go-eevee",
    "lumiose-city": "legends-za",
    "hyperspace": "mega-dimension",
}


def csv_rows(text: str) -> list[dict[str, str]]:
    return list(csv.DictReader(io.StringIO(text)))


def parse_regional_dexes(
    pokedexes_csv: str,
    dex_numbers_csv: str,
    version_group_regions_csv: str,
    version_groups_csv: str,
    regions_csv: str,
    national_species: list[dict],
) -> list[dict]:
    """Join main-series regional Pokédexes to the project's National Dex names."""
    regions = {int(row["id"]): row["identifier"] for row in csv_rows(regions_csv)}
    group_generations = {int(row["id"]): int(row["generation_id"]) for row in csv_rows(version_groups_csv)}
    group_generation_by_name = {row["identifier"]: int(row["generation_id"]) for row in csv_rows(version_groups_csv)}
    regional_generations: dict[int, int] = {}
    for row in csv_rows(version_group_regions_csv):
        group_id, region_id = int(row["version_group_id"]), int(row["region_id"])
        if group_id not in group_generations or region_id not in regions:
            continue
        generation = group_generations[group_id]
        regional_generations[region_id] = min(generation, regional_generations.get(region_id, generation))

    dexes: dict[int, dict] = {}
    for row in csv_rows(pokedexes_csv):
        if row["is_main_series"] != "1" or not row["region_id"]:
            continue
        dex_id, region_id = int(row["id"]), int(row["region_id"])
        if dex_id in dexes:
            raise ValueError(f"duplicate regional dex ID {dex_id}")
        if region_id not in regions or region_id not in regional_generations:
            raise ValueError(f"regional dex {dex_id} has no known region generation")
        identifier = row["identifier"]
        name = " ".join(word.capitalize() if word not in {"of", "the"} else word for word in identifier.split("-"))
        if identifier == "letsgo-kanto":
            name = "Let's Go Kanto"
        generation = group_generation_by_name.get(
            REGIONAL_DEX_VERSION_GROUPS.get(identifier, ""), regional_generations[region_id]
        )
        dexes[dex_id] = {
            "dataset_id": f"pokeapi-pokedex-{dex_id}",
            "identifier": identifier,
            "name": name,
            "generation": generation,
            "species": [],
        }

    national_by_number = {item["number"]: item for item in national_species}
    seen: dict[int, set[int]] = {dex_id: set() for dex_id in dexes}
    for row in csv_rows(dex_numbers_csv):
        dex_id = int(row["pokedex_id"])
        if dex_id not in dexes:
            continue
        species_id, dex_number = int(row["species_id"]), int(row["pokedex_number"])
        if dex_number == 0:
            continue
        if dex_number < 0 or dex_number in seen[dex_id]:
            raise ValueError(f"duplicate regional number {dex_number} in dex {dex_id}")
        species = national_by_number.get(species_id)
        if species is None:
            raise ValueError(f"unknown National Dex species {species_id} in dex {dex_id}")
        seen[dex_id].add(dex_number)
        dexes[dex_id]["species"].append({
            "number": dex_number,
            "national_number": species_id,
            "name": species["name"],
            "generation": species["generation"],
        })

    result = []
    for dex_id, dex in sorted(dexes.items()):
        dex["species"].sort(key=lambda item: item["number"])
        dex["species_count"] = len(dex["species"])
        if not dex["species"]:
            raise ValueError(f"regional dex {dex_id} has no species")
        result.append(dex)
    return result


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
    gen3_map_bytes = GEN3_SPECIES_SOURCE.read_bytes()
    gen3_map = parse_gen3_species_dex(gen3_map_bytes.decode("utf-8"))
    gen3_item_bytes = GEN3_ITEM_SOURCE.read_bytes()
    gen3_items = parse_item_names(gen3_item_bytes.decode("utf-8"))
    regional_sources = {name: (POKEAPI_DATA / name).read_bytes() for name in REGIONAL_DEX_FILES}
    regional_dexes = parse_regional_dexes(
        *(regional_sources[name].decode("utf-8") for name in REGIONAL_DEX_FILES), species
    )
    regional_digest = hashlib.sha256()
    for name in REGIONAL_DEX_FILES:
        regional_digest.update(name.encode("utf-8") + b"\0" + regional_sources[name])
    regional_hash = regional_digest.hexdigest()
    source_commit = (ROOT / "scripts/data/pokeapi/SOURCE_COMMIT.txt").read_text(encoding="utf-8").strip()
    for dex in regional_dexes:
        dex.update({
            "species_source_sha256": regional_hash,
            "species_source_commit": source_commit,
            "species_license": "BSD-3-Clause",
            "species_attribution": "PokeAPI contributors",
            "species_source_url": f"https://github.com/PokeAPI/pokeapi/tree/{source_commit}/data/v2/csv",
            "species_scope": "Pokédex regional de la serie principal según PokeAPI; no garantiza disponibilidad exacta por versión regional del juego ni incluye formas alternativas.",
        })
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
        "regional_dexes": regional_dexes,
        "regional_dex_source_files": [f"scripts/data/pokeapi/{name}" for name in REGIONAL_DEX_FILES],
        "regional_dex_source_sha256": regional_hash,
        "regional_dex_source_commit": source_commit,
        "regional_dex_license": "BSD-3-Clause",
        "regional_dex_attribution": "PokeAPI contributors",
        "gen3_species_map": gen3_map,
        "gen3_species_source_sha256": hashlib.sha256(gen3_map_bytes).hexdigest(),
        "gen3_item_names": gen3_items,
        "gen3_item_source_sha256": hashlib.sha256(gen3_item_bytes).hexdigest(),
    }


def parse_gen3_species_dex(source: str) -> list[int]:
    match = re.search(r"speciesNationalDex:\s*IntArray\s*=\s*intArrayOf\((.*?)\)", source, re.S)
    if not match:
        raise ValueError("missing generated Gen III species-to-National-Dex map")
    values = [int(value) for value in re.findall(r"\d+", match.group(1))]
    if len(values) != 412 or values[0] != 0:
        raise ValueError(f"invalid Gen III species map size or sentinel: {len(values)}")
    if any(number < 0 or number > 411 for number in values):
        raise ValueError("Gen III species map contains an invalid National Dex number")
    return values


def parse_item_names(source: str) -> dict[str, str]:
    match = re.search(r"itemNamesFireRedGame:\s*Map<Int, String>\s*=\s*mapOf\((.*?)\n\)", source, re.S)
    if not match:
        raise ValueError("missing generated Gen III item-name table")
    entries = {}
    for item_id, encoded_name in re.findall(r'(\d+)\s+to\s+("(?:\\.|[^"\\])*")', match.group(1)):
        key = str(int(item_id))
        if key in entries:
            raise ValueError(f"duplicate Gen III item ID {key}")
        name = json.loads(encoded_name)
        if not name or any(ord(char) < 32 for char in name):
            raise ValueError(f"invalid Gen III item name at ID {key}")
        entries[key] = name
    if not entries:
        raise ValueError("empty Gen III item-name table")
    return entries


def main() -> None:
    dataset = build_dataset(SOURCE.read_bytes())
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(dataset, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {OUTPUT.relative_to(ROOT)}: {dataset['species_count']} species, {dataset['license']}")


if __name__ == "__main__":
    main()
