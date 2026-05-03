use super::config::IntroModBadgeSpec;
use super::mod_icons::{
    generated_mod_icon_is_excluded, generated_mod_icon_record, GENERATED_MOD_ICON_HEIGHT,
    GENERATED_MOD_ICON_TEXT_SHADOW, GENERATED_MOD_ICON_WIDTH,
};
use super::text::{
    draw_text_rgba, font_badge_value, font_bold, measure_text, render_text_simple_with_font,
    FontWeight,
};
use ab_glyph::PxScale;
use image::{imageops::FilterType, load_from_memory, Rgba, RgbaImage};
fn normalized_mod_abbr(abbr: &str) -> String {
    abbr.trim().to_ascii_uppercase()
}
fn clean_summary(summary: Option<&str>) -> Option<String> {
    let summary = summary?.trim();
    (!summary.is_empty()).then(|| summary.to_string())
}
fn is_key_count_badge(abbr: &str) -> bool {
    matches!(
        abbr,
        "1K" | "2K" | "3K" | "4K" | "5K" | "6K" | "7K" | "8K" | "9K"
    )
}
fn is_excluded_non_mania_badge(abbr: &str) -> bool {
    generated_mod_icon_is_excluded(abbr)
}
fn is_hidden_intro_badge(abbr: &str) -> bool {
    // SV1 is an internal "no scroll velocity change" marker, not a user-facing mod.
    matches!(abbr, "SV1")
}
pub fn mod_color(abbr: &str) -> [u8; 3] {
    generated_mod_icon_record(abbr)
        .map(|entry| entry.accent)
        .unwrap_or([0xA8, 0xB0, 0xC7])
}
pub fn decode_mods(bitmask: u32) -> Vec<&'static str> {
    // Replay mod masks use osu!'s legacy bit positions, including mania key-count bits.
    const MODS: &[(u32, &str)] = &[
        (1 << 0, "NF"),
        (1 << 1, "EZ"),
        (1 << 2, "TD"),
        (1 << 3, "HD"),
        (1 << 4, "HR"),
        (1 << 5, "SD"),
        (1 << 6, "DT"),
        (1 << 7, "RX"),
        (1 << 8, "HT"),
        (1 << 9, "NC"),
        (1 << 10, "FL"),
        (1 << 11, "AT"),
        (1 << 12, "SO"),
        (1 << 13, "AP"),
        (1 << 14, "PF"),
        (1 << 15, "4K"),
        (1 << 16, "5K"),
        (1 << 17, "6K"),
        (1 << 18, "7K"),
        (1 << 19, "8K"),
        (1 << 20, "FI"),
        (1 << 21, "RD"),
        (1 << 22, "CM"),
        (1 << 23, "TP"),
        (1 << 24, "9K"),
        (1 << 25, "CO"),
        (1 << 26, "1K"),
        (1 << 27, "3K"),
        (1 << 28, "2K"),
        (1 << 29, "V2"),
        (1 << 30, "MR"),
    ];
    let mut out = Vec::new();
    for &(bit, abbr) in MODS {
        if bitmask & bit != 0 {
            out.push(abbr);
        }
    }
    out
}
pub fn display_mods(bitmask: u32) -> Vec<&'static str> {
    let mut mods = decode_mods(bitmask);
    // Perfect and Nightcore imply Sudden Death and Double Time, but the intro shows only the stronger badge.
    if mods.contains(&"PF") {
        mods.retain(|mod_abbr| *mod_abbr != "SD");
    }
    if mods.contains(&"NC") {
        mods.retain(|mod_abbr| *mod_abbr != "DT");
    }
    mods.into_iter()
        .filter(|mod_abbr| !is_key_count_badge(mod_abbr) && !is_excluded_non_mania_badge(mod_abbr))
        .filter(|mod_abbr| !is_hidden_intro_badge(mod_abbr))
        .collect()
}
fn normalize_display_mods<I, S>(mods: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut out = Vec::new();
    for mod_abbr in mods {
        let normalized = normalized_mod_abbr(mod_abbr.as_ref());
        if normalized.is_empty() {
            continue;
        }
        if is_key_count_badge(&normalized)
            || is_excluded_non_mania_badge(&normalized)
            || is_hidden_intro_badge(&normalized)
        {
            continue;
        }
        if !out.iter().any(|existing| existing == &normalized) {
            out.push(normalized);
        }
    }
    if out.iter().any(|mod_abbr| mod_abbr == "PF") {
        out.retain(|mod_abbr| mod_abbr != "SD");
    }
    if out.iter().any(|mod_abbr| mod_abbr == "NC") {
        out.retain(|mod_abbr| mod_abbr != "DT");
    }
    out
}
fn normalize_intro_mod_badge_specs<I>(specs: I) -> Vec<IntroModBadgeSpec>
where
    I: IntoIterator<Item = IntroModBadgeSpec>,
{
    let mut out = Vec::new();
    for spec in specs {
        let normalized = normalized_mod_abbr(&spec.acronym);
        if normalized.is_empty() {
            continue;
        }
        if is_key_count_badge(&normalized)
            || is_excluded_non_mania_badge(&normalized)
            || is_hidden_intro_badge(&normalized)
        {
            continue;
        }
        let summary = clean_summary(spec.summary.as_deref());
        if let Some(existing) = out
            .iter_mut()
            .find(|existing: &&mut IntroModBadgeSpec| existing.acronym == normalized)
        {
            // Keep the first occurrence unless a later duplicate carries the only summary text.
            if existing.summary.is_none() && summary.is_some() {
                existing.summary = summary;
                existing.summary_priority = spec.summary_priority;
            }
            continue;
        }
        out.push(IntroModBadgeSpec {
            acronym: normalized,
            summary,
            summary_priority: spec.summary_priority,
        });
    }
    if out.iter().any(|spec| spec.acronym == "PF") {
        out.retain(|spec| spec.acronym != "SD");
    }
    if out.iter().any(|spec| spec.acronym == "NC") {
        out.retain(|spec| spec.acronym != "DT");
    }
    out
}
pub struct Badge {
    pub image: RgbaImage,
    pub width: u32,
    pub height: u32,
}
fn create_asset_badge(abbr: &str, size: u32) -> Option<Badge> {
    let source = load_from_memory(generated_mod_icon_record(abbr)?.asset_bytes)
        .ok()?
        .into_rgba8();
    create_badge_from_source_image(source, size)
}
fn create_badge_from_source_image(source: RgbaImage, size: u32) -> Option<Badge> {
    let target_height = size.max(1);
    let aspect_ratio = source.width() as f32 / source.height() as f32;
    let target_width = ((target_height as f32 * aspect_ratio).round() as u32).max(1);
    let resized =
        image::imageops::resize(&source, target_width, target_height, FilterType::Lanczos3);
    Some(Badge {
        image: resized,
        width: target_width,
        height: target_height,
    })
}
fn create_fallback_mod_badge(abbr: &str, size: u32) -> Option<Badge> {
    let font = font_bold()?;
    let h = size.max(1);
    let font_size = if abbr.len() >= 3 {
        (size as f32 * 0.28).round()
    } else {
        (size as f32 * 0.34).round()
    };
    let scale = PxScale::from(font_size);
    let text_w = measure_text(abbr, font_size, FontWeight::Bold);
    let min_w = ((GENERATED_MOD_ICON_WIDTH as f32 / GENERATED_MOD_ICON_HEIGHT as f32) * h as f32)
        .round() as u32;
    let w = min_w.max((text_w + size as f32 * 0.75).round() as u32);
    let accent = mod_color(abbr);
    let mut img = RgbaImage::new(w, h);
    let radius = (size as f32 * 0.2) as u32;
    draw_rounded_rect(&mut img, 0, 0, w, h, radius, Rgba([28, 29, 25, 255]));
    if w > 4 && h > 4 {
        draw_rounded_rect(
            &mut img,
            2,
            2,
            w - 4,
            h - 4,
            radius.saturating_sub(2),
            Rgba([43, 44, 37, 255]),
        );
    }
    if w > 10 && h > 10 {
        draw_rounded_rect(
            &mut img,
            5,
            5,
            w - 10,
            h - 10,
            radius.saturating_sub(5),
            Rgba([33, 34, 29, 255]),
        );
    }
    let tx = ((w as f32 - text_w) / 2.0).round() as i32;
    let ty = ((h as f32 - font_size) / 2.0 - font_size * 0.1).round() as i32;
    draw_text_rgba(
        &mut img,
        Rgba(GENERATED_MOD_ICON_TEXT_SHADOW),
        tx,
        ty + 1,
        scale,
        font,
        abbr,
    );
    draw_text_rgba(
        &mut img,
        Rgba([accent[0], accent[1], accent[2], 255]),
        tx,
        ty,
        scale,
        font,
        abbr,
    );
    Some(Badge {
        image: img,
        width: w,
        height: h,
    })
}
fn alpha_blit(canvas: &mut RgbaImage, sprite: &RgbaImage, x: i32, y: i32) {
    for iy in 0..sprite.height() {
        let dy = y + iy as i32;
        if dy < 0 || dy >= canvas.height() as i32 {
            continue;
        }
        for ix in 0..sprite.width() {
            let dx = x + ix as i32;
            if dx < 0 || dx >= canvas.width() as i32 {
                continue;
            }
            let src = sprite.get_pixel(ix, iy).0;
            let src_a = src[3] as f32 / 255.0;
            if src_a < 0.001 {
                continue;
            }
            let dst = canvas.get_pixel(dx as u32, dy as u32).0;
            let dst_a = dst[3] as f32 / 255.0;
            let out_a = src_a + dst_a * (1.0 - src_a);
            if out_a <= 0.001 {
                continue;
            }
            let inv = 1.0 / out_a;
            canvas.put_pixel(
                dx as u32,
                dy as u32,
                Rgba([
                    ((src[0] as f32 * src_a + dst[0] as f32 * dst_a * (1.0 - src_a)) * inv) as u8,
                    ((src[1] as f32 * src_a + dst[1] as f32 * dst_a * (1.0 - src_a)) * inv) as u8,
                    ((src[2] as f32 * src_a + dst[2] as f32 * dst_a * (1.0 - src_a)) * inv) as u8,
                    (out_a * 255.0) as u8,
                ]),
            );
        }
    }
}
fn trim_transparent_bounds(image: &RgbaImage) -> RgbaImage {
    let mut min_x = image.width();
    let mut min_y = image.height();
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut found = false;
    for y in 0..image.height() {
        for x in 0..image.width() {
            if image.get_pixel(x, y).0[3] == 0 {
                continue;
            }
            found = true;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    if !found {
        return image.clone();
    }
    image::imageops::crop_imm(image, min_x, min_y, max_x - min_x + 1, max_y - min_y + 1).to_image()
}
fn create_summary_badge(spec: &IntroModBadgeSpec, size: u32) -> Option<Badge> {
    let acronym = normalized_mod_abbr(&spec.acronym);
    let summary = clean_summary(spec.summary.as_deref())?;
    let badge =
        create_asset_badge(&acronym, size).or_else(|| create_fallback_mod_badge(&acronym, size))?;
    create_summary_badge_from_base(&acronym, &summary, badge, size)
}
fn create_summary_badge_from_base(
    acronym: &str,
    summary: &str,
    badge: Badge,
    size: u32,
) -> Option<Badge> {
    let accent = mod_color(acronym);
    let horizontal_padding = ((size.max(1) as f32) * 0.18).round().max(5.0) as u32;
    let vertical_padding = ((size.max(1) as f32) * 0.08).round().max(3.0) as u32;
    let gap = ((size.max(1) as f32) * 0.07).round().max(2.0) as u32;
    let summary_box_width = ((size.max(1) as f32) * 1.75).round().max(20.0) as u32;
    let summary_shadow = render_compact_summary_text(
        summary,
        summary_box_width,
        badge.height,
        [0x00, 0x00, 0x00, 210],
    )?;
    let summary_text = render_compact_summary_text(
        summary,
        summary_box_width,
        badge.height,
        [accent[0], accent[1], accent[2], 255],
    )?;
    let summary_panel_width = (summary_text.width + horizontal_padding * 2)
        .max((size.max(1) as f32 * 0.85).round() as u32);
    let summary_panel_height = summary_text
        .height
        .max(summary_shadow.height)
        .saturating_add(vertical_padding * 2)
        .max(((size.max(1) as f32) * 0.32).round() as u32);
    let total_width = badge.width.max(summary_panel_width);
    let total_height = badge
        .height
        .saturating_add(gap)
        .saturating_add(summary_panel_height);
    let mut composed = RgbaImage::new(total_width, total_height);
    let panel_x = ((total_width as i32 - summary_panel_width as i32) / 2).max(0) as u32;
    let panel_y = badge.height + gap;
    let radius = ((summary_panel_height as f32) * 0.5).round().max(4.0) as u32;
    draw_rounded_rect(
        &mut composed,
        panel_x,
        panel_y,
        summary_panel_width,
        summary_panel_height,
        radius,
        Rgba([12, 13, 14, 205]),
    );
    let badge_x = ((total_width as i32 - badge.width as i32) / 2).max(0);
    alpha_blit(&mut composed, &badge.image, badge_x, 0);
    let summary_x =
        panel_x as i32 + ((summary_panel_width as i32 - summary_text.width as i32) / 2).max(0);
    let summary_y =
        panel_y as i32 + ((summary_panel_height as i32 - summary_text.height as i32) / 2).max(0);
    alpha_blit(
        &mut composed,
        &summary_shadow.image,
        summary_x + 1,
        summary_y + 1,
    );
    alpha_blit(
        &mut composed,
        &summary_text.image,
        summary_x,
        summary_y.max(0),
    );
    Some(Badge {
        image: composed,
        width: total_width,
        height: total_height,
    })
}
fn render_compact_summary_text(
    summary: &str,
    max_width: u32,
    height: u32,
    color: [u8; 4],
) -> Option<super::text::RenderedText> {
    let compact_summary = compact_summary_text(summary);
    let max_width = max_width.max(14);
    let mut font_size = (height as f32 * 0.23).max(7.5);
    let font = font_badge_value()?;
    while font_size >= 7.0 {
        let rendered = render_text_simple_with_font(&compact_summary, font_size, color, font)?;
        let trimmed = trim_transparent_bounds(&rendered.image);
        if trimmed.width() <= max_width {
            return Some(super::text::RenderedText {
                width: trimmed.width(),
                height: trimmed.height(),
                image: trimmed,
            });
        }
        font_size -= 0.5;
    }
    let rendered = render_text_simple_with_font(&compact_summary, 7.0, color, font)?;
    let trimmed = trim_transparent_bounds(&rendered.image);
    Some(super::text::RenderedText {
        width: trimmed.width(),
        height: trimmed.height(),
        image: trimmed,
    })
}
fn compact_summary_text(summary: &str) -> String {
    let mut compact = summary.trim().replace("->", ">");
    if compact.matches('.').count() >= 2 {
        compact = compact
            .split('>')
            .map(compact_numeric_token)
            .collect::<Vec<_>>()
            .join(">");
    } else if compact.contains('.') {
        compact = compact_numeric_token(&compact);
    }
    compact
}
fn compact_numeric_token(token: &str) -> String {
    let mut suffix = String::new();
    let mut number = token.trim().to_string();
    while let Some(last) = number.chars().last() {
        if last.is_ascii_alphabetic() || last == '%' {
            suffix.insert(0, last);
            number.pop();
        } else {
            break;
        }
    }
    if let Some((whole, fractional)) = number.split_once('.') {
        let trimmed_fractional = fractional.trim_end_matches('0');
        if trimmed_fractional.is_empty() {
            return format!("{whole}{suffix}");
        }
        return format!("{whole}.{trimmed_fractional}{suffix}");
    }
    format!("{number}{suffix}")
}
pub fn create_mod_badge_from_spec(spec: &IntroModBadgeSpec, size: u32) -> Option<Badge> {
    let normalized = normalized_mod_abbr(&spec.acronym);
    if normalized.is_empty() || is_hidden_intro_badge(&normalized) {
        return None;
    }
    let has_summary = clean_summary(spec.summary.as_deref()).is_some();
    if has_summary {
        create_summary_badge(spec, size)
            .or_else(|| create_asset_badge(&normalized, size))
            .or_else(|| create_fallback_mod_badge(&normalized, size))
    } else {
        create_asset_badge(&normalized, size)
            .or_else(|| create_fallback_mod_badge(&normalized, size))
    }
}
pub fn create_mod_badge_from_image_spec(
    spec: &IntroModBadgeSpec,
    size: u32,
    source: RgbaImage,
) -> Option<Badge> {
    let normalized = normalized_mod_abbr(&spec.acronym);
    if normalized.is_empty() || is_hidden_intro_badge(&normalized) {
        return None;
    }
    let badge = create_badge_from_source_image(source, size)?;
    match clean_summary(spec.summary.as_deref()) {
        Some(summary) => create_summary_badge_from_base(&normalized, &summary, badge, size),
        None => Some(badge),
    }
}
pub fn create_mod_badge(abbr: &str, size: u32) -> Option<Badge> {
    let normalized = normalized_mod_abbr(abbr);
    if normalized.is_empty() || is_hidden_intro_badge(&normalized) {
        return None;
    }
    create_asset_badge(&normalized, size).or_else(|| create_fallback_mod_badge(&normalized, size))
}
pub fn create_key_badge(key_count: u8, star_rating: Option<f32>) -> Option<Badge> {
    let font = font_bold()?;
    let w = 55u32;
    let h = 32u32;
    let font_size = 16.0f32;
    let scale = PxScale::from(font_size);
    let bg = star_rating
        .map(star_rating_color_rgb)
        .unwrap_or([0x63, 0x66, 0xf1]);
    let mut img = RgbaImage::new(w, h);
    draw_rounded_rect(&mut img, 0, 0, w, h, 8, Rgba([bg[0], bg[1], bg[2], 255]));
    let text = format!("{}K", key_count);
    let text_w = measure_text(&text, font_size, FontWeight::Bold);
    let tx = ((w as f32 - text_w) / 2.0).round() as i32;
    let ty = ((h as f32 - font_size) / 2.0 - font_size * 0.1).round() as i32;
    draw_text_rgba(
        &mut img,
        Rgba([255, 255, 255, 255]),
        tx,
        ty,
        scale,
        font,
        &text,
    );
    Some(Badge {
        image: img,
        width: w,
        height: h,
    })
}
pub fn create_mod_badges_from_specs(specs: &[IntroModBadgeSpec], size: u32) -> Vec<Badge> {
    let specs = normalize_intro_mod_badge_specs(specs.iter().cloned());
    if specs.is_empty() {
        return create_mod_badge("NM", size).into_iter().collect();
    }
    specs
        .iter()
        .filter_map(|spec| create_mod_badge_from_spec(spec, size))
        .collect()
}
fn total_badge_width_for_specs(specs: &[IntroModBadgeSpec], size: u32, spacing: u32) -> u32 {
    let badges = create_mod_badges_from_specs(specs, size);
    badges.iter().map(|badge| badge.width).sum::<u32>()
        + spacing.saturating_mul(badges.len().saturating_sub(1) as u32)
}
pub fn fit_intro_mod_badge_specs_to_width(
    specs: &[IntroModBadgeSpec],
    size: u32,
    spacing: u32,
    max_total_width: u32,
) -> Vec<IntroModBadgeSpec> {
    let mut fitted = normalize_intro_mod_badge_specs(specs.iter().cloned());
    if fitted.is_empty() {
        return vec![IntroModBadgeSpec::new("NM")];
    }
    while total_badge_width_for_specs(&fitted, size, spacing) > max_total_width {
        let current_width = total_badge_width_for_specs(&fitted, size, spacing);
        // Lower-priority summaries are removed first so the core mod icon order stays stable.
        let Some(index) = fitted
            .iter()
            .enumerate()
            .filter(|(_, spec)| spec.summary.is_some())
            .map(|(index, spec)| {
                let mut candidate = fitted.clone();
                candidate[index].summary = None;
                (
                    index,
                    spec.summary_priority,
                    total_badge_width_for_specs(&candidate, size, spacing),
                )
            })
            .filter(|(_, _, width_after)| *width_after < current_width)
            .min_by_key(|(_, priority, width_after)| (*priority, *width_after))
            .map(|(index, _, _)| index)
        else {
            break;
        };
        fitted[index].summary = None;
    }
    fitted
}
pub fn create_mod_badges(bitmask: u32, size: u32) -> Vec<Badge> {
    let mods = normalize_display_mods(display_mods(bitmask));
    if mods.is_empty() {
        return create_mod_badge("NM", size).into_iter().collect();
    }
    mods.iter()
        .filter_map(|mod_abbr| create_mod_badge(mod_abbr, size))
        .collect()
}
pub fn create_mod_badges_from_list(mods: &[String], size: u32) -> Vec<Badge> {
    let specs = normalize_display_mods(mods.iter().map(String::as_str))
        .into_iter()
        .map(IntroModBadgeSpec::new)
        .collect::<Vec<_>>();
    create_mod_badges_from_specs(&specs, size)
}
fn star_rating_color_rgb(sr: f32) -> [u8; 3] {
    // These stops mirror osu!'s difficulty color ramp so key badges match familiar star colors.
    const DOMAIN: &[f32] = &[0.0, 0.1, 1.25, 2.0, 2.5, 3.25, 4.5, 6.0, 6.75, 7.75, 9.0];
    const COLORS: &[[u8; 3]] = &[
        [0xAA, 0xAA, 0xAA],
        [0x4F, 0xC0, 0xFF],
        [0x4F, 0xC0, 0xFF],
        [0x4F, 0xFF, 0xD5],
        [0x7C, 0xFF, 0x4F],
        [0xF6, 0xF0, 0x5C],
        [0xFF, 0x80, 0x68],
        [0xFF, 0x4E, 0x6F],
        [0xC6, 0x45, 0xB8],
        [0x65, 0x63, 0xDE],
        [0x18, 0x15, 0x8E],
    ];
    for i in 0..DOMAIN.len() - 1 {
        if sr >= DOMAIN[i] && sr < DOMAIN[i + 1] {
            let t = (sr - DOMAIN[i]) / (DOMAIN[i + 1] - DOMAIN[i]);
            return interp_color(COLORS[i], COLORS[i + 1], t);
        }
    }
    *COLORS.last().unwrap()
}
fn interp_color(c1: [u8; 3], c2: [u8; 3], t: f32) -> [u8; 3] {
    [
        (c1[0] as f32 + (c2[0] as f32 - c1[0] as f32) * t).round() as u8,
        (c1[1] as f32 + (c2[1] as f32 - c1[1] as f32) * t).round() as u8,
        (c1[2] as f32 + (c2[2] as f32 - c1[2] as f32) * t).round() as u8,
    ]
}
fn draw_rounded_rect(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, r: u32, color: Rgba<u8>) {
    let r = r.min(w / 2).min(h / 2);
    if h > 2 * r {
        fill_rect(img, x, y + r, w, h - 2 * r, color);
    }
    if w > 2 * r {
        fill_rect(img, x + r, y, w - 2 * r, r, color);
        fill_rect(img, x + r, y + h - r, w - 2 * r, r, color);
    }
    draw_corner(img, x + r, y + r, r, color, 0);
    draw_corner(img, x + w - r - 1, y + r, r, color, 1);
    draw_corner(img, x + r, y + h - r - 1, r, color, 2);
    draw_corner(img, x + w - r - 1, y + h - r - 1, r, color, 3);
}
fn fill_rect(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, color: Rgba<u8>) {
    let max_x = (x.saturating_add(w)).min(img.width());
    let max_y = (y.saturating_add(h)).min(img.height());
    for py in y..max_y {
        for px in x..max_x {
            img.put_pixel(px, py, color);
        }
    }
}
fn draw_corner(img: &mut RgbaImage, cx: u32, cy: u32, r: u32, color: Rgba<u8>, quadrant: u8) {
    let r2 = (r * r) as i32;
    for dy in 0..=r {
        for dx in 0..=r {
            let dist2 = (dx * dx + dy * dy) as i32;
            if dist2 <= r2 {
                let (px, py) = match quadrant {
                    0 => (cx - dx, cy - dy),
                    1 => (cx + dx, cy - dy),
                    2 => (cx - dx, cy + dy),
                    _ => (cx + dx, cy + dy),
                };
                if px < img.width() && py < img.height() {
                    img.put_pixel(px, py, color);
                }
            }
        }
    }
}
