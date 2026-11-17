import importlib.util
import json
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "export_desktop_reference", ROOT / "scripts" / "export_desktop_reference.py"
)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)
VERIFY_SPEC = importlib.util.spec_from_file_location(
    "verify_desktop_standard_type_chart", ROOT / "scripts" / "verify_desktop_standard_type_chart.py"
)
VERIFY_MODULE = importlib.util.module_from_spec(VERIFY_SPEC)
VERIFY_SPEC.loader.exec_module(VERIFY_MODULE)


class ExportDesktopReferenceTests(unittest.TestCase):
    def test_pokeapi_species_parser_uses_generation_historical_types_and_default_forms(self):
        pokemon = "id,identifier,species_id,is_default\n1,alpha,1,1\n2,alpha-alt,1,0\n3,beta,2,1\n"
        current_types = "pokemon_id,type_id,slot\n1,1,1\n1,18,2\n2,4,1\n3,9,1\n"
        past_types = "pokemon_id,generation_id,type_id,slot\n1,5,1,1\n1,5,13,2\n"
        species = "id,identifier,generation_id\n1,alpha,1\n2,beta,9\n"
        pokeapi_type_ids = {1: 0, 9: 8, 13: 3, 18: 9}
        gen5 = MODULE.parse_pokeapi_species_types(
            5, {1: "Alpha", 2: "Beta"}, pokeapi_type_ids, pokemon, current_types, past_types, species
        )
        self.assertEqual(gen5, [{"number": 1, "name": "Alpha", "types": [0, 3]}])
        gen9 = MODULE.parse_pokeapi_species_types(
            9, {1: "Alpha", 2: "Beta"}, pokeapi_type_ids, pokemon, current_types, past_types, species
        )
        self.assertEqual(gen9, [
            {"number": 1, "name": "Alpha", "types": [0, 9]},
            {"number": 2, "name": "Beta", "types": [8]},
        ])

    def test_parses_official_standard_chart_symbols(self):
        html = '''<table class="pIXJrq_typeChartTable"><thead><tr>
          <th></th><th><img alt="NORMAL"/></th><th><img alt="GHOST"/></tr></thead><tbody>
          <tr class="pIXJrq_dataLine"><th><img alt="NORMAL"/></th><td class="pIXJrq_dataCell"></td><td class="pIXJrq_dataCell"></td></tr>
          <tr class="pIXJrq_dataLine"><th><img alt="GHOST"/></th>
          <td class="pIXJrq_dataCell"><img alt="×"/></td><td class="pIXJrq_dataCell"><img alt="●"/></td></tr>
          </tbody></table>'''
        self.assertEqual(
            VERIFY_MODULE.parse_official_chart(html, {"NORMAL": 0, "GHOST": 7}),
            {(0, 0): 100, (0, 7): 100, (7, 0): 0, (7, 7): 200},
        )

    def test_parses_type_ids_and_sparse_effectiveness_pairs(self):
        types, matchups = MODULE.parse_type_chart(
            'val typeNames = mapOf(0 to "Normal", 1 to "Fire")',
            "val typeEffectiveness: Map<Int, Int> = mapOf(100 to 200, 1 to 50)",
        )
        self.assertEqual(types, [{"id": 0, "name": "Normal"}, {"id": 1, "name": "Fire"}])
        self.assertEqual(matchups, [
            {"attack": 0, "defense": 1, "percent": 50},
            {"attack": 1, "defense": 0, "percent": 200},
        ])

    def test_rejects_pair_that_references_unknown_type(self):
        with self.assertRaises(ValueError):
            MODULE.parse_type_chart(
                'val typeNames = mapOf(0 to "Normal")',
                "val typeEffectiveness: Map<Int, Int> = mapOf(101 to 200)",
            )

    def test_parses_versioned_generation_chart_variable_names(self):
        types, matchups = MODULE.parse_type_chart(
            'val typeNamesYellow = mapOf(0 to "NORMAL", 8 to "GHOST")',
            'val typeEffectivenessYellow = mapOf(8 to 0)',
            names_variable="typeNamesYellow",
            chart_variable="typeEffectivenessYellow",
        )
        self.assertEqual(types, [{"id": 0, "name": "NORMAL"}, {"id": 8, "name": "GHOST"}])
        self.assertEqual(matchups, [{"attack": 0, "defense": 8, "percent": 0}])

    def test_exports_versioned_base_and_hack_charts_with_source_provenance(self):
        data = json.loads(MODULE.render_additional_datasets())
        by_id = {dataset["dataset_id"]: dataset for dataset in data["datasets"]}
        yellow = by_id["pokemon-gen1-yellow-type-chart"]
        crystal = by_id["pokemon-gen2-crystal-type-chart"]
        glazed = by_id["pokemon-gen6-glazed-type-chart"]
        rowe = by_id["pokemon-gen6-rowe-type-chart"]
        self.assertEqual((yellow["generation"], yellow["reference_game"]), (1, "Pokémon Yellow"))
        self.assertEqual((crystal["generation"], crystal["reference_game"]), (2, "Pokémon Crystal"))
        self.assertEqual(yellow["license"], "GPL-3.0-only")
        self.assertIn({"attack": 8, "defense": 24, "percent": 0}, yellow["matchups"])
        self.assertIn({"attack": 27, "defense": 24, "percent": 200}, crystal["matchups"])
        self.assertIn({"id": 9, "name": "STEEL"}, crystal["types"])
        self.assertEqual((glazed["generation"], glazed["reference_game"]), (6, "Pokémon Glazed 9.2.0"))
        self.assertIn({"id": 9, "name": "Fairy"}, glazed["types"])
        self.assertIn({"attack": 9, "defense": 16, "percent": 200}, glazed["matchups"])
        self.assertEqual((rowe["generation"], rowe["reference_game"]), (6, "Pokémon R.O.W.E. v2.1.9.1 Experimental"))
        self.assertIn({"id": 18, "name": "Fairy"}, rowe["types"])
        self.assertIn({"attack": 18, "defense": 1, "percent": 200}, rowe["matchups"])
        pikachu = next(species for species in rowe["species"] if species["number"] == 25)
        self.assertEqual(pikachu, {"number": 25, "name": "Pikachu", "types": [13]})
        self.assertEqual(rowe["species_count"], len(rowe["species"]))
        self.assertEqual(len(rowe["species_source_sha256"]), 64)
        self.assertEqual(len(by_id), 19)
        classic_gen4 = by_id["pokemon-gen4-standard-type-chart"]
        classic_gen5 = by_id["pokemon-gen5-standard-type-chart"]
        modern_gen6 = by_id["pokemon-gen6-standard-type-chart"]
        modern_gen7 = by_id["pokemon-gen7-standard-type-chart"]
        modern_gen8 = by_id["pokemon-gen8-standard-type-chart"]
        modern_gen9 = by_id["pokemon-gen9-standard-type-chart"]
        for chart, expected_count in zip(
            (classic_gen4, classic_gen5, modern_gen6, modern_gen7, modern_gen8, modern_gen9),
            (493, 649, 721, 809, 905, 1025),
        ):
            self.assertEqual(chart["species_count"], expected_count)
            self.assertEqual(len(chart["species"]), expected_count)
            self.assertEqual(len(chart["species_source_sha256"]), 64)
            self.assertEqual(chart["species_source_commit"], MODULE.POKEAPI_COMMIT)
            self.assertEqual(chart["species_license"], "BSD-3-Clause")
            self.assertEqual(chart["species_attribution"], "PokeAPI contributors")
            self.assertIn("formas predeterminadas", chart["species_scope"])
        clefairy_gen5 = next(species for species in classic_gen5["species"] if species["number"] == 35)
        clefairy_gen6 = next(species for species in modern_gen6["species"] if species["number"] == 35)
        self.assertEqual(clefairy_gen5["types"], [0])
        self.assertEqual(clefairy_gen6["types"], [9])
        magnemite_gen5 = next(species for species in classic_gen5["species"] if species["number"] == 81)
        mawile_gen5 = next(species for species in classic_gen5["species"] if species["number"] == 303)
        mawile_gen6 = next(species for species in modern_gen6["species"] if species["number"] == 303)
        self.assertEqual(magnemite_gen5["types"], [13, 8])
        self.assertEqual(mawile_gen5["types"], [8])
        self.assertEqual(mawile_gen6["types"], [8, 9])
        self.assertEqual(classic_gen4["generation"], 4)
        self.assertEqual(classic_gen5["generation"], 5)
        self.assertEqual(classic_gen4["types"], classic_gen5["types"])
        self.assertEqual(classic_gen4["matchups"], classic_gen5["matchups"])
        classic_base = json.loads(MODULE.render_dataset())
        self.assertEqual(classic_gen4["types"], classic_base["types"])
        self.assertEqual(classic_gen4["matchups"], classic_base["matchups"])
        self.assertIn({"attack": 7, "defense": 8, "percent": 50}, classic_gen4["matchups"])
        self.assertEqual([chart["generation"] for chart in (modern_gen6, modern_gen7, modern_gen8, modern_gen9)], [6, 7, 8, 9])
        for chart in (modern_gen6, modern_gen7, modern_gen8, modern_gen9):
            self.assertIn({"id": 9, "name": "Fairy"}, chart["types"])
            self.assertEqual(chart["default_percent"], 100)
            self.assertEqual(len(chart["source_sha256"]), 64)
            self.assertEqual(chart["verification_source_url"], "https://sg.portal-pokemon.com/game/type-chart/")
            self.assertEqual(chart["matchups"], modern_gen6["matchups"])
        modern = modern_gen9
        self.assertIn({"attack": 9, "defense": 16, "percent": 200}, modern["matchups"])
        self.assertEqual(next((pair["percent"] for pair in modern["matchups"] if pair["attack"] == 7 and pair["defense"] == 8), 100), 100)
        profiles = [(1, "yellow-legacy"), (6, "wishes"), (6, "team-rocket"), (6, "soul-gold"),
                    (6, "rogue"), (6, "quetzal"), (6, "lazarus"), (6, "imperium"), (6, "hns")]
        for generation, slug in profiles:
            dataset = by_id[f"pokemon-gen{generation}-{slug}-type-chart"]
            self.assertEqual(len(dataset["source_sha256"]), 64)
            self.assertEqual(dataset["license"], "GPL-3.0-only")
            self.assertTrue(dataset["reference_game"])

    def test_joins_game_species_names_to_source_types_and_rejects_unknown_types(self):
        entries = MODULE.parse_species_reference(
            'val speciesNamesRowe = mapOf(25 to "Pikachu", 26 to "Raichu")',
            "speciesNamesRowe",
            "val speciesTypeDataRowe = mapOf(25 to SpeciesTypes(13, 13), 26 to SpeciesTypes(13, 13))",
            "speciesTypeDataRowe",
            {1, 13},
        )
        self.assertEqual(entries, [
            {"number": 25, "name": "Pikachu", "types": [13]},
            {"number": 26, "name": "Raichu", "types": [13]},
        ])
        with self.assertRaises(ValueError):
            MODULE.parse_species_reference(
                'val names = mapOf(1 to "Bulbasaur")', "names",
                "val types = mapOf(1 to SpeciesTypes(99, 99))", "types", {12, 3},
            )
        self.assertEqual(
            MODULE.parse_species_reference(
                'val names = mapOf(1 to "Form")', "names",
                "val types = mapOf(1 to SpeciesTypes(99, 99))", "types", {12, 3}, strict_type_ids=False,
            ),
            [{"number": 1, "name": "Form", "types": []}],
        )


if __name__ == "__main__":
    unittest.main()
