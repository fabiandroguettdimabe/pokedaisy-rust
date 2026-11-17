# Regional Dex Implementation Plan

**Goal:** Add attributed, offline regional Pokédex lists to the Desktop Dex using the repository's pinned PokeAPI revision.

**Architecture:** Extend the existing Dex exporter with the PokeAPI `pokedexes.csv` and `pokemon_dex_numbers.csv` inputs. Store regional lists separately from the National Dex in the existing generated Dex asset, then expose them as browse-only sources in Companion; type data remains manual unless a compatible chart dataset supplies it.

**Tech Stack:** Python standard library exporter/tests; generated JSON; React UI; Node test runner and Playwright.

**Spec:** `docs/VYNTRA_PLAN_TECNICO.md`, T15 CMP-DEX/BATTLE.

## Tasks

1. Add pinned source CSVs and exporter tests for regional identity, contiguous numbering, duplicate rejection, attribution, and data hash.
2. Export regional Dex lists with stable IDs, regional numbers, National Dex numbers, names, and generation; verify with `--check`.
3. Add regional Dex sources to Companion, preserve separate IDs, and leave battle types unasserted where unavailable.
4. Add UI coverage for Kanto and a later regional list; update T15 scope and verification, run exporter/UI checks, commit and push.
