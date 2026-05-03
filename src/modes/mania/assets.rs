#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManiaFamily {
    One,
    Two,
    Special,
}
pub const MANIA_ASSETS: ManiaAssets = ManiaAssets {
    keys: KeyAssets {
        one: "mania-key1.png",
        two: "mania-key2.png",
        special: "mania-keyS.png",
        one_d: "mania-key1D.png",
        two_d: "mania-key2D.png",
        special_d: "mania-keySD.png",
    },
    notes: NoteAssets {
        one: "mania-note1.png",
        two: "mania-note2.png",
        special: "mania-noteS.png",
        one_h: "mania-note1H.png",
        two_h: "mania-note2H.png",
        special_h: "mania-noteSH.png",
        one_l: "mania-note1L.png",
        two_l: "mania-note2L.png",
        special_l: "mania-noteSL.png",
        one_t: "mania-note1T.png",
        two_t: "mania-note2T.png",
        special_t: "mania-noteST.png",
    },
    hits: HitAssets {
        hit0: "mania-hit0.png",
        hit50: "mania-hit50.png",
        hit100: "mania-hit100.png",
        hit200: "mania-hit200.png",
        hit300: "mania-hit300.png",
        hit300g: "mania-hit300g.png",
    },
    stage: StageAssets {
        left: "mania-stage-left.png",
        right: "mania-stage-right.png",
        bottom: "mania-stage-bottom.png",
        hint: "mania-stage-hint.png",
        light: "mania-stage-light.png",
        lighting_n: "lightingN.png",
        lighting_l: "lightingL.png",
    },
};
pub struct KeyAssets {
    pub one: &'static str,
    pub two: &'static str,
    pub special: &'static str,
    pub one_d: &'static str,
    pub two_d: &'static str,
    pub special_d: &'static str,
}
pub struct NoteAssets {
    pub one: &'static str,
    pub two: &'static str,
    pub special: &'static str,
    pub one_h: &'static str,
    pub two_h: &'static str,
    pub special_h: &'static str,
    pub one_l: &'static str,
    pub two_l: &'static str,
    pub special_l: &'static str,
    pub one_t: &'static str,
    pub two_t: &'static str,
    pub special_t: &'static str,
}
pub struct HitAssets {
    pub hit0: &'static str,
    pub hit50: &'static str,
    pub hit100: &'static str,
    pub hit200: &'static str,
    pub hit300: &'static str,
    pub hit300g: &'static str,
}
pub struct StageAssets {
    pub left: &'static str,
    pub right: &'static str,
    pub bottom: &'static str,
    pub hint: &'static str,
    pub light: &'static str,
    pub lighting_n: &'static str,
    pub lighting_l: &'static str,
}
pub struct ManiaAssets {
    pub keys: KeyAssets,
    pub notes: NoteAssets,
    pub hits: HitAssets,
    pub stage: StageAssets,
}
const PATTERNS: &[(u8, &[ManiaFamily])] = &[
    (
        4,
        &[
            ManiaFamily::One,
            ManiaFamily::Two,
            ManiaFamily::Two,
            ManiaFamily::One,
        ],
    ),
    (
        5,
        &[
            ManiaFamily::One,
            ManiaFamily::Two,
            ManiaFamily::Special,
            ManiaFamily::Two,
            ManiaFamily::One,
        ],
    ),
    (
        6,
        &[
            ManiaFamily::One,
            ManiaFamily::Two,
            ManiaFamily::One,
            ManiaFamily::One,
            ManiaFamily::Two,
            ManiaFamily::One,
        ],
    ),
    (
        7,
        &[
            ManiaFamily::One,
            ManiaFamily::Two,
            ManiaFamily::One,
            ManiaFamily::Special,
            ManiaFamily::One,
            ManiaFamily::Two,
            ManiaFamily::One,
        ],
    ),
    (
        8,
        &[
            ManiaFamily::One,
            ManiaFamily::Two,
            ManiaFamily::Two,
            ManiaFamily::One,
            ManiaFamily::One,
            ManiaFamily::Two,
            ManiaFamily::Two,
            ManiaFamily::One,
        ],
    ),
    (
        9,
        &[
            ManiaFamily::One,
            ManiaFamily::Two,
            ManiaFamily::One,
            ManiaFamily::Two,
            ManiaFamily::Special,
            ManiaFamily::Two,
            ManiaFamily::One,
            ManiaFamily::Two,
            ManiaFamily::One,
        ],
    ),
];
pub fn map_column_family(
    key_count: u8,
    column: u8,
    override_pattern: Option<&[ManiaFamily]>,
) -> ManiaFamily {
    if let Some(pattern) = override_pattern {
        if (column as usize) < pattern.len() {
            return pattern[column as usize];
        }
    }
    for &(kc, pattern) in PATTERNS {
        if kc == key_count && (column as usize) < pattern.len() {
            return pattern[column as usize];
        }
    }
    ManiaFamily::One
}
pub fn required_asset_names() -> Vec<&'static str> {
    vec![
        MANIA_ASSETS.keys.one,
        MANIA_ASSETS.keys.two,
        MANIA_ASSETS.keys.special,
        MANIA_ASSETS.keys.one_d,
        MANIA_ASSETS.keys.two_d,
        MANIA_ASSETS.keys.special_d,
        MANIA_ASSETS.notes.one,
        MANIA_ASSETS.notes.two,
        MANIA_ASSETS.notes.special,
        MANIA_ASSETS.notes.one_h,
        MANIA_ASSETS.notes.two_h,
        MANIA_ASSETS.notes.special_h,
        MANIA_ASSETS.notes.one_l,
        MANIA_ASSETS.notes.two_l,
        MANIA_ASSETS.notes.special_l,
        MANIA_ASSETS.notes.one_t,
        MANIA_ASSETS.notes.two_t,
        MANIA_ASSETS.notes.special_t,
        MANIA_ASSETS.hits.hit0,
        MANIA_ASSETS.hits.hit50,
        MANIA_ASSETS.hits.hit100,
        MANIA_ASSETS.hits.hit200,
        MANIA_ASSETS.hits.hit300,
        MANIA_ASSETS.hits.hit300g,
        MANIA_ASSETS.stage.left,
        MANIA_ASSETS.stage.right,
        MANIA_ASSETS.stage.bottom,
        MANIA_ASSETS.stage.hint,
        MANIA_ASSETS.stage.light,
        MANIA_ASSETS.stage.lighting_n,
        MANIA_ASSETS.stage.lighting_l,
    ]
}
