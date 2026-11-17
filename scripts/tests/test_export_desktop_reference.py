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

    def test_exports_versioned_yellow_crystal_and_glazed_charts(self):
        data = json.loads(MODULE.render_additional_datasets())
        yellow, crystal, glazed = data["datasets"]
        self.assertEqual((yellow["generation"], yellow["reference_game"]), (1, "Pokémon Yellow"))
        self.assertEqual((crystal["generation"], crystal["reference_game"]), (2, "Pokémon Crystal"))
        self.assertEqual(yellow["license"], "GPL-3.0-only")
        self.assertIn({"attack": 8, "defense": 24, "percent": 0}, yellow["matchups"])
        self.assertIn({"attack": 27, "defense": 24, "percent": 200}, crystal["matchups"])
        self.assertIn({"id": 9, "name": "STEEL"}, crystal["types"])
        self.assertEqual((glazed["generation"], glazed["reference_game"]), (6, "Pokémon Glazed 9.2.0"))
        self.assertIn({"id": 9, "name": "Fairy"}, glazed["types"])
        self.assertIn({"attack": 9, "defense": 16, "percent": 200}, glazed["matchups"])


if __name__ == "__main__":
    unittest.main()
