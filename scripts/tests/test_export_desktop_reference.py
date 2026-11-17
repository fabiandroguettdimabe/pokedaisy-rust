import importlib.util
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


if __name__ == "__main__":
    unittest.main()
