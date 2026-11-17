# PokeAPI species and regional Dex snapshot

These CSV files are a pinned offline snapshot from `PokeAPI/pokeapi`, commit
`7764ee08de86ac11031ae2b883b598599af2b960`, under the included BSD 3-Clause
license. The type-profile exporter uses default Pokémon records, current type
rows, historical type rows, species introduction generations, and type names.
It joins those records to the project's National Dex name table by species ID.
The regional Dex exporter also uses `pokedexes.csv`, `pokemon_dex_numbers.csv`,
`version_group_regions.csv`, `version_groups.csv`, and `regions.csv` to publish
the main-series regional lists with regional and National Dex numbers.
The form index joins `pokemon.csv` and `pokemon_forms.csv` to include alternate
form names, introduction generations, and battle-only/Mega flags. These records
do not assert that a form appears in every title or hack containing that species.
Each list generation follows its version group where a later regional update
shares an earlier region (for example, Updated Hoenn uses Omega Ruby/Alpha
Sapphire and Let's Go Kanto uses Let's Go Pikachu/Eevee).
Five regional lists with an explicit PokeAPI mapping also retain their `version_group`
identifier in the exported dataset. Other regional lists do not currently carry
a game-group mapping, so the field is omitted instead of inferred from region
or generation.

To refresh intentionally, select a new upstream commit, replace these CSVs and
`SOURCE_COMMIT.txt` together, inspect the license, then regenerate and review
`desktop-rust/ui/src/data/type-charts-reference.json` and
`desktop-rust/ui/src/data/pokedex-gen9.json`.

Scope: type profiles cover default forms and species introduced through each
generation. Regional lists follow the main-series PokeAPI Pokédex records but do
not guarantee exact availability per game revision or cover alternate forms.
