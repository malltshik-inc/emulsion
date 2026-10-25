//! Folder scanning, file selection and background decoding.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;

use eframe::egui;
use egui::Vec2;
use emulsion_core::params::PipelineParams;

use crate::app::EmulsionApp;
use crate::model::{CropDrag, Loaded, Orientation, ThumbData, TreeNode};
use crate::theme::{THUMB_DECODE_EDGE, THUMB_EDGE};

/// The image leaves currently reachable through the expanded folders.
fn visible_indices(nodes: &[TreeNode], expanded: &HashSet<String>) -> Vec<usize> {
    let mut out = Vec::new();
    let mut stack: Vec<&[TreeNode]> = vec![nodes];
    // Flat-collect via a worklist: a folder is entered only while expanded.
    while let Some(list) = stack.pop() {
        for node in list {
            match node.file_index {
                Some(i) => out.push(i),
                None => {
                    if expanded.contains(&node.path) {
                        stack.push(&node.children);
                    }
                }
            }
        }
    }
    out
}

/// The visible leaves whose thumbnail base is still missing (should be decoded
/// next). An index is loadable only when it is in bounds and its cached base is
/// `None` — `Some(&None).is_none()` is false, so `.get(i).is_none()` would
/// wrongly drop every in-bounds index.
fn indices_to_load<T>(thumb_bases: &[Option<T>], visible: &[usize]) -> Vec<usize> {
    visible
        .iter()
        .copied()
        .filter(|i| thumb_bases.get(*i).is_some_and(Option::is_none))
        .collect()
}

