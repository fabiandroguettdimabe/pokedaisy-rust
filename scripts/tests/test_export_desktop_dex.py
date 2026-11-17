import importlib.util
import unittest

ROOT = __import__("pathlib").Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "export_desktop_dex", ROOT / "scripts" / "export_desktop_dex.py"
)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ExportDesktopDexTests(unittest.TestCase):
    def test_exports_contiguous_species_and_generation_from_national_numbers(self):
        source = '''val speciesNamesNationalDex: Map<Int, String> = mapOf(
            1 to "Bulbasaur", 2 to "Ivysaur", 3 to "Venusaur", 4 to "Charmander")'''
        entries = MODULE.parse_species_names(source, max_dex=4)
        self.assertEqual(entries, [
            {"number": 1, "name": "Bulbasaur", "generation": 1},
            {"number": 2, "name": "Ivysaur", "generation": 1},
            {"number": 3, "name": "Venusaur", "generation": 1},
            {"number": 4, "name": "Charmander", "generation": 1},
        ])

    def test_rejects_missing_or_duplicate_dex_numbers(self):
        with self.assertRaises(ValueError):
            MODULE.parse_species_names('mapOf(1 to "Bulbasaur", 3 to "Venusaur")', max_dex=3)
        with self.assertRaises(ValueError):
            MODULE.parse_species_names('mapOf(1 to "Bulbasaur", 1 to "Duplicate")', max_dex=1)

    def test_shipped_dataset_covers_all_generation_boundaries_and_source_hash(self):
        source = (ROOT / "app/src/main/kotlin/com/pokedaisy/app/companion/data/SpeciesNamesNationalDex.kt").read_bytes()
        dataset = MODULE.build_dataset(source)
        self.assertEqual(dataset["dataset_id"], "pokemon-national-dex-gen9")
        self.assertEqual(dataset["max_generation"], 9)
        self.assertEqual(dataset["species_count"], 1025)
        self.assertEqual(dataset["source_sha256"], MODULE.hashlib.sha256(source).hexdigest())
        self.assertEqual(
            [(dataset["species"][number - 1]["number"], dataset["species"][number - 1]["generation"])
             for number in (151, 152, 251, 252, 386, 387, 493, 494, 649, 650, 721, 722, 809, 810, 905, 906, 1025)],
            [(151, 1), (152, 2), (251, 2), (252, 3), (386, 3), (387, 4), (493, 4), (494, 5),
             (649, 5), (650, 6), (721, 6), (722, 7), (809, 7), (810, 8), (905, 8), (906, 9), (1025, 9)],
        )
        self.assertEqual(dataset["species"][-1]["name"], "Pecharunt")


if __name__ == "__main__":
    unittest.main()
