#!/usr/bin/env python3
"""Compare the bundled modern chart against the official Pokémon chart (network required)."""

import re
import importlib.util
from pathlib import Path
from urllib.request import urlopen

EXPORT_SPEC = importlib.util.spec_from_file_location(
    "export_desktop_reference", Path(__file__).with_name("export_desktop_reference.py")
)
EXPORT_MODULE = importlib.util.module_from_spec(EXPORT_SPEC)
EXPORT_SPEC.loader.exec_module(EXPORT_MODULE)
GEN6_NAMES = EXPORT_MODULE.GEN6_NAMES
parse_type_chart = EXPORT_MODULE.parse_type_chart

SOURCE_URL = "https://sg.portal-pokemon.com/game/type-chart/"


def parse_official_chart(html: str, type_ids: dict[str, int]) -> dict[tuple[int, int], int]:
    table_match = re.search(
        r'<table class="pIXJrq_typeChartTable">(.*?)</table>', html, re.DOTALL
    )
    if not table_match:
        raise ValueError("Official type chart table was not found")
    table = table_match.group(1)
    head_match = re.search(r"<thead>(.*?)</thead>", table, re.DOTALL)
    if not head_match:
        raise ValueError("Official type chart headers were not found")
    defending_names = re.findall(r'<img[^>]*alt="([^"]+)"', head_match.group(1))
    defending_ids = [type_ids.get(name.upper()) for name in defending_names]
    expected_types = len(type_ids)
    if len(defending_ids) != expected_types or any(value is None for value in defending_ids):
        raise ValueError("Official chart has an unexpected defending type list")

    pairs: dict[tuple[int, int], int] = {}
    rows = re.findall(r'<tr class="pIXJrq_dataLine">(.*?)</tr>', table, re.DOTALL)
    if len(rows) != expected_types:
        raise ValueError(f"Official chart has {len(rows)} attacking types; expected {expected_types}")
    for row in rows:
        attack_name = re.search(r'<th[^>]*><img[^>]*alt="([^"]+)"', row)
        if not attack_name or attack_name.group(1).upper() not in type_ids:
            raise ValueError("Official chart has an unknown attacking type")
        attack_id = type_ids[attack_name.group(1).upper()]
        cells = re.findall(r'<td class="pIXJrq_dataCell">(.*?)</td>', row, re.DOTALL)
        if len(cells) != len(defending_ids):
            raise ValueError("Official chart row has an unexpected number of cells")
        for defense_id, cell in zip(defending_ids, cells):
            symbol = re.search(r'<img[^>]*alt="([^"]+)"', cell)
            multiplier = {"×": 0, "▲": 50, "●": 200}.get(symbol.group(1), 100) if symbol else 100
            pairs[(attack_id, defense_id)] = multiplier
    return pairs


def verify() -> int:
    source = GEN6_NAMES.read_text(encoding="utf-8")
    types, matchups = parse_type_chart(
        source, source, "typeNamesGlazed", "typeEffectivenessGlazed"
    )
    type_ids = {entry["name"].upper(): entry["id"] for entry in types}
    official = parse_official_chart(urlopen(SOURCE_URL, timeout=30).read().decode("utf-8"), type_ids)
    bundled = {(pair["attack"], pair["defense"]): pair["percent"] for pair in matchups}
    differences = [
        (attack, defense, official_value, bundled.get((attack, defense), 100))
        for (attack, defense), official_value in official.items()
        if bundled.get((attack, defense), 100) != official_value
    ]
    if differences:
        raise SystemExit(f"{len(differences)}/324 chart cells differ; first differences: {differences[:12]}")
    return len(official)


if __name__ == "__main__":
    print(f"official modern type chart: {verify()}/324 cells match ({SOURCE_URL})")
