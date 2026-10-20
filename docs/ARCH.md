# Architecture — technical reference

> **Type: descriptive.** This file explains how Emulsion is built. It is not a spec
> (that is `PRODUCT.md`) and not a rulebook (that is `CODEX.md`). It records the current
> design and, just as importantly, every known bug, gotcha and limitation, so that the
> next change starts from facts instead of rediscovery.

---

## 1. Overview

Emulsion is a desktop app that converts negative film scans into positive images.
Two crates in one Cargo workspace:

```
emulsion-core/   pure Rust pixel pipeline — zero GUI deps, fully unit-testable
emulsion-app/    egui frontend — opens files, runs the pipeline, draws + exports
```

Everything the product does is one pure function in `emulsion-core`:

```
process(source_path, params: PipelineParams) -> RgbaImage
```

The app never mutates image state. The user edits a `PipelineParams` struct; the
pipeline re-runs from the original source. This makes undo, presets and patch files
trivially expressible as "a saved params struct" (see `PRODUCT.md` for the product
meaning; the mechanism is one serde-JSON struct).

No C dependencies are ever linked. `emulsion-core` is pure Rust everywhere.

---

## 2. Crate layout

### emulsion-core (`emulsion-core/src/`)

| module | responsibility |
|---|---|
| `lib.rs` | `process(source, params) -> Result<RgbaImage>` — the one pure entry point; re-exports public types |
| `params.rs` | `PipelineParams` + `CropRect` — serde `Serialize/Deserialize`, `Default` impl |
| `image.rs` | `RgbaImage` (RGBA8 interleaved `Vec<u8>`) and `LinearImage` (flat RGB interleaved `Vec<f32>`), both `(width, height, data)` |
| `decode.rs` | `decode(path) -> Result<LinearImage>` — RAW via `rawler`, everything else via `image` crate (de-gamma'd to linear) |
| `demosaic.rs` | `demosaic_bilinear(width, height, bayer: &[u16], cfa) -> LinearImage` + `robust_bounds` — robust 0.25%/99.75% percentile per-channel auto-scale |
| `colorops.rs` | `wb_gains`, `apply_wb`, `apply_exposure`, `apply_black_point`, `invert` (rayon-parallel, in place) |
| `geometry.rs` | `crop`, `rotate_90`, `rotate_fine` (bilinear, white fill), `downscale` (box average) |
| `tonemap.rs` | `develop(base, params) -> RgbaImage` — the interactive render path (WB→exposure→black-point→invert→crop/rotate→sRGB) |
| `export.rs` | `save_png`, `save_jpeg` (quality 1..100) |
| `error.rs` | `CoreError`, `Result<T>` = `std::result::Result<T, CoreError>` |
| `tests.rs` | 19 unit tests, one per op (incl. robust-bounds outlier tests) |

### emulsion-app (`emulsion-app/src/main.rs`)

Single-file egui app (intentionally; split when it earns it — see Gotcha 9).

State held by `EmulsionApp`:

| field | role |
|---|---|
| `folder`, `files`, `cursor` | flat recursive image list of the opened folder |
| `loaded: Option<Loaded>` | preview base (`LinearImage`, downscaled to ≤1800 px longest edge) |
| `load_rx` | mpsc `Receiver` for the background decode result |
| `params: PipelineParams` | the live editing state ("the patch") |
| `search: String` | tools-panel filter box text |
| `texture`, `texture_size` | egui GPU texture of the developed preview |
| `zoom`, `pan` | canvas transform |
| `status`, `dirty` | status line + re-render flag |

Workflow in `App::ui` each frame:
1. `poll_load()` — drain the decode channel if done.
2. `sync_texture()` — if `dirty`, `develop(base_small, params)` → upload RGB(A8) bytes to texture.
3. Build panels (top bar, left file list, right tools, bottom status, central canvas);
   each panel calls its own method on `self` (keeps the egui closures single-capture).
4. Canvas: `allocate_painter` + `painter.image(texture_id, rect, uv, white)` with wheel-zoom / drag-pan (left or middle button). The file panel reuses the same texture, painted small, as its selection preview.

---

## 3. The pixel pipeline (in order)

```
source
  └ decode()                     rawler demosaic (ORF…) or image |→ sRGB-degamma → linear 0..1
      └ demosaic_bilinear        bayer → RGB; robust percentile auto-scale to *measured* range
          └ develop()            [interactive path, re-runs on every param edit]
              WB gains           temperature/tint → per-channel multiply
              exposure           2^EV multiply
              black point ×3     (v - bp) / (1 - bp), clamped [0, 0.98]
              invert             1 − v   (negative → positive)
              crop               fractional rect  (params.crop)
              rotate 90°         u8 0..3 quarter turns
              fine angle         bilinear around center, white fill outside
              sRGB gamma         0.00304 crossover / 12.92 / 1.055 / 2.4
              quantize           round → u8, alpha 255
```

Two decode paths:

- **RAW** (`.orf/.nef/.cr2/.cr3/.arw/.rw2/.raf/.pef/.srw/.dng/.erf/.kdc/.mrw/.mos/.dcr/.iiq`):
  `rawler::decode_file` → `pixels_u16()` + `camera.cfa` → our bilinear demosaic.
  rawler is used **only as the decoder**; we never touch its `imgop`/develop
  pipeline (which does exist and is a reference for what "correct" looks like).
- **Regular** (jpeg/png/tif): `image::ImageReader::open(...).decode()` → `into_rgba8()`
  → `srgb_decode` per sample → linear. TIFF is read as its 8-bit decode.

### Why measured auto-scale (the "a-ha" core)

The spike measured the owner's Olympus E-M5 Mark II high-res ORFs (9280×6932):
bayer data spans only **254..910** against a sensor white level of **4000** — the raw
scan uses ~¼ of the sensor's range. Nominal whitelevel scaling therefore produces a
nearly-black image. The demosaic instead computes per-channel **measured** bounds
from the actual Bayer samples and maps those to [0,1]. This single decision is what
makes the tool "just work" on dark scans, and the R/G/B black-point sliders give the
user manual control on top.

Since v0.1 the measured bounds are **robust percentiles** (0.25% / 99.75% per
channel, stride-sampled and sorted — `robust_bounds`), not raw min/max. A raw min/max
lets one hot or dead pixel stretch a channel's whole range, crushing everything else
into a dark sliver: the "image needs +0.8..+0.9 EV" and "shadows are pure black with
Bayer artifacts" symptom. The percentile bounds ignore a handful of outliers. The
auto stretch is baked into the demosaic's output, so the black-point sliders start at
0 meaning "auto baseline already applied"; surfacing the baseline as non-zero slider
starts would require the stretch to live in a params-level op with a per-channel
white point, which is not done yet (see Limitation 10).

---

## 4. Data representations (and why)

- **`LinearImage`** = one flat `Vec<f32>`, RGB interleaved (stride 3), linear light.
  A single interleaved buffer is deliberate: rayon safely parallelizes by iterating
  **chunks of the buffer being written** (`par_chunks_exact_mut`), and a flat pixel
  layout is the natural chunk unit. Per-plane buffers were tried; they fight rayon's
  ownership model (closures may not capture `&mut` to a captured `Vec`).
- **`RgbaImage`** = flat `Vec<u8>` RGBA — the display/export format.
- Memory cost at 64MP (9280×6932): linear f32 ≈ **772 MB**, RGBA8 ≈ **257 MB**,
  bayer u16 ≈ **129 MB**. Linear + RGBA coexist transiently during develop/upload.
  On a desktop this is fine; it is a documented limit (see Limitations 1).

---

## 5. Parallelism

`rayon 1.12` (the real data-parallel crate, stable) via `use rayon::prelude::*`.

Key idioms that work:
```
vec.par_chunks_exact_mut(3).for_each(|pix| …)          // mutate buffer being iterated
vec.par_chunks_exact_mut(w*3).enumerate().for_each(|(y,row)| …)  // row-parallel
(0..n).into_par_iter().for_each(|i| …)                 // index-parallel (read src, write dst)
vec.par_iter().map(..).reduce(|| identity, |a,b| ..)   // reductions — identity is a closure
```
Rules that must not be violated:
- The **mutated** buffer is the one you iterate. Read-only captures are shared `&`; that's fine.
- `reduce` requires an *identity closure* `|| -> T` (2006 stdlib renamed `reduce`; `ok_or`/`ok_or_else` are gone from `Result` — use `map_err`).
- `fold::<T>` turbofish form is wrong; plain `fold` with inferred type works in tests.

---

## 6. egui 0.37 frontend patterns

The app uses `eframe::App` (`fn ui(&mut self, ui, frame)`) + `eframe::run_native`.

Confirmed widget/API surface (versions pinned in `emulsion-app/Cargo.toml`):
```
Panel::top/left/right/bottom(id).exact_size/.default_size/.min_size.show(ui, |ui| …)  (no SidePanel/Side enum)
CentralPanel::default().show(ui, |ui| …)   — added LAST
Panel::bottom works too — used for the status line
ScrollArea::both().show(ui, |ui| …)  → ScrollAreaOutput<R> with .inner: R
ui.take_available_space()  — REQUIRED inside side panels to make them resizable
CollapsingHeader::new("Title").default_open(true).open(Some(true)|None).show(ui, |ui| …)
ui.collapsing("Title", |ui| …)  — starts collapsed
ui.text_edit_singleline(&mut String)  — TextEdit::singleline(..).hint_text(..).desired_width(..).show(ui)
ui.button("…").clicked(), ui.checkbox(&mut bool, "…").changed(), ui.heading(…), ui.label(…)
Slider::new(&mut f32, -100.0..=100.0).text("…").changed()
ui.horizontal(|ui| …)  — row layout
ColorImage::from_rgba_unmultiplied([w,h], &rgba) → ctx.load_texture(name, image, TextureOptions::LINEAR) → TextureHandle
handle.set(image, opts)   — cheap re-upload; TextureHandle Clone is ref-counted (cheap)
ui.allocate_painter(size, Sense::hover()) → (response, painter)
painter.image(tex.id(), rect, full_uv, Color32::WHITE)
painter.rect_filled(response.rect, 0.0, color); painter.text(pos, Align2::LEFT_TOP, s, FontId::proportional(13.0), Color32::WHITE)
ui.input(|i| i.smooth_scroll_delta().y)             — wheel
ui.input(|i| i.pointer.primary_down() || …middle_down()) ; i.pointer.delta()   — pan is the LEFT button now
ui.ctx().request_repaint()
Vec2: fields .x/.y (NOT .0/.1); Vec2::ZERO; operators + - * work; self.pan += delta works
```
Case-insensitive substring search has **no `String::contains`** in this stdlib —
use `hay.to_lowercase().as_str().match_indices(needle.to_lowercase().as_str()).next().is_some()`.
Native bits: `rfd::FileDialog::new().pick_folder()/.save_file()/.add_filter(name, &["ext"]).set_directory(&path).set_file_name(…)`,
`std::fs::read_dir(path) -> iterator of Result<DirEntry>` (recursive walk lives in the app),
`std::thread::spawn(move || …)` with `std::sync::mpsc::channel()` + `Receiver::try_recv()` for non-blocking decode.

---

## 7. Patch / presets (mechanism)

`PipelineParams` already derives `serde::Serialize/Deserialize` and `Default`.
So:
- a **patch file** = JSON of `PipelineParams` (+ the source path, app-side).
- a **preset** = same JSON without the source.
- **undo** = a stack of `PipelineParams` (in-memory), each being "edit → push → re-render".
None of this is wired into the UI yet (see 8).

---

## 8. Known bugs, gotchas and limitations

### Bugs (confirmed behaviour that is wrong)

1. **RGBA `ColorImage::from_rgba_unmultiplied` is fed `&out.data` directly.** This is
   correct because `RgbaImage.data` is already RGBA — but it is a silent coupling:
   if `RgbaImage` ever changes interleave, the texture silently shifts. Keep the two
   layouts in lockstep; add a test that asserts `RgbaImage.data.len() = w*h*4`.
2. **`sync_texture` uploads on the frame the image loads.** For a 1800 px preview this
   is sub-10 ms; for full-res exports we re-decode + `develop` per export (no base
   reuse — see Limitation 3). No lazy placeholder is shown while the first texture
   uploads; on very slow first frames the canvas shows the status text instead.
3. **Early `rotate_quarter_cw/ccw` had out-of-bounds source reads** for non-square
   images (read `pixel(x, H-1-y)` with a `row`-local index). Fixed by mapping
   `(out.x, out.y) → src(y, H-1-x)` / `(W-1-y, x)` and indexing `row[x*3]` not
   `row[i*3]`. Covered by `rotate_90_cw_corners` / `rotate_90_ccw_corners` tests.
   Any future rewrite of `rotate_180` must keep the sequential flip (it was the
   simplest correct version; rayon version was abandoned mid-edit — see leftover
   comment history).

### Gotchas (behaviour that bites the next change)

4. **egui closures move-capture `self`.** You cannot have two `.show(ui, |ui| …)`
   closures in one `fn ui` that each close over `&mut self` — the second capture
   errors. Follow `docs/…/panels.rs` pattern: call methods that take `&mut self`
   from inside a single closure, or destructure fields out of `self` first so each
   closure captures only the field refs it needs. Multiple closures over disjoint
   fields is fine; overlapping `&mut self` is not.
5. **`dyn` is a keyword** in this stdlib (edition 2024 / Rust 1.99). Any variable or
   name that used to be `dyn` must be renamed (we use `dimg` / `base`). The compiler
   suggests `` `r#dyn` `` but renaming is cleaner.
6. **`Option::cloned()` is only on `Option<&_>`** — to pattern-match `&self.texture`
   use `match &self.texture { Some(tex) => … }`; to mutate through it bind
   `&mut self.texture` and match (not `if let Some(mut x) = &expr` — this edition
   forbids `mut` in implicitly-borrowing patterns; use `match` with `&mut` scrutinee).
7. **rayon `reduce` identity is a closure**, `reduce(|| 0, |a,b| …)` — the 2006
   edition changed identity from a value to a thunk. Also `Sender::try_send` was
   removed (only blocking `send` on unbounded channels); non-blocking receive is
   `Receiver::try_recv()` which returns `Err(TryRecvError::Empty)` when idle — loop
   then break on `Empty`, since `load_rx` may receive multiple messages.
8. **`to_lower()` does not exist** — it is `to_lowercase()`. `String::new()`,
   `String::from(&str)`, `.as_str()`, `format!("{}", x)` are the string toolbox.
   `Path` has `to_string_lossy().into_owned()` (no `as_str` on `Path`); the app wraps
   `PathBuf -> String` via `into_string().unwrap()`.
9. **Closures over `self.files[i]`** move the `String` out — always `let label = &self.files[i]`
   then `format!("{}", *label)`.
10. **A side panel only resizes while its content consumes the available space.**
    A `ScrollArea` or a `Separator`, or `ui.take_available_space()` at the top of the
    panel's closure, is what makes the resize drag actually do anything. Without it the
    cursor changes to the resize icon but the width stays put. Left/right panels do
    call `take_available_space()` on purpose.

### Limitations (known, mostly deferred by PRODUCT.md)

1. **Memory**: f32 linear base + RGBA output at 64MP peaks ≈ 1 GB (f32 772 MB +
   RGBA 257 MB + bayer 129 MB transients). No streaming / tiling. PRODUCT.md targets
   24 MP comfortable / 64 MP headroom — this is that headroom being expensive.
2. **Preview is downscaled to ≤1800 px** so sliders are instant; export re-develops
   the full-res source. There is no full-res "100% pixel peep" view yet.
3. **Export re-decodes + re-develops the source from scratch.** The app does not keep
   the full-res base in memory between preview and export (deliberate trade, see 1).
   For multiple exports of the same image this repeats decoding work.
4. **`rotate_fine` extra**: fine angle is bilinear with **white fill** outside the
   source — corners become white. Straighten-then-crop is the natural workflow and
   crop is applied *before* rotation, so the UI should offer crop before fine angle.
5. **WB temperature/tint sliders** are a simplified gain model
   (`1 + 0.30·t`, `1 + 0.15·m` around neutral). They are a knob, not a colourimetric
   temperature; refining them toward a real Kelvin/CIE model is the "a-ha colors"
   continuing work (Percept `PRODUCT.md`).
6. **TIFF input** is downconverted to 8-bit (per PRODUCT.md); 16-bit pipeline later.
7. **RAW support list** is the `rawler` decoder set, but *only ORF has been tested on
   the owner's machine* (Olympus E-M5 Mark II, incl. high-res 9280×6932 16-bit mode —
   decode ≈ 1–2 s). Other makes are untested.
