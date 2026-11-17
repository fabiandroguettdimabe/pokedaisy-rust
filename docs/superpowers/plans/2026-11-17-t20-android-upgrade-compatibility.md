# T20 Android Upgrade and Storage Compatibility Plan

> **For agentic workers:** Keep application identity and existing saves readable; validate in stages.

**Goal:** Protect Android install identity and legacy ROM-derived save/state identity while making storage permission recovery verifiable.

**Architecture:** Preserve Gradle `applicationId` and CRC32 path scheme. Add golden coverage for known CRC behavior; inspect SAF/manifest flow. Device validation must install the previous signed build, revoke grants, upgrade, and confirm ROM references and saves survive.

**Tech Stack:** Kotlin/JUnit, Android Gradle Plugin, Android SAF, existing `SaveStates`/`RomIntake`.

**Spec:** `docs/VYNTRA_PLAN_TECNICO.md`, AND-09/01/05 and T20.

## Global Constraints

- Keep `applicationId = com.pokedaisy.app` for in-place upgrades.
- Keep legacy CRC32-derived save/state paths readable.
- Never delete or rewrite ROM/save files during permission recovery.
- Do not claim upgrade/signature/lifecycle acceptance without an Android device or emulator.

## Review Focus

- Known ROM bytes retain the historic CRC32 directory key.
- Renaming the same ROM does not change its CRC key.
- Revoked storage access preserves indexed games and reports reauthorization.
- Upgrade with same signer preserves package data; a different signer fails safely.
- Process recreation and lifecycle transitions do not start duplicate engines.

## Tasks

- [x] Add golden tests for the known CRC32 compatibility value and filename-independent identity.
- [x] Add small deterministic tests for SAF byte-copy limits: exact-cap acceptance and overflow rejection before writing the oversized chunk.
- [x] Keep the POSIX Gradle wrapper on LF across Windows checkouts via `.gitattributes`.
- [x] Recheck current package/version/manifest and storage recovery flow.
- [x] Prepare a temporary JDK 17, Gradle 8.10.2, and checksum-verified Android command-line tools outside the repository.
- [ ] Obtain authorization for the NDK 28.2.13676358 SDK license, install required SDK packages, and run Android tests; `:app:testDebugUnitTest` currently stops at `LicenceNotAcceptedException`.
- [ ] Validate upgrade/signature, SAF permission revocation/recovery, and lifecycle with an Android device or emulator.
- [x] Mark T20 partial if device/signature/SAF validation remains unavailable; commit and push.

