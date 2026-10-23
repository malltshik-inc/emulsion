# AGENTS.md

Repo guidance for Emulsion (and its docs set). Keep this file short — read the
docs on demand, don't inline them.

## Overview

Emulsion is a desktop photo editor that converts negative film scans to positive
images. Simple UI, precise color. Rust edition 2024,
egui 0.37 GUI, image-rs + rawler for I/O, rayon for pixel parallelism. Windows,
macOS and Linux (fonts are bundled; no OS-specific asset paths). Two crates:
`emulsion-core` (pure pipeline, no GUI) and `emulsion-app` (egui frontend).

**Product rules live in `docs/PRODUCT.md`** (normative). Read it before touching
behavior. **Technical truth lives in `docs/ARCH.md`** (descriptive) — read it before
touching pipeline/UI internals; it records the gotchas and limitations too.

## Docs structure

| file | type | about |
|---|---|---|
| `docs/PRODUCT.md` | normative | what the product does, controls, export, acceptance |
| `docs/ARCH.md` | descriptive | how it's built, data structures, egui patterns, known bugs/limits |
| `docs/CODEX.md` | normative, upstream | the 12 precepts — binds every agent, beats AGENTS.md |
| `docs/METHODS.md` | recommendative | defaults; repository wins over it |

Rules from CODEX/METHODS are the floor; this file only adds what is Emulsion-specific.

## Main rules

- Follow `docs/CODEX.md` first. It wins on conflict.
- 37 core tests give the baseline; keep every op unit-tested (`cargo test`).
- `cargo fmt` and `cargo clippy` must stay clean.
- `emulsion-core` stays pure Rust, no GUI/IO-glue deps, no C deps, fully testable.
- The app is one pure call: `process(source, params) -> image`. Edits are params only.
- Never commit unless explicitly asked. Never touch secrets.
- Keep README + AGENTS short; put depth in `docs/`. Update ARCH whenever you change
  a gotcha or the module map; update PRODUCT when behavior changes.

## General anti-pattern

**Proxy results for real ones.** "It should work" is not a run. A green build is not a
test. A passing unit test is not the ORF opening. On Windows, actually run
`cargo test`, build the app, launch `emulsion-app.exe`, open a real ORF, export, and
report what was observed — including the negative case (what failed loudly).

## Way of working

1. **Destination first** (codex §1): state the goal + acceptance before touching code.
   When unsure, ask (your options + consequences).
2. **Discover, don't invent** (codex §2): read `PRODUCT.md`/`ARCH.md` for behavior
   and internals; read `CODEX.md` for rules; grep the code for the nearest
   convention. Ask rather than guess.
3. **Smallest change** (codex §9):
   - extend `emulsion-core` ops + tests, not new abstractions;
   - UI is a thin egui shell over the single pure call;
   - if a change touches both crates, land core first (tests green), then app.
4. **Verify against the product** (codex §4–7): each acceptance criterion gets a
   runnable signal — a unit test, or a manual repro line, or an explicit
   "cannot signal, needs human" note with a named owner in the change description.
5. **Leave it operable** (codex §12): no dead code, no silent behavior changes;
   update `docs/` in the same change.