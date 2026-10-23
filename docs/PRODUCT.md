# Product — specification

> **Type: normative.** This file dictates how Emulsion behaves from the user's point of
> view. It knows nothing of classes, methods or code. When implementation and this file
> disagree, this file wins and the implementation must change.
>
> Definitions of "MUST / SHOULD / MAY" follow RFC 2119.

---

## 1. Purpose

Emulsion converts negative film scans into positive images, with precise color
control and a minimal, predictable interface. The product does a few things and does
them extremely well. It is not a general-purpose photo editor.

## 2. Product principles

1. **One document model.** The user works on exactly one image at a time. Everything
   they do is a *setting*, and the image is re-rendered from the original scan with
   those settings. The original file is never modified.
2. **The original is sacred.** No operation ever writes back to the source. The only
   way to produce a file is a deliberate export.
3. **Fewer controls, sharper ones.** Every control must earn its place. The right six
   knobs, done precisely, beat twenty approximate ones.
4. **"A-ha colors" over out-of-the-box correctness.** The flagship promise is that a
   dark, orange-masked scan becomes a vivid, correct positive with near-zero effort —
   and that the few knobs give the user power they can feel.
5. **Fast by default.** Opening a scan and correcting it should feel instant. Waiting
   is only acceptable for one-time decode of huge RAW files.

## 3. Who it is for

People who photograph film negatives with a digital camera (or scan film) and want a
tool that gets them from scan → nice positive in as few steps as possible, without
learning a full raw processor. The owner's reference workflow is an Olympus E-M5
Mark II shooting high-res mode (≈64 MP ORF files).

## 4. User interface (MUST, v1)

Three-zone window:

- **File browser (left):** contains the *Open folder…* button, a preview of the
  currently selected image, and a flat, recursive list of all supported images in
  the opened folder. Clicking an image opens it.
- **Canvas (center):** shows the current result, fit to the window by default.
  Zoom with the mouse wheel. Pan by dragging with the left button or middle button.
  Press **Space** to recenter the image.
- **Tools (top strip):** the controls that change the image, as toggle tabs
  (Develop, Simulations, Crop, Rotate, White Balance, Tint, Exposure, Contrast, Saturation).
  Simulations, Crop, Rotate, White Balance, Tint, Exposure, Contrast and Saturation expand
  their panels below the strip; **Develop toggles negative→positive immediately without opening a panel**.
  Each slider shows a title first, then its rail with the numeric value beside it. The R/G/B black-point
  sliders are hidden in the v1 UI (the params and automatic baseline still apply).
  The Simulations panel puts a search box in its header and adds a strip of film
  simulations that scrolls horizontally with the mouse wheel.

Supporting structure:

- **Top bar:** `Export`.
- **Status line** runs along the bottom edge of the canvas and always shows the
  current state: number of images, which file is loading or loaded, and the outcome
  of an export.

## 5. Opening files (MUST)

- MUST support **JPEG, PNG, TIFF** and **ORF (Olympus RAW)**.
- Other camera RAW formats (NEF, CR2/3, ARW, RW2, RAF, DNG, …) should be supported
  via the same path, but are not a v1 release gate and must not degrade ORF.
- MUST show a clear "loading" state while a RAW decodes.
- A file never edited in this session starts from **neutral settings** (see §7
  defaults), with Develop off. The user enables it when editing a negative. Each file keeps its own settings: switching away and back restores them
  (session-scoped; see §10).

## 6. File browser (MUST)

- MUST list images recursively under the chosen folder, sorted by name.
- MUST be reachable via `Open folder…`, which lives inside the file browser panel
  and uses the platform's native folder picker.
- MUST show a small thumbnail per file in the list (decoded in the background; a
  placeholder until it is ready). There is no separate selection preview pane.
- MUST list every decodable format (all RAW extensions the decoder accepts, plus
  JPEG/PNG/TIFF) - an ARW next to an ORF must be visible.
- MUST NOT modify, move or delete any file.
- MUST remember each file's settings for the session: selecting another file loads
  that file's settings (neutral if untouched), and thumbnails reflect each file's own
  settings while the selected file shows live edits.

## 7. Controls (MUST — the complete v1 set)

Exactly these controls. No curve editor, no HSL, no channels panel, no LUT import in v1.

Color sliders (five visible in the v1 UI):

1. **White balance — temperature** (range −100..+100; 0 = neutral, + = warmer)
2. **White balance — tint** (range −100..+100; 0 = neutral, + = magenta)
3. **Exposure** (range −3..+3 EV)
4. **Contrast** (range −100..+100; 0 = neutral, + = punchier, − = flatter)
5. **Saturation** (range −100..+100; 0 = neutral, − = greyscale, + = more colour)
6. **Black point R** (0..1) — param only; slider hidden in the v1 UI
7. **Black point G** (0..1) — param only; slider hidden in the v1 UI
8. **Black point B** (0..1) — param only; slider hidden in the v1 UI

Film:

9. **Develop** (toggle) — the negative→positive base conversion
10. **Simulations** — a bundled Hald CLUT chosen from a horizontal strip.
    The strip contains every black-and-white and colour simulation from the
    RawTherapee Film Simulation collection (bundle licence in
    `emulsion-core/assets/film/ATTRIBUTION.md`). A search box in the header
    filters by name; the mouse wheel scrolls the strip. "No simulation" (the
    default) applies none. The LUT runs last, after contrast and saturation.

Rotate:

11. **Rotate 90°** (two buttons: counter-clockwise, clockwise)
12. **Fine angle** (slider, −5°..+5°, as fine as 0.1°)

