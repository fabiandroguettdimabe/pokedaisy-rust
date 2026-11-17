# PokeAPI type data snapshot

These CSV files are a pinned offline snapshot from `PokeAPI/pokeapi`, commit
`7764ee08de86ac11031ae2b883b598599af2b960`, under the included BSD 3-Clause
license. The type-profile exporter uses default Pokémon records, current type
rows, historical type rows, species introduction generations, and type names.
It joins those records to the project's National Dex name table by species ID.

To refresh intentionally, select a new upstream commit, replace these CSVs and
`SOURCE_COMMIT.txt` together, inspect the license, then regenerate and review
`desktop-rust/ui/src/data/type-charts-reference.json`.

Scope: default forms and species introduced through each generation. This does
not claim game-specific regional availability or cover alternate/regional forms.
