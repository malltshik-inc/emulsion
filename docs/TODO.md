# TODO - bugs & improvements (next session)

Each item has the observed behaviour, the code seam, and a proposed acceptance
criterion. Product rules: `docs/PRODUCT.md`; internals: `docs/ARCH.md`.

## 1. TOP: negative view clips to pure white when "Develop film" is off

**Observed (owner screenshots, `_AGP7722.ARW`, WB 80 / EV +2.25 / contrast 35 /
saturation 35):** with **Develop film ON** the positive is good; with it **OFF**
the canvas and thumbnail are almost pure white - no readable negative. The owner
expects both directions to stay readable across toggles, as in RawTherapee.

**Cause:** the positive look is applied directly to the negative display.
Exposure multiplies linear light, and the demosaic auto-scale has already put the
film base near 1.0; +2.25 EV multiplies that to ~4.8 and every frame area clips
to white. It is not a decode/CFA problem.

**Correct model:** sliders describe the *positive* result, so the negative view
must show the inverse adjustment: +EV makes the negative **darker**, +contrast
makes it **flatter**, +saturation makes it **less** chromatic, and +WB (warm)
makes the negative **cooler**. Today all of these act on the negative as if it
were a positive.

**Where:** `emulsion-core/src/tonemap.rs` (`develop`), `emulsion-app/src/main.rs`
(file selection / render call).

**Proposed approach:** separate "this source is a film negative" from "display
it inverted". Track negative-ness per file when Develop is first enabled. For a
negative source with invert off, render the positive pipeline and invert the
encoded result again for display (or apply the algebraic inverse per op). Do not
zero or hide the sliders - they must still describe the positive.

**Acceptance:** with EV +2.25 / contrast +35 / saturation +35, toggling Develop
off shows a readable negative (pavement and people visible, no white clipping)
and toggling back reproduces the exact same positive; an ordinary positive scan
(e.g. JPEG) is unchanged; unit tests cover the round trip.

## 2. DONE: Develop baseline

The negative→positive conversion now adds a fixed baseline on top of the sliders
(exposure +1.4 EV, WB temperature +80, contrast +82, saturation +20) so a neutral
scan converts to a usable positive on the first Develop click instead of a dark,
pale one. Chosen over a visible preset (which would set the slider values when
Develop is enabled) because the sliders then stay at 0 = baseline and the
positive (non-inverted) path is untouched. Covered by
`develop_defaults_apply_conversion_baseline`; see PRODUCT §7 and ARCH §3.

**Still open:** auto exposure from the inverted luma distribution for very
thin/under-exposed negatives (the baseline is fixed, not adaptive), and a
suggested WB beyond the fixed +80.

**Reference:** a Gemini post-mortem the owner supplied emphasises film-base /
percentile normalization (we already do this in `robust_bounds`), logarithmic
density inversion, an exposure lift for under-exposed negatives, and an S-curve
before gamma. Useful as confirmation; its LibRaw C dependency and auto-crop are
not adopted (pure-Rust constraint, crop is deferred).

## 3. Per-file settings are session-only

**State:** `EmulsionApp::settings` keys params by absolute path and restores them
on selection. Closing the app loses all edits.

**Where:** `emulsion-app/src/main.rs` (`select_file`, `settings`).

**Acceptance:** reopening a folder restores each file's last settings (patch-file
autosave, PRODUCT sec. 10), with a neutral fallback for files never edited.

## 4. Precision wheel control for sliders (deferred)

**State:** a plain hover-wheel changed slider values, but one notch moved the
range too far (felt destructive), so it was removed. Sliders are drag + type +
double-click reset again.

**Idea:** bring wheel back only with a modifier (Ctrl+wheel = fine step,
Shift+wheel = coarse) or with a genuine line-unit delta from
`Event::MouseWheel`, not the point-scaled smooth delta.

**Where:** `emulsion-app/src/main.rs` (`param_slider`).

**Acceptance:** wheel never moves a slider without the modifier; one notch with
the modifier is a small, predictable change (<=0.5% of the range), and the canvas
zoom/panel scroll are unaffected.

## 5. Camera colour matrix not applied

**Observed:** `emulsion-core` applies no camera->sRGB transform; colour is the
sensor/demosaic response after per-channel auto-scale. An experiment using
`raw.xyz_to_cam` rendered black; the cause (empty matrix vs. misuse) was not
isolated, and the rawler reference pipeline it was compared against produced
casts/ghosting, so the experiment was shelved.

**Where:** `emulsion-core/src/decode.rs`, `demosaic.rs`.

**Acceptance:** if implemented, use `raw.color_matrix` (D65 entry) and prove on
the real ORF/ARW that skin/sky do not shift hue against the current output;
otherwise document the omission explicitly.