8. **Not in the UI yet** (core supports the mechanism, UI wiring pending):
   - crop tool (marquee + handles + aspect lock) — `params.crop` + `geometry::crop`
     already implemented
   - undo history (params stack)
   - patch-file autosave/restore and presets
   - thumbnails in the file list (a selection preview pane exists in the left panel;
     per-file thumbnails are still not)
   - persist last folder / last export settings (OS config JSON, per PRODUCT.md)
9. **App is intentionally single-file.** It will be split into modules only when the
   file list/canvas/tools each earn their own file (see `METHODS.md`: prefer the
   smallest change; splitting is cosmetic until then).
10. **The auto black-point baseline is baked into the demosaic output**, so the
    R/G/B sliders start at 0 (0 = "auto baseline already applied"). Making the
    baseline *visible* as non-zero slider starts would require moving the per-channel
    stretch into a params-level op with a per-channel white point. The robust
    percentile change already removes the outage/crush symptom; this is the seam to
    revisit if the sliders ever need to show the scan's floor.

---

## 9. Anti-patterns to reject in AI-written Rust

Review checklist for code produced by an AI agent, written from a
managed-runtime (Java/Kotlin) reviewer's perspective.

1. **The borrow-checker bypass: `.clone()` / `Arc<Mutex<T>>` spray.** Reaching
   for shared pointers or deep clones wherever the borrow checker bites. Look for
   hot-path `.clone()` and wide use of `Arc<Mutex<T>>` where a `&T` / `&mut T`
   lifetime or linear ownership transfer would do. Costs allocations + lock
   contention for free.
