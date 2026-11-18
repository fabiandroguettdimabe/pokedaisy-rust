# Vault Invalid File E2E Implementation Plan

> **For agentic workers:** Work through the checked steps in order. Keep the test isolated from any already-open user Launcher.

**Goal:** Verify that selecting an invalid Vault ZIP in the native Windows file picker produces a correlated terminal error visible in the real Launcher UI.

**Architecture:** Add a standalone PowerShell E2E smoke that starts a separate Launcher process with a temporary data root and WebView2 profile. Playwright opens Vault and starts the import command; Windows UI Automation selects an invalid ZIP in the native dialog; Playwright then verifies the accessible alert. No production behavior changes are expected.

**Tech Stack:** PowerShell, Windows UI Automation, WebView2 remote debugging, Playwright Core, existing Vyntra Desktop executable.

**Spec:** `docs/VYNTRA_PLAN_TECNICO.md`, T02 / INFRA-01.

## Global Constraints

- Preserve the running user Launcher and its data.
- Use an isolated temporary data root and WebView2 profile for the smoke.
- Do not claim T02 complete; other action messages still need review.
- Do not use ROMs or core DLLs for this Vault import failure case.

## Review Focus

- Native picker cannot be found or automated: fail with useful dialog diagnostics and clean up the smoke process.
- Invalid file is not selected: verify the picker actually receives the fixture path before accepting any alert.
- Backend error fails to reach UI: require an accessible alert with the invalid archive reason and close affordance.
- Existing app process is accidentally targeted: scope native dialog lookup to the smoke Launcher PID.
- WebView2 endpoint or page is absent: fail before interacting with the native dialog.

### Task 1: Add isolated Launcher Vault import smoke

**Files:**
- Create: `desktop-rust/scripts/test-launcher-vault-invalid-e2e.ps1`
- Modify: `docs/VYNTRA_PLAN_TECNICO.md`
- Modify: `docs/VYNTRA_IMPLEMENTATION_STATUS.md`
- Modify: `docs/vyntra-capabilities.md`

**Interfaces:**
- Input: launcher executable path and optional temporary data root.
- Output: a `PASS` record naming the real UI route, invalid fixture, surfaced alert, and launcher PID.

- [x] **Step 1: Write the standalone smoke script**

  Start an isolated Launcher, navigate to Vault through Playwright/CDP, click `Importar paquete`, select a generated invalid ZIP in the Windows native dialog scoped to that process, assert the visible accessible alert, then close the temporary Launcher in `finally`.

- [x] **Step 2: Run the script against the current implementation**

  Run: `pwsh -File desktop-rust/scripts/test-launcher-vault-invalid-e2e.ps1 -LauncherExe <fresh-test-build-exe>`
  Expected: `PASS`, error alert from invalid ZIP, exit code 0; no user Launcher process is modified.

- [x] **Step 3: Fix only harness issues and rerun**

  Keep changes test-only unless the real flow exposes a production defect; if it does, add a failing assertion/test before the smallest fix.

- [x] **Step 4: Update evidence and publish the milestone**

  Record exact command/result and remaining T02 gaps, run `git diff --check`, then commit and push only these milestone files.