Defaults (MUST be identical on every fresh open): temperature 0, tint 0, exposure 0,
contrast 0, saturation 0, no film simulation, all black points 0, Develop **off**,
rotation 0, angle 0, no crop.

Behavior requirements (MUST):

- Every control change re-renders the preview **immediately** (no apply button).
- Double-clicking a slider track resets that control to its default (0).
- 90° rotation and fine angle combine — the result is the sum of the two.
- The visible sliders (plus the hidden black-point params) and the invert switch
  work on the *same* image and combine.
- The preview reflects the final result that export will produce, at the resolution
  the canvas can show.

## 8. The black points (special requirement)

The R/G/B black points are the tool's core "a-ha" control. Their purpose: remove the
orange cast of color-negative film *before* inverting, so positive images have clean
blacks and correct colors. They MUST automatically start from a sensible per-channel
baseline derived from the scan itself (so a fresh open already looks close), with the
sliders letting the user refine. Concretely: a scan that only uses part of the
sensor's range MUST be stretched to the full visible range automatically; a slider
adjusts that per channel. The automatic stretch MUST be robust to stray hot/dead
pixels — a handful of outlying samples must not be allowed to crush the rest of the
image into a dark sliver, and a fresh open must be close without a large exposure
bump. The manual R/G/B sliders are currently hidden in the v1 UI (see §7); the
automatic baseline is still computed and applied.

## 9. Crop (MUST, v1 mechanism; UI in §13)

- MUST allow cropping the image to a rectangular region.
- MUST support free-form crop and at least one aspect-ratio lock (e.g. 3:2, 1:1, 4:5)
  with a selectable aspect ratio and an "free" option.
- The crop preview MUST show the remaining region.
- The crop is part of the settings (see §10) and MUST survive as a patch/undo unit.
- Crop is applied as part of the final render/export.

## 10. Settings as data (MUST)

The user's entire edit is a single value: the visible sliders (plus the hidden
black-point params), invert, the selected film simulation, rotate, angle, crop.
This value MUST:

- be loadable/savable as a small text file (the **patch**), which is only the settings
  plus a reference to the source image;
- support being stored **without** the source reference (a **preset**);
- be the unit of **undo**: each committed change toggles back to the previous value;
- preserve nothing pixel-related.

Per-file settings are the same value associated with each source path. They live
in memory for the session (no disk autosave yet); patches/presets are the durable
form.

Undo (in-memory) is REQUIRED in v1; patch-file autosave/restore and presets are
REQUIRED mechanisms but may be exposed in the UI later (§13).

## 11. Export (MUST)

- The **only** file-writing operation.
- Supported destinations: **JPEG** (with a quality control, 1–100, default high)
  and **PNG** (lossless, no quality control).
- MUST export at the source resolution (real pixels, not the preview scale).
- Uses the native save dialog; the chosen folder/file name is user's. The dialog
  pre-fills the **source file stem** (e.g. `P6030011.png` for `P6030011.ORF`).
- MUST never touch the original file.
- After a successful save, writes a sidecar next to the export
  (`<name>.emulsion.json`) containing the source path, the exact `PipelineParams`
  used and, when a film simulation was selected, its name. Format
  `emulsion-params-v1`. This is for reproduction and taste analysis; it is not
  the patch autosave.
- After export, the status line states the result (success and path, or error).

## 12. Performance (MUST)

- Target comfort: **24 MP** images.
- Headroom: **64 MP** (the owner's high-res ORF files) must open and remain usable.
- Opening a RAW: MUST show progress/loading; decode completing within a few seconds
  on a current desktop is the goal.
- After a scan is open, every slider, rotation and crop change MUST update the
  preview in ~one frame on the target sizes. This is a hard requirement: "fast by
  default" is a product principle.
- Export of a 64 MP image may take noticeably longer than preview — that is expected
  and stated in the status line.

## 13. Out of scope / deferred (MAY, not v1 gates)

- Crop **tool** in the UI (marquee + handles + aspect lock) — mechanism exists, tool
  is next.
- Patch-file autosave/restore and preset management in the UI.
- Importing user-supplied `.cube`/Hald LUTs (the bundled film strip is v1).
- Undo UI beyond in-memory history.
- 100%-zoom pixel peeping view.
- 16-bit pipeline and TIFF export.
- Other RAW makes than ORF.
- Batch processing, plugins, scripting, printing, cloud, accounts.

## 14. Non-goals (MUST NOT)

- No destructive editing, no "save over source", no autosave of pixels.
- No catalog/database, no keywords, no face detection, no "AI" features.
- No curve editor/HSL/user LUT import in v1 (see §7: the bundled film
  simulations are the only profiles).
- No network access, no telemetry.

## 15. Acceptance checklist (how v1 is known done)

- [ ] Open a folder with mixed images (JPEG/PNG/TIFF/ORF); all appear; ORF opens with
      a visible loading state.
- [ ] With a color-negative ORF: select it, flip Develop → the result
      looks like a correct positive with a single click; every visible slider
      visibly pushes the image in the labeled direction.
- [ ] A user who opens a scan on the owner's hardware reaches a satisfying positive in
      **three interactions or fewer** (open, invert, one slider).
- [ ] Every control update re-renders without pressing apply; 24 MP stays interactive;
      64 MP opens and remains usable.
- [ ] Film strip lists every bundled simulation, search narrows it by name, and
      picking one updates the preview, thumbnails and export.
- [ ] 90° + fine angle + crop combine correctly and match the export.
- [ ] Export JPEG (with quality) and PNG at full resolution; original byte-for-byte
      untouched (verified by file hash before/after).
- [ ] Undo reverts each committed setting change.
- [ ] Patch and preset files round-trip: save settings, reopen/restore, image renders
      identically.