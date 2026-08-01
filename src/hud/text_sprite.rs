#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextPrefix {
    Score,
    Combo,
    Accuracy,
}
#[derive(Debug, Clone)]
pub struct DigitPosition {
    pub ch: char,
    pub x: f32,
    pub width: f32,
    pub height: f32,
    pub asset_candidates: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct TextSpriteLayout {
    pub total_width: f32,
    pub height: f32,
    pub digits: Vec<DigitPosition>,
}
fn get_asset_candidates(
    ch: char,
    prefix: TextPrefix,
    score_prefix: &str,
    combo_prefix: &str,
) -> Vec<String> {
    let mut candidates = Vec::new();
    match ch {
        '0'..='9' => match prefix {
            // Try skin-specific prefixes first, then the osu! default score/combo names.
            TextPrefix::Score | TextPrefix::Accuracy => {
                candidates.push(format!("{}-{}.png", score_prefix, ch));
                candidates.push(format!("score-{}.png", ch));
            }
            TextPrefix::Combo => {
                candidates.push(format!("{}-{}.png", combo_prefix, ch));
                candidates.push(format!("combo-{}.png", ch));
            }
        },
        ',' => {
            if prefix == TextPrefix::Score || prefix == TextPrefix::Accuracy {
                candidates.push(format!("{}-comma.png", score_prefix));
                candidates.push("score-comma.png".to_string());
            }
        }
        '.' => {
            if prefix == TextPrefix::Score || prefix == TextPrefix::Accuracy {
                candidates.push(format!("{}-dot.png", score_prefix));
                candidates.push("score-dot.png".to_string());
            }
        }
        '%' => {
            if prefix == TextPrefix::Score || prefix == TextPrefix::Accuracy {
                candidates.push(format!("{}-percent.png", score_prefix));
                candidates.push("score-percent.png".to_string());
            }
        }
        'x' if prefix == TextPrefix::Combo => {
            candidates.push(format!("{}-x.png", combo_prefix));
            candidates.push("combo-x.png".to_string());
        }
        _ => {}
    }
    candidates
}
fn estimate_digit_width(ch: char, height: f32, aspect_ratios: Option<&[f32]>) -> f32 {
    let default_aspect = match ch {
        '1' => 0.4,
        '.' | ',' => 0.3,
        '%' => 0.8,
        _ => 0.6,
    };
    let aspect = aspect_ratios
        .and_then(|ratios| {
            let idx = match ch {
                '0'..='9' => (ch as usize) - ('0' as usize),
                _ => 10,
            };
            ratios.get(idx).copied()
        })
        .unwrap_or(default_aspect);
    height * aspect
}
pub fn compose_text_sprite_positions(
    text: &str,
    prefix: TextPrefix,
    height: f32,
    base_overlap: f32,
    native_digit_height: f32,
    score_prefix: &str,
    combo_prefix: &str,
    aspect_ratios: Option<&[f32]>,
) -> TextSpriteLayout {
    if text.is_empty() {
        return TextSpriteLayout {
            total_width: 0.0,
            height,
            digits: Vec::new(),
        };
    }
    let overlap_scaled = if native_digit_height > 0.0 {
        (base_overlap * (height / native_digit_height)).round()
    } else {
        base_overlap
    };
    let effective_overlap = match prefix {
        TextPrefix::Accuracy => 0.0,
        _ => overlap_scaled,
    };
    let mut digits: Vec<DigitPosition> = Vec::with_capacity(text.len());
    let mut x: f32 = 0.0;
    for (i, ch) in text.chars().enumerate() {
        let width = estimate_digit_width(ch, height, aspect_ratios);
        let candidates = get_asset_candidates(ch, prefix, score_prefix, combo_prefix);
        digits.push(DigitPosition {
            ch,
            x,
            width,
            height,
            asset_candidates: candidates,
        });
        if i < text.len() - 1 {
            x += width - effective_overlap;
        } else {
            x += width;
        }
    }
    TextSpriteLayout {
        total_width: x,
        height,
        digits,
    }
}
pub fn compose_text_sprite_positions_precise(
    text: &str,
    prefix: TextPrefix,
    digit_widths: &[f32],
    height: f32,
    base_overlap: f32,
    native_digit_height: f32,
    padding_top: f32,
    padding_bottom: f32,
) -> TextSpriteLayout {
    if text.is_empty() || digit_widths.len() != text.len() {
        return TextSpriteLayout {
            total_width: 0.0,
            height,
            digits: Vec::new(),
        };
    }
    let (effective_spacing, use_eed) = if prefix == TextPrefix::Score {
        // Score digits include transparent padding; EED spacing uses the visible glyph width.
        let total_padding = padding_top + padding_bottom;
        let eed = total_padding - base_overlap;
        let scaled_eed = if native_digit_height > 0.0 {
            eed * (height / native_digit_height)
        } else {
            eed
        };
        (scaled_eed, true)
    } else {
        let overlap_scaled = if native_digit_height > 0.0 {
            base_overlap * (height / native_digit_height)
        } else {
            base_overlap
        };
        let effective_overlap = match prefix {
            TextPrefix::Accuracy => 0.0,
            _ => overlap_scaled,
        };
        (effective_overlap, false)
    };
    let mut digits: Vec<DigitPosition> = Vec::with_capacity(text.len());
    let mut x: f32 = 0.0;
    for (i, ch) in text.chars().enumerate() {
        let width = digit_widths[i];
        digits.push(DigitPosition {
            ch,
            x,
            width,
            height,
            asset_candidates: Vec::new(),
        });
        if i < text.len() - 1 {
            if use_eed {
                let scaled_padding =
                    (padding_top + padding_bottom) * (height / native_digit_height.max(1.0));
                let visible_width = (width - scaled_padding).max(1.0);
                x += visible_width + effective_spacing;
            } else {
                x += width - effective_spacing;
            }
        } else {
            x += width;
        }
    }
    TextSpriteLayout {
        total_width: x,
        height,
        digits,
    }
}
