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


class ExportDesktopReferenceTests(unittest.TestCase):
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
        self.assertEqual(len(by_id), 13)
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
