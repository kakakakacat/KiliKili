# Regression CI Implementation Plan

> **For agentic workers:** Implement inline; verify each job and its trigger before declaring the gate complete.

**Goal:** Make every PR run the existing upstream-behavior comparisons, Rust checks, real FFI tests, and an Android package smoke check through one required status.

**Architecture:** A single `ci.yml` owns PR, main, nightly, and manual triggers. Existing Rust and Flutter workflows become reusable jobs. A new bridge test exercises a typed Rust network path with a local fixture server. Android builds an unsigned development APK and checks that the Rust library is packaged.

**Tech Stack:** GitHub Actions, Flutter test, Cargo, Python artifact inspector.

---

### Task 1: Make existing checks reusable

**Files:** `.github/workflows/rust.yml`, `.github/workflows/flutter-test.yml`

- [ ] Keep their existing jobs and commands.
- [ ] Replace selective PR triggers with `workflow_call` and manual dispatch.
- [ ] Confirm callers can refer to both workflows from the unified CI.

### Task 2: Cover a typed Rust path across the FFI

**Files:** `test/bridge/search_bridge_test.dart`

- [ ] Serve a committed search fixture from a loopback HTTP server.
- [ ] Call generated `searchTrending` with a local `SearchContext`.
- [ ] Assert a real HTTP request arrived, result fields match the frozen baseline, and malformed responses return a protocol error.
- [ ] Run the bridge suite with a built native library when Flutter is available.

### Task 3: Build one automatic PR gate

**Files:** `.github/workflows/ci.yml`

- [ ] Trigger on every PR, push to main, nightly schedule, and manual dispatch.
- [ ] Call reusable Rust and Flutter checks.
- [ ] Build an Android APK and inspect it for `libbridge_flutter.so`.
- [ ] Add a final job that fails if any dependency failed, was skipped, or was cancelled.

### Task 4: Document coverage and verify

**Files:** `docs/qa/RELEASE-VALIDATION.md`

- [ ] Explain the automatic gate and remaining device checks.
- [ ] Validate workflow syntax and inspect the diff.
- [ ] Run local checks that the available toolchain supports; report unavailable Flutter verification explicitly.
