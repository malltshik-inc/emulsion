//! UI-side model types shared by the panels.

use emulsion_core::params::PipelineParams;

/// Which adjustment panel is expanded in the tool strip.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tool {
    Film,
    Simulations,
    Wb,
    Tint,
    Ev,
    Contrast,
    Saturation,
    Shadows,
    Highlights,
    Vignette,
    Rotate,
    Crop,
}

impl Tool {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Film => "Film",
            Self::Simulations => "Simulations",
            Self::Wb => "White Balance",
            Self::Tint => "Tint",
            Self::Ev => "Exposure",
            Self::Contrast => "Contrast",
            Self::Saturation => "Saturation",
            Self::Shadows => "Shadows",
            Self::Highlights => "Highlights",
            Self::Vignette => "Vignette",
            Self::Rotate => "Rotate",
            Self::Crop => "Crop",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Film => "FILM",
            Self::Simulations => "SIMULATIONS",
            Self::Wb => "WB",
            Self::Tint => "TINT",
            Self::Ev => "EV",
            Self::Contrast => "CONTRAST",
            Self::Saturation => "SATURATION",
            Self::Shadows => "SHADOWS",
            Self::Highlights => "HIGHLIGHTS",
            Self::Vignette => "VIGNETTE",
            Self::Rotate => "ROTATE",
            Self::Crop => "CROP",
        }
    }

    pub(crate) fn aliases(self) -> &'static str {
        match self {
            Self::Film => "develop negative positive invert film",
            Self::Simulations => "film simulation lut negative positive invert develop",
            Self::Wb => "white balance temperature warm cool",
            Self::Tint => "tint magenta green",
            Self::Ev => "exposure brightness",
            Self::Contrast => "contrast",
            Self::Saturation => "saturation vibrance color",
            Self::Shadows => "shadows dark lift",
            Self::Highlights => "highlights bright recovery",
            Self::Vignette => "vignette edges darken",
            Self::Rotate => "rotate angle straighten",
            Self::Crop => "crop aspect ratio",
        }
    }

    pub(crate) fn range(self) -> std::ops::RangeInclusive<f32> {
        match self {
            Self::Ev => -3.0..=3.0,
            Self::Rotate => -5.0..=5.0,
            Self::Crop | Self::Simulations => 0.0..=0.0, // use different UI
            _ => -100.0..=100.0,
        }
    }

    pub(crate) fn decimals(self) -> usize {
        match self {
            Self::Ev | Self::Rotate => 1,
            _ => 0,
        }
    }

    pub(crate) fn suffix(self) -> &'static str {
        match self {
            Self::Rotate => "°",
            _ => "",
        }
    }

    pub(crate) fn value_mut(self, params: &mut PipelineParams) -> Option<&mut f32> {
        match self {
            Self::Wb => Some(&mut params.wb_temperature),
            Self::Tint => Some(&mut params.wb_tint),
            Self::Ev => Some(&mut params.exposure),
            Self::Contrast => Some(&mut params.contrast),
            Self::Saturation => Some(&mut params.saturation),
            Self::Shadows => Some(&mut params.shadows),
            Self::Highlights => Some(&mut params.highlights),
            Self::Vignette => Some(&mut params.vignette),
            Self::Rotate => Some(&mut params.fine_angle),
            Self::Film => Some(&mut params.invert_sensitivity),
            Self::Crop | Self::Simulations => None,
        }
    }

    pub(crate) fn changed(self, params: &PipelineParams) -> bool {
        match self {
            Self::Film => params.invert,
            Self::Simulations => params.film.is_some(),
            Self::Rotate => {
                params.fine_angle != 0.0 || params.rotate_90 != 0 || params.flip_horizontal
            }
            Self::Crop => params.crop.is_some(),
            _ => {
                let mut copy = *params;
                self.value_mut(&mut copy).is_some_and(|t| *t != 0.0)
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CropAspect {
    Free,
    Original,
    Square,
    FourThree,
    ThreeTwo,
    SixteenNine,
    TwoOne,
}

impl CropAspect {
    pub(crate) const ALL: [CropAspect; 7] = [
        Self::Free,
        Self::Original,
        Self::Square,
        Self::FourThree,
        Self::ThreeTwo,
        Self::SixteenNine,
        Self::TwoOne,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Original => "Original",
            Self::Square => "1:1",
            Self::FourThree => "4:3",
            Self::ThreeTwo => "3:2",
            Self::SixteenNine => "16:9",
            Self::TwoOne => "2:1",
        }
    }

    pub(crate) fn is_free(self) -> bool {
        matches!(self, Self::Free)
    }

    /// Base width / height, or `None` for an unconstrained crop.
    pub(crate) fn ratio(self) -> Option<f32> {
        match self {
            Self::Free | Self::Original => None,
            Self::Square => Some(1.0),
            Self::FourThree => Some(4.0 / 3.0),
            Self::ThreeTwo => Some(3.0 / 2.0),
            Self::SixteenNine => Some(16.0 / 9.0),
            Self::TwoOne => Some(2.0),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Orientation {
    Portrait,
    Landscape,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CropDrag {
    None,
    Draw, // drawing a fresh rect
    Rect, // dragging whole rect
    TL,
    TR,
    BL,
    BR, // corners
    T,
    B,
    L,
    R, // edges
}

/// A decoded source image at preview resolutions.
pub(crate) struct Loaded {
    pub(crate) base_small: emulsion_core::image::LinearImage,
    pub(crate) base_draft: emulsion_core::image::LinearImage,
    pub(crate) source_size: (usize, usize),
}

pub(crate) struct ThumbData {
    pub(crate) index: usize,
    pub(crate) base: emulsion_core::image::LinearImage,
    pub(crate) source_size: (usize, usize),
}

/// A row in the library tree: an image leaf (`file_index` set) or a folder
/// whose children are shown only while `expanded` contains its `path`.
pub(crate) struct TreeNode {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) children: Vec<TreeNode>,
    pub(crate) file_index: Option<usize>,
}

pub(crate) type ThumbMessage = (u64, Option<ThumbData>);

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Rect;

    #[test]
    fn every_slider_tool_has_a_movable_range() {
        // Regression: the Film (Develop) slider was mapped to
        // `invert_sensitivity` but kept the 0..=0 "no slider" range, so it
        // rendered without a rail and could not be dragged.
        let mut params = PipelineParams::default();
        for tool in [
            Tool::Film,
            Tool::Simulations,
            Tool::Wb,
            Tool::Tint,
            Tool::Ev,
            Tool::Contrast,
            Tool::Saturation,
            Tool::Shadows,
            Tool::Highlights,
            Tool::Vignette,
            Tool::Rotate,
            Tool::Crop,
        ] {
            if tool.value_mut(&mut params).is_none() {
                continue;
            }
            let range = tool.range();
            assert!(
                range.start() < range.end(),
                "{} slider has an empty range",
                tool.name()
            );
            assert!(
                range.contains(&0.0),
                "{} slider cannot reach its neutral value",
                tool.name()
            );
        }
    }

    fn draw_slider(
        ctx: &egui::Context,
        value: &mut f32,
        rect: &mut Rect,
        range: &std::ops::RangeInclusive<f32>,
        input: egui::RawInput,
    ) {
        ctx.run_ui(input, |ui| {
            *rect = ui
                .add(
                    egui::Slider::new(value, range.clone())
                        .show_value(false)
                        .step_by(1.0),
                )
                .rect;
        })
        .drop_without_applying_deltas();
    }

    #[test]
    fn film_slider_drag_updates_invert_sensitivity() {
        // Regression: the Film (Develop) panel's Output slider was wired to
        // `invert_sensitivity` but kept the 0..=0 "no slider" range, so it had
        // no rail and a drag left the value at 0.
        let ctx = egui::Context::default();
        let mut params = PipelineParams::default();
        let range = Tool::Film.range();
        let mut rect = Rect::NOTHING;
        let value = Tool::Film.value_mut(&mut params).unwrap();

        draw_slider(&ctx, value, &mut rect, &range, egui::RawInput::default());
        draw_slider(&ctx, value, &mut rect, &range, egui::RawInput::default());
        let start = rect.center();
        let end = egui::pos2(rect.right() - 4.0, rect.center().y);
        draw_slider(
            &ctx,
            value,
            &mut rect,
            &range,
            egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(start),
                    egui::Event::PointerButton {
                        pos: start,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                ],
                ..Default::default()
            },
        );
        draw_slider(
            &ctx,
            value,
            &mut rect,
            &range,
            egui::RawInput {
                events: vec![egui::Event::PointerMoved(end)],
                ..Default::default()
            },
        );
        draw_slider(
            &ctx,
            value,
            &mut rect,
            &range,
            egui::RawInput {
                events: vec![egui::Event::PointerButton {
                    pos: end,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }],
                ..Default::default()
            },
        );

        let moved = *value;
        assert!(moved > 50.0, "film slider did not move: {moved}");
        assert_eq!(params.invert_sensitivity, moved);
    }
}
