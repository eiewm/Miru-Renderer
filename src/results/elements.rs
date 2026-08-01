//! The pieces of the built-in results screen, each with a name a HUD layer can
//! point at. A design that carries `results.*` layers composes the screen out
//! of them, so any piece can be moved, resized or left out.

use super::animation::{AnimatedElement, ResultsAnimationState};
use image::RgbaImage;

/// Judgment rows in the order `ResultsLayout` lays them out.
const JUDGMENT_IDS: [&str; 6] = [
    "results.judgment300",
    "results.judgment200",
    "results.judgment50",
    "results.judgmentMax",
    "results.judgment100",
    "results.judgmentMiss",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResultsElement {
    Background,
    Panel,
    TitleBar,
    TitleLogo,
    Title,
    Mapper,
    Player,
    Mods,
    Score,
    Rank,
    Judgment(usize),
    Combo,
    Accuracy,
    Graph,
    Timing,
    Perfect,
}

/// Every element, in the order the built-in screen draws them.
pub(crate) const RESULTS_ELEMENTS: [ResultsElement; 21] = [
    ResultsElement::Background,
    ResultsElement::Panel,
    ResultsElement::TitleBar,
    ResultsElement::TitleLogo,
    ResultsElement::Title,
    ResultsElement::Mapper,
    ResultsElement::Player,
    ResultsElement::Mods,
    ResultsElement::Score,
    ResultsElement::Rank,
    ResultsElement::Judgment(0),
    ResultsElement::Judgment(1),
    ResultsElement::Judgment(2),
    ResultsElement::Judgment(3),
    ResultsElement::Judgment(4),
    ResultsElement::Judgment(5),
    ResultsElement::Combo,
    ResultsElement::Accuracy,
    ResultsElement::Graph,
    ResultsElement::Timing,
    ResultsElement::Perfect,
];

impl ResultsElement {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Background => "results.background",
            Self::Panel => "results.panel",
            Self::TitleBar => "results.titleBar",
            Self::TitleLogo => "results.titleLogo",
            Self::Title => "results.title",
            Self::Mapper => "results.mapper",
            Self::Player => "results.player",
            Self::Mods => "results.mods",
            Self::Score => "results.score",
            Self::Rank => "results.rank",
            Self::Judgment(index) => JUDGMENT_IDS[index.min(JUDGMENT_IDS.len() - 1)],
            Self::Combo => "results.combo",
            Self::Accuracy => "results.accuracy",
            Self::Graph => "results.graph",
            Self::Timing => "results.timing",
            Self::Perfect => "results.perfect",
        }
    }

    pub(crate) fn from_layer_type(layer_type: &str) -> Option<Self> {
        RESULTS_ELEMENTS
            .into_iter()
            .find(|element| element.id() == layer_type)
    }

    /// The slot of the entry animation this element rides in on. The background
    /// has none: it is the floor everything else lands on.
    pub(crate) fn animation(self, state: &ResultsAnimationState) -> AnimatedElement {
        match self {
            Self::Background => AnimatedElement {
                alpha: 1.0,
                offset_x: 0.0,
                offset_y: 0.0,
            },
            Self::Panel => state.panel,
            Self::TitleBar | Self::TitleLogo | Self::Title | Self::Mapper | Self::Player
            | Self::Mods => state.title,
            Self::Score => state.score,
            Self::Rank => state.grade,
            Self::Judgment(index) => state.judgments[index.min(state.judgments.len() - 1)],
            Self::Combo => state.combo,
            Self::Accuracy => state.accuracy,
            Self::Graph => state.graph_frame,
            Self::Timing => state.timing,
            Self::Perfect => state.perfect,
        }
    }
}

/// One element drawn on its own and cropped to what it covers.
pub(crate) struct ResultsElementSprite {
    pub(crate) element: ResultsElement,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) image: RgbaImage,
}

/// Looks up sprites by element, which is what drawing a layer needs.
#[derive(Default)]
pub(crate) struct ResultsElementSprites {
    sprites: Vec<ResultsElementSprite>,
}

impl ResultsElementSprites {
    pub(crate) fn new(sprites: Vec<ResultsElementSprite>) -> Self {
        Self { sprites }
    }

    pub(crate) fn get(&self, element: ResultsElement) -> Option<&ResultsElementSprite> {
        self.sprites
            .iter()
            .find(|sprite| sprite.element == element)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.sprites.is_empty()
    }
}

/// Where a sprite ended up inside the atlas.
pub(crate) struct ResultsAtlasPlacement {
    pub(crate) element: ResultsElement,
    pub(crate) atlas_x: u32,
    pub(crate) atlas_y: u32,
}

pub(crate) struct ResultsElementAtlas {
    pub(crate) image: RgbaImage,
    pub(crate) placements: Vec<ResultsAtlasPlacement>,
}

/// Gutter between sprites, so scaling one in the editor cannot smear a
/// neighbour into its edge.
const ATLAS_GUTTER: u32 = 2;

/// Packs the sprites into shelves. The background stays out: it is already the
/// preview image, and copying a whole canvas in here would double the download.
pub(crate) fn pack_element_atlas(
    sprites: &[ResultsElementSprite],
    max_width: u32,
) -> Option<ResultsElementAtlas> {
    let max_width = max_width.max(256);
    let mut packed: Vec<&ResultsElementSprite> = sprites
        .iter()
        .filter(|sprite| sprite.element != ResultsElement::Background)
        .collect();
    if packed.is_empty() {
        return None;
    }
    // Tallest first keeps the shelves from stacking up wasted rows.
    packed.sort_by_key(|sprite| std::cmp::Reverse(sprite.image.height()));

    let mut placements = Vec::with_capacity(packed.len());
    let mut cursor_x = 0u32;
    let mut cursor_y = 0u32;
    let mut shelf_height = 0u32;
    let mut atlas_width = 0u32;
    for sprite in &packed {
        let width = sprite.image.width();
        let height = sprite.image.height();
        if cursor_x > 0 && cursor_x + width > max_width {
            cursor_x = 0;
            cursor_y += shelf_height + ATLAS_GUTTER;
            shelf_height = 0;
        }
        placements.push(ResultsAtlasPlacement {
            element: sprite.element,
            atlas_x: cursor_x,
            atlas_y: cursor_y,
        });
        cursor_x += width + ATLAS_GUTTER;
        shelf_height = shelf_height.max(height);
        atlas_width = atlas_width.max(cursor_x.saturating_sub(ATLAS_GUTTER));
    }
    let atlas_height = cursor_y + shelf_height;

    let mut image = RgbaImage::from_pixel(
        atlas_width.max(1),
        atlas_height.max(1),
        image::Rgba([0, 0, 0, 0]),
    );
    for (sprite, placement) in packed.iter().zip(placements.iter()) {
        image::imageops::replace(
            &mut image,
            &sprite.image,
            placement.atlas_x as i64,
            placement.atlas_y as i64,
        );
    }
    Some(ResultsElementAtlas { image, placements })
}

/// The box the visible pixels sit in, or `None` when nothing was drawn.
pub(crate) fn opaque_bounds(image: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel[3] == 0 {
            continue;
        }
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    (min_x != u32::MAX).then(|| (min_x, min_y, max_x - min_x + 1, max_y - min_y + 1))
}

#[cfg(test)]
mod tests;