2. **Lazy errors: `.unwrap()`, `.expect("never fails")`, `panic!` in production
   logic.** Unlike a checked exception that bubbles to a framework, a panic on a
   worker thread aborts the task or the runtime. Errors must flow via
   `Result<T, E>` / `Option<T>` (match, `?`, combinators), not by silencing the
   type.
3. **OOP simulation: trait hierarchies as class trees, `Box<dyn Trait>`
   polymorphism, stateful mega-structs.** Rust is data-oriented: plain structs of
   data, behavior in decoupled traits, generics for static dispatch. `dyn`
   dispatch costs a vtable where zero-cost abstraction was the whole point.
4. **Blocking I/O inside async fns.** Sync file ops / DB drivers / CPU-bound loops
   written directly in `async fn` starve the runtime's worker threads
   (`tokio::task::spawn_blocking` exists for this).
5. **C/Java-style imperative loops.** `for i in 0..len` with manual indexing or
   external mutable accumulators where an iterator chain (`.iter()`, `.map()`,
   `.filter()`, `.fold()`, `.collect()`) is both cheaper and safer. Index-based
   mutation is the only excuse.

---

## 10. Verification

- `cargo test` — 19 core unit tests (every op + serde roundtrip + demosaic shape +
  downscale + develop-invert integration).
- `cargo fmt`, `cargo clippy` — both clean (clippy emits style suggestions only).
- Manual smoke: `cargo build --release && target\release\emulsion-app.exe` — open
  `emulsion-core\examples\orf`, click an ORF, drag sliders, `Export…`.
- Spike baseline: `target\release\develop.exe <in.orf> <out.png> [--invert]` matches
  the PNG `emulsion-core\examples\orf` chrome (ignored by git).