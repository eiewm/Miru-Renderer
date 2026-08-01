/// osu!mania skin coordinates are authored in a 640x480 logical space.
pub const SKIN_LOGICAL_HEIGHT: f32 = 480.0;

#[derive(Debug, Clone, Copy, Default)]
pub struct ColumnLayout {
    pub x: i32,
    pub width: u32,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct StageLayout {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub hit_y: i32,
    pub top_y: i32,
    pub bottom_y: i32,
    /// Top edge of the drawn playfield, which is not always the top of the box.
    ///
    /// `top_y`/`bottom_y`/`height` are the skin's stage: they set the scale of
    /// every HUD element and anchor the receptors and the stage graphics, so they
    /// have to keep the proportions the skin was authored with. In 16:9 that box
    /// happens to fill the canvas exactly, so the two coincide.
    ///
    /// In 9:16 it does not: the box is pushed down by the HUD band and ends short
    /// of the bottom edge, and a playfield drawn only inside it is a rectangle
    /// floating over the map art, with notes sliding in above its top edge and a
    /// strip of background left over below. So the fill, the column lines and the
    /// note clipping use this pair instead, and the box keeps doing the measuring.
    pub view_top_y: i32,
    pub view_bottom_y: i32,
}
#[derive(Debug, Clone, Default)]
pub struct ManiaLayoutInfo {
    pub stage: StageLayout,
    pub columns: Vec<ColumnLayout>,
    pub scale_y: f32,
    /// The skin's `HitPosition`, in its own 480-tall space, after any stage move.
    ///
    /// Scroll speed is calibrated against it, and it cannot be recovered from
    /// `hit_y`: in 9:16 the hit line is placed as a fraction of the canvas while
    /// `scale_y` comes from the width budget, so dividing one by the other lands
    /// past the end of the legacy space on any wide keycount and the calibration
    /// clamps, leaving the notes four times too fast.
    pub hit_position: f32,
    /// Gap from the hit line to the stage edge, in column pixels.
    pub key_area_px: i32,
    pub upside_down: bool,
    /// Where skin-authored HUD coordinates start.
    ///
    /// `ComboPosition` and `ScorePosition` in `skin.ini` are written against a
    /// stage that fills the canvas, so they are read from the top of the frame.
    /// In 9:16 the stage no longer starts there: the HUD band pushes it down, and
    /// without this offset the combo and the judgement would land that much
    /// higher than the skin intends. Zero on every horizontal canvas.
    pub hud_anchor_y: i32,
}
impl ManiaLayoutInfo {
    pub fn num_columns(&self) -> usize {
        self.columns.len()
    }
    /// Height of the drawn playfield.
    pub fn view_height(&self) -> u32 {
        (self.stage.view_bottom_y - self.stage.view_top_y).max(1) as u32
    }
    /// Scale that maps the skin's 480-tall logical space onto the drawn playfield.
    ///
    /// The stage graphics — `mania-stage-bottom`, `mania-stage-left`, its right
    /// twin — are authored against that space, so this is what sizes them. It is
    /// `scale_y` on every horizontal canvas, where the drawn playfield and the
    /// skin box are the same rectangle. In 9:16 `scale_y` comes from the width
    /// budget instead, and using it there put `mania-stage-bottom` a tenth of the
    /// frame below where the same skin puts it in 16:9.
    pub fn furniture_scale(&self) -> f32 {
        self.view_height() as f32 / SKIN_LOGICAL_HEIGHT
    }
    /// Distance a note covers between spawning and reaching the hit line. Notes
    /// enter from the top edge when scrolling down and from the bottom edge when
    /// scrolling up, so measuring it from the top in both cases collapses the
    /// upscroll playfield to the few pixels above the receptors.
    pub fn scroll_travel_px(&self) -> i32 {
        if self.upside_down {
            (self.stage.bottom_y - self.stage.hit_y).max(1)
        } else {
            (self.stage.hit_y - self.stage.top_y).max(1)
        }
    }
}
