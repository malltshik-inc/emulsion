# TODO — bugs & improvements (next session)

Each item has the observed behaviour, the code seam, and a proposed acceptance
criterion. Product rules: `docs/PRODUCT.md`; internals: `docs/ARCH.md`.

## 1. File browser: per-file thumbnails, not paths

**Bug:** the file panel shows a single "selected image" preview pane at the top
(`emulsion-app/src/main.rs` `preview_pane`, ~line 264) and the list is just
buttons of full paths (~line 254). We don't want the selected-image preview; we
want a **miniature of each file in the folder** rendered inside the list entry
instead of the path.

**Where:** `emulsion-app/src/main.rs` `file_panel` + `preview_pane`.

**Acceptance:** open a folder with mixed JPEG/PNG/TIFF/ORF; each list row shows
a small thumbnail (plus filename), no separate preview pane above the list.
Thumbnails decode once per file, cached; ORF rows show a placeholder while
decoding (don't block the UI).

## 2. Pan must only work on the canvas

**Bug:** image drag-and-pan is triggered from anywhere — including while moving
settings sliders. `canvas()` pans on any `primary_down() || middle_down()`
without checking the pointer is over the canvas.

**Where:** `emulsion-app/src/main.rs` `canvas` (~line 420).

**Acceptance:** dragging in the central canvas pans; dragging sliders/buttons in
the tools or file panels never pans. Pan only when the canvas is hovered (e.g.
`response.hovered()` / `response.dragged()`).

## 3. Exposure slider direction is inverted

**Bug:** `+0.90` darkens, `-0.90` brightens. Root cause: `develop()` applies
exposure **before** `invert()` (`emulsion-core/src/tonemap.rs`), so a gain on
the negative looks like the opposite on the positive. PRODUCT.md §7 defines
exposure +3..-3 EV with the natural expectation that **+ = brighter**.

**Where:** `emulsion-core/src/tonemap.rs` `develop` ordering; `colorops::apply_exposure`.

**Acceptance:** with a negative flipped positive, `+exposure` makes the image
brighter and `-exposure` darker, monotonic. (Fix by applying exposure after
invert, or flipping the sign applied to the pre-invert negative — keep tests.)

## 4. Converted negatives too bright; shadows fall apart into black + artifacts

**Bug:** after inversion the image is too bright; lowering exposure crushes
shadows to pure black with artifacts around them. On the sky/tree/sakura shot,
making the sky look realistic pushes all shadows (even slight ones) to solid
black — yet at a brighter exposure the detail is there.

**Symptoms point to:** pure linear `invert` (`1 - x`) + sRGB encode with a hard
`clamp(0,1)` and no shoulder/toe (`emulsion-core/src/tonemap.rs`
`tonemap_and_quantize`). The negative's bright end (positive shadows) is
compressed by sRGB gamma and clipped, so there is no gradation to the shadows —
they band and crush. Auto black-point stretch in decode/demosaic interacts too.

**Where:** `emulsion-core/src/tonemap.rs`; `emulsion-core/src/colorops.rs`.

**Acceptance:** the sakura sky/tree shot keeps visible gradation in the darker
tree/shadow areas while the sky reads as a realistic colour — no solid-black
crush with artifacts on slight shadows. A core unit test drives a synthetic
ramp through develop and asserts monotonic, non-crushed output in the shadow
third. Likely needs a film-style tone curve (toe/shoulder) or a gentler
inversion with headroom before sRGB encode — scope in the session before coding.
