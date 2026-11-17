# R13: shared session snapshot contract

## Intent

Reduce Rust/Kotlin protocol drift by defining one versioned JSON contract and
using the same golden fixture in both test suites. This is a compatibility
foundation; it does not connect Android to the Desktop Player.

## Scope

- Define `contracts/vyntra/v1/session-snapshot.schema.json`.
- Add canonical v1 and incompatible-major fixtures.
- Make Rust publish validation reject unsupported major versions.
- Add a Kotlin decoder with the same required fields and version behavior.
- Test both implementations against the shared fixtures.

## Contract

The envelope has `protocol_version`, `session_id`, `epoch`, `revision`, and an
object-valued `payload`. Version 1 is supported. Missing/invalid required
fields and unsupported versions are rejected. Unknown fields are tolerated for
forward-compatible optional additions; an incompatible major requires an
explicit future implementation.

## Approaches

- Duplicate language-local fixtures: rejected because they can drift silently.
- Generate Rust/Kotlin types from a schema: deferred because the Android and
  Desktop build chains do not share a generator and there is no live channel.
- Shared schema and golden JSON decoded by both languages: selected for a
  small, reviewable first compatibility gate.

## Verification

Run the Rust desktop suite and Android unit test task if the local toolchain
permits it. Validate the JSON Schema and fixture shape independently. Any
NDK-license block remains external and must be reported accurately.
