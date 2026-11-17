import importlib.util
import json
import unittest

ROOT = __import__("pathlib").Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "export_desktop_dex", ROOT / "scripts" / "export_desktop_dex.py"
)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

class ExportDesktopDexTests(unittest.TestCase):
    def test_pinned_regional_pokedex_export_preserves_both_numbering_systems_and_generation(self):
        species = [
            {"number": 25, "name": "Pikachu", "generation": 1},
            {"number": 26, "name": "Raichu", "generation": 1},
        ]
        dexes = "id,region_id,identifier,is_main_series\n1,,national,1\n2,1,kanto,1\n3,1,unofficial,0\n"
        dex_numbers = "species_id,pokedex_id,pokedex_number\n25,2,1\n26,2,2\n"
        version_regions = "version_group_id,region_id\n1,1\n2,1\n"
        version_groups = "id,identifier,generation_id,order\n1,red-blue,1,1\n2,yellow,1,2\n"
        regions = "id,identifier\n1,kanto\n"
        result = MODULE.parse_regional_dexes(
            dexes, dex_numbers, version_regions, version_groups, regions, species
        )
        self.assertEqual(result, [{
            "dataset_id": "pokeapi-pokedex-2",
            "identifier": "kanto",
            "name": "Kanto",
            "generation": 1,
            "species_count": 2,
            "species": [
                {"number": 1, "national_number": 25, "name": "Pikachu", "generation": 1},
                {"number": 2, "national_number": 26, "name": "Raichu", "generation": 1},
            ],
        }])

    def test_regional_pokedex_export_rejects_duplicate_numbers_and_unknown_species(self):
        dexes = "id,region_id,identifier,is_main_series\n2,1,kanto,1\n"
        version_regions = "version_group_id,region_id\n1,1\n"
        version_groups = "id,identifier,generation_id,order\n1,red-blue,1,1\n"
        regions = "id,identifier\n1,kanto\n"
        species = [{"number": 25, "name": "Pikachu", "generation": 1}]
        with self.assertRaisesRegex(ValueError, "duplicate regional number"):
            MODULE.parse_regional_dexes(
                dexes, "species_id,pokedex_id,pokedex_number\n25,2,1\n25,2,1\n",
                version_regions, version_groups, regions, species,
            )
        with self.assertRaisesRegex(ValueError, "unknown National Dex species"):
            MODULE.parse_regional_dexes(
                dexes, "species_id,pokedex_id,pokedex_number\n9999,2,1\n",
                version_regions, version_groups, regions, species,
            )
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

    def test_gen3_mapping_preserves_internal_species_ids_and_rejects_truncation(self):
        source = "speciesNationalDex: IntArray = intArrayOf(" + ",".join(map(str, range(412))) + ")"
        mapping = MODULE.parse_gen3_species_dex(source)
        self.assertEqual(len(mapping), 412)
        self.assertEqual(mapping[0], 0)
        self.assertEqual(mapping[25], 25)
        with self.assertRaises(ValueError):
            MODULE.parse_gen3_species_dex("speciesNationalDex: IntArray = intArrayOf(0, 1)")

    def test_gen3_item_names_reject_duplicate_ids_and_keep_game_text(self):
        self.assertEqual(MODULE.parse_item_names('val itemNamesFireRedGame: Map<Int, String> = mapOf(\n1 to "MASTER BALL",\n2 to "POTION"\n)'), {"1": "MASTER BALL", "2": "POTION"})
        with self.assertRaises(ValueError):
            MODULE.parse_item_names('val itemNamesFireRedGame: Map<Int, String> = mapOf(\n1 to "MASTER BALL",\n1 to "DUPLICATE"\n)')

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
        self.assertEqual(len(dataset["gen3_species_map"]), 412)
        self.assertEqual(dataset["gen3_species_map"][277], 252)
        self.assertEqual(dataset["gen3_species_source_sha256"], MODULE.hashlib.sha256(
            (ROOT / "app/src/main/kotlin/com/pokedaisy/app/companion/data/save/SpeciesDexGen.kt").read_bytes()
        ).hexdigest())
        self.assertEqual(dataset["gen3_item_names"]["13"], "POTION")
        self.assertEqual(dataset["gen3_item_source_sha256"], MODULE.hashlib.sha256(
            (ROOT / "app/src/main/kotlin/com/pokedaisy/app/companion/data/ItemTextGame.kt").read_bytes()
        ).hexdigest())

    def test_shipped_dataset_includes_pinned_regional_dexes_with_provenance(self):
        dataset = json.loads(MODULE.OUTPUT.read_text(encoding="utf-8"))
        by_id = {item["dataset_id"]: item for item in dataset["regional_dexes"]}
        kanto = by_id["pokeapi-pokedex-2"]
        self.assertEqual((kanto["name"], kanto["generation"], kanto["species_count"]), ("Kanto", 1, 151))
        self.assertEqual(kanto["species"][24], {
            "number": 25, "national_number": 25, "name": "Pikachu", "generation": 1,
        })
        self.assertEqual(by_id["pokeapi-pokedex-15"]["species_count"], 211)
        self.assertEqual(by_id["pokeapi-pokedex-15"]["generation"], 6)
        self.assertEqual(by_id["pokeapi-pokedex-26"]["generation"], 7)
        self.assertEqual(by_id["pokeapi-pokedex-34"]["generation"], 9)
        self.assertEqual(len(dataset["regional_dex_source_sha256"]), 64)
        self.assertEqual(dataset["regional_dex_source_commit"], "7764ee08de86ac11031ae2b883b598599af2b960")
        self.assertEqual(kanto["species_license"], "BSD-3-Clause")
        self.assertIn("no garantiza disponibilidad exacta", kanto["species_scope"])


if __name__ == "__main__":
    unittest.main()