impl EmulsionApp {
    /// Recursive scan of `dir`: image leaves and one node per subfolder. Each
    /// directory lists its images (by name) then its subfolders (by name).
    /// Leaf paths are appended to `files`, so `file_index` matches the flat
    /// list used by the cursor, settings and export.
    fn build_tree(dir: &str, files: &mut Vec<String>) -> Result<Vec<TreeNode>, String> {
        let mut subdirs: Vec<(String, String)> = Vec::new();
        let mut file_paths: Vec<String> = Vec::new();
        match std::fs::read_dir(dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let s = path.to_string_lossy().into_owned();
                        subdirs.push((crate::util::file_name(&s), s));
                    } else {
                        let s = path.to_string_lossy().into_owned();
                        if emulsion_core::decode::is_supported(&s) {
                            file_paths.push(s);
                        }
                    }
                }
            }
            Err(e) => return Err(format!("cannot read {}: {}", dir, e)),
        }
        subdirs.sort_by(|a, b| a.0.cmp(&b.0));
        file_paths.sort();
        let mut nodes = Vec::with_capacity(file_paths.len() + subdirs.len());
        for path in file_paths {
            let index = files.len();
            let name = crate::util::file_name(&path);
            files.push(path.clone());
            nodes.push(TreeNode {
                name,
                path,
                children: Vec::new(),
                file_index: Some(index),
            });
        }
        for (name, path) in subdirs {
            let children = Self::build_tree(&path, files)?;
            nodes.push(TreeNode {
                name,
                path,
                children,
                file_index: None,
            });
        }
        Ok(nodes)
    }

    pub(crate) fn open_folder(&mut self) {
        let dir = rfd::FileDialog::new().pick_folder();
        if let Some(p) = dir {
            let path = p.to_string_lossy().into_owned();
            self.save_current();
            self.cursor = None;
            self.loaded = None;
            self.texture = None;
            self.texture_size = None;
            self.histogram = None;
            self.params = PipelineParams::default();
            self.film_textures.clear();
            self.film_preview_base = None;
            self.film_preview_params = None;
            self.film_preview_source = None;
            self.dirty = true;
            let mut files = Vec::new();
            match Self::build_tree(path.as_str(), &mut files) {
                Ok(tree) => {
                    self.tree = tree;
                    self.files = files;
                    self.status = format!("{} images in folder", self.files.len());
                }
                Err(e) => {
                    self.tree = Vec::new();
                    self.files = Vec::new();
                    self.status = format!("error: {}", e);
                }
            }
            self.expanded.clear();
            self.refresh_thumbnails();
        }
    }

    fn save_current(&mut self) {
        if let Some(current) = self.cursor
            && let Some(path) = self.files.get(current)
        {
            self.settings.insert(path.clone(), self.params);
        }
    }

    pub(crate) fn select_file(&mut self, index: usize) {
        self.save_current();
        self.cursor = Some(index);
        self.params = self
            .files
            .get(index)
            .and_then(|path| self.settings.get(path))
            .copied()
            .unwrap_or_default();
        self.crop_rect = self.params.crop;
        self.crop_drag = CropDrag::None;
        self.crop_drag_start = None;
        self.crop_drag_rect = None;
        if let Some((w, h)) = self.current_size() {
            self.crop_orientation = if w >= h {
                Orientation::Landscape
            } else {
                Orientation::Portrait
            };
        }
        self.loaded = None;
        self.texture = None;
        self.texture_size = None;
        self.histogram = None;
        self.film_textures.clear();
        self.film_preview_base = None;
        self.film_preview_params = None;
        self.film_preview_source = None;
        self.dirty = true;
        self.load_selected();
    }

    fn load_selected(&mut self) {
        let Some(i) = self.cursor else { return };
        if i >= self.files.len() {
            return;
        }
        let path = &self.files[i];
        self.loading = true;
        self.status = format!("loading {}", path);
        let (sender, receiver) = channel::<Result<Loaded, String>>();
        self.load_rx = Some(receiver);
        let path_owned = String::from(path.as_str());
        std::thread::spawn(move || {
            let base = match emulsion_core::decode::decode(path_owned.as_str()) {
                Ok(b) => b,
                Err(e) => {
                    let _ = sender.send(Err(format!("decode failed: {}", e.message())));
                    return;
                }
            };
            let source_size = (base.width, base.height);
            let base_small = emulsion_core::geometry::downscale(&base, 2000);
            let base_draft = emulsion_core::geometry::downscale(&base, 1600);
            let _ = sender.send(Ok(Loaded {
                base_small,
                base_draft,
                source_size,
            }));
        });
    }

    pub(crate) fn poll_load(&mut self) {
        loop {
            let Some(rx) = &self.load_rx else { return };
            match rx.try_recv() {
                Ok(msg) => {
                    match msg {
                        Ok(loaded) => {
                            self.loaded = Some(loaded);
                            self.status = String::from("ready — adjust the sliders");
                        }
                        Err(e) => self.status = e,
                    }
                    self.loading = false;
                    self.dirty = true;
                    self.zoom = 0.0;
                    self.fit_zoom = 0.0;
                    self.pan = Vec2::ZERO;
                    self.load_rx = None;
                }
                Err(_) => return,
            }
        }
    }

    /// Decodes thumbnails for `indices` on a background thread. A new call
    /// cancels the previous worker via the shared flag + generation counter.
    fn start_thumbnail_worker(&mut self, indices: Vec<usize>) {
        if let Some(flag) = &self.thumb_cancel {
            flag.store(true, Ordering::Relaxed);
        }
        self.thumb_generation = self.thumb_generation.wrapping_add(1);
        let generation = self.thumb_generation;
        let cancel = Arc::new(AtomicBool::new(false));
        self.thumb_cancel = Some(Arc::clone(&cancel));

        let files = self.files.clone();
        let (sender, receiver) = channel();
        self.thumb_rx = Some(receiver);
        std::thread::spawn(move || {
            for index in indices {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                let Some(path) = files.get(index) else {
                    continue;
                };
                let thumb =
                    match emulsion_core::decode::decode_thumbnail(path.as_str(), THUMB_DECODE_EDGE)
                    {
                        Ok(thumb) => thumb,
                        Err(_) => continue,
                    };
                let source_size = thumb.size;
                let small = emulsion_core::geometry::downscale(&thumb.image, THUMB_EDGE);
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                if sender
                    .send((
                        generation,
                        Some(ThumbData {
                            index,
                            base: small,
                            source_size,
                        }),
                    ))
                    .is_err()
                {
                    return;
                }
            }
            let _ = sender.send((generation, None));
        });
    }

    /// Full reset after a folder open: clears every cache and schedules the
    /// thumbnails of the rows that are visible right now (root files only,
    /// since folders start collapsed).
    pub(crate) fn refresh_thumbnails(&mut self) {
        self.thumb_bases = vec![None; self.files.len()];
        self.thumb_textures = vec![None; self.files.len()];
        self.thumb_sizes = vec![None; self.files.len()];
        self.thumb_params = vec![None; self.files.len()];
        // A new folder invalidates any queued indices and the dedup flags.
        self.thumbs_pending.clear();
        self.thumb_queued = vec![false; self.files.len()];
        let visible = visible_indices(&self.tree, &self.expanded);
        self.start_thumbnail_worker(visible);
    }

    /// On-demand follow-up after a folder is expanded: decode only the newly
    /// visible leaves that are not cached yet (already-loaded rows are kept).
    pub(crate) fn schedule_visible_thumbnails(&mut self) {
        let visible = visible_indices(&self.tree, &self.expanded);
        let to_load = indices_to_load(&self.thumb_bases, &visible);
        if !to_load.is_empty() {
            self.start_thumbnail_worker(to_load);
        }
    }

    pub(crate) fn poll_thumbnails(&mut self) {
        loop {
            let Some(rx) = &self.thumb_rx else { return };
            match rx.try_recv() {
                Ok((generation, message)) => {
                    if generation != self.thumb_generation {
                        continue;
                    }
                    match message {
                        Some(data) => {
                            if data.index < self.thumb_bases.len() {
                                self.thumb_bases[data.index] = Some(data.base);
                                self.thumb_sizes[data.index] = Some(data.source_size);
                                self.enqueue_thumbnail(data.index);
                            }
                        }
                        None => {
                            self.thumb_rx = None;
                            return;
                        }
                    }
                }
                Err(_) => return,
            }
        }
    }

    pub(crate) fn params_for_index(&self, index: usize) -> PipelineParams {
        if self.cursor == Some(index) {
            self.params
        } else {
            self.files
                .get(index)
                .and_then(|path| self.settings.get(path))
                .copied()
                .unwrap_or_default()
        }
    }

    pub(crate) fn current_size(&self) -> Option<(usize, usize)> {
        let index = self.cursor?;
        if let Some(loaded) = &self.loaded {
            return Some(loaded.source_size);
        }
        self.thumb_sizes.get(index).copied().flatten()
    }

    pub(crate) fn current_name(&self) -> String {
        self.cursor
            .and_then(|index| self.files.get(index))
            .map(|path| crate::util::file_stem(path))
            .unwrap_or_else(|| String::from("No image"))
    }

    pub(crate) fn current_meta(&self) -> String {
        let Some(index) = self.cursor else {
            return String::from("OPEN A FOLDER");
        };
        let Some(path) = self.files.get(index) else {
            return String::from("OPEN A FOLDER");
        };
        let extension = crate::util::file_extension(path);
        match self.current_size() {
            Some((w, h)) => format!("{} · {} MP", extension, crate::util::megapixels(w, h)),
            None => extension,
        }
    }

    pub(crate) fn is_edited_at(&self, index: usize) -> bool {
        self.params_for_index(index) != PipelineParams::default()
    }

    pub(crate) fn is_current_edited(&self) -> bool {
        self.params != PipelineParams::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fixture() -> (std::path::PathBuf, String) {
        let root = std::env::temp_dir().join(format!(
            "emulsion-tree-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("alpha/nested")).unwrap();
        std::fs::create_dir_all(root.join("zeta")).unwrap();
        std::fs::write(root.join("b.jpg"), vec![0u8]).unwrap();
        std::fs::write(root.join("a.orf"), vec![0u8]).unwrap();
        std::fs::write(root.join("alpha/x.png"), vec![0u8]).unwrap();
        std::fs::write(root.join("alpha/nested/y.arw"), vec![0u8]).unwrap();
        std::fs::write(root.join("notes.txt"), "not an image".as_bytes()).unwrap();
        let path = root.to_string_lossy().into_owned();
        (root, path)
    }

    fn cleanup(root: &std::path::Path) {
        for _ in 0..5 {
            if std::fs::remove_dir_all(root).is_ok() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn tree_groups_files_first_then_folders_sorted() {
        let (root, path) = make_fixture();
        let mut files = Vec::new();
        let tree = EmulsionApp::build_tree(&path, &mut files).unwrap();

        let top: Vec<&str> = tree
            .iter()
            .map(|n| {
                if n.file_index.is_some() {
                    "file"
                } else {
                    "dir"
                }
            })
            .collect();
        assert_eq!(top, vec!["file", "file", "dir", "dir"]);
        assert_eq!(tree[0].name, "a.orf");
        assert_eq!(tree[1].name, "b.jpg");
        assert_eq!(tree[2].name, "alpha");
        assert_eq!(tree[3].name, "zeta");
        assert_eq!(tree[2].file_index, None);
        assert_eq!(tree[3].file_index, None);
        assert_eq!(tree[0].file_index, Some(0));
        assert_eq!(tree[1].file_index, Some(1));

        let alpha = &tree[2].children;
        assert_eq!(alpha[0].name, "x.png");
        assert_eq!(alpha[1].name, "nested");
        assert_eq!(alpha[0].file_index, Some(2));

        let nested = &alpha[1].children;
        assert_eq!(nested[0].name, "y.arw");
        assert_eq!(nested[0].file_index, Some(3));

        assert_eq!(files.len(), 4);
        assert_eq!(files[0], root.join("a.orf").to_string_lossy());
        assert_eq!(
            files[3],
            root.join("alpha")
                .join("nested")
                .join("y.arw")
                .to_string_lossy()
        );

        cleanup(&root);
    }

    #[test]
    fn visible_indices_follows_expansion() {
        let (root, path) = make_fixture();
        let mut files = Vec::new();
        let tree = EmulsionApp::build_tree(&path, &mut files).unwrap();
        cleanup(&root);

        let empty = HashSet::new();
        assert_eq!(visible_indices(&tree, &empty), vec![0, 1]);

        let mut expanded = HashSet::new();
        expanded.insert(tree[2].path.clone());
        assert_eq!(visible_indices(&tree, &expanded), vec![0, 1, 2]);

        expanded.insert(tree[2].children[1].path.clone());
        assert_eq!(visible_indices(&tree, &expanded), vec![0, 1, 2, 3]);
    }

    #[test]
    fn indices_to_load_skips_cached_and_out_of_bounds() {
        // Regression: `.get(i).is_none()` tests the outer Option, which is
        // `Some(..)` for every in-bounds index, so nothing was ever scheduled
        // for expanded folders (root-only thumbnails still worked because
        // `refresh_thumbnails` bypasses this filter).
        let thumb_bases: Vec<Option<u8>> = vec![Some(1), None, None];
        assert_eq!(indices_to_load(&thumb_bases, &[0, 1, 2]), vec![1, 2]);
        assert_eq!(indices_to_load(&thumb_bases, &[0, 2, 99]), vec![2]);
        assert_eq!(indices_to_load(&thumb_bases, &[0]), Vec::<usize>::new());
    }
}
