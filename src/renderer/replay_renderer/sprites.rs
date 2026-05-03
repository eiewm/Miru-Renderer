use crate::renderer::gpu::SpriteBlendMode;
pub type Tint = [f32; 4];
#[derive(Debug, Clone)]
pub struct SpriteCommand {
    pub texture_id: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub precise_position: Option<[f32; 2]>,
    pub precise_size: Option<[f32; 2]>,
    pub tint: Tint,
    pub uv_rect: [f32; 4],
    pub origin: [f32; 2],
    pub rotation: f32,
    pub z_order: i32,
    pub blend_mode: SpriteBlendMode,
}
impl SpriteCommand {
    pub fn new(texture_id: impl Into<String>, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            texture_id: texture_id.into(),
            x,
            y,
            width,
            height,
            precise_position: None,
            precise_size: None,
            tint: [1.0, 1.0, 1.0, 1.0],
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            origin: [0.0, 0.0],
            rotation: 0.0,
            z_order: 0,
            blend_mode: SpriteBlendMode::Alpha,
        }
    }
    pub fn with_tint(mut self, tint: Tint) -> Self {
        self.tint = tint;
        self
    }
    pub fn with_uv(mut self, uv: [f32; 4]) -> Self {
        self.uv_rect = uv;
        self
    }
    pub fn with_origin(mut self, ox: f32, oy: f32) -> Self {
        self.origin = [ox, oy];
        self
    }
    pub fn with_rotation(mut self, rot: f32) -> Self {
        self.rotation = rot;
        self
    }
    pub fn with_z(mut self, z: i32) -> Self {
        self.z_order = z;
        self
    }
}
impl Default for SpriteCommand {
    fn default() -> Self {
        Self {
            texture_id: String::new(),
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            precise_position: None,
            precise_size: None,
            tint: [1.0, 1.0, 1.0, 1.0],
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            origin: [0.0, 0.0],
            rotation: 0.0,
            z_order: 0,
            blend_mode: SpriteBlendMode::Alpha,
        }
    }
}
#[derive(Debug, Clone)]
pub struct LnBodyCommand {
    pub texture_id: String,
    pub x: i32,
    pub top_y: i32,
    pub bottom_y: i32,
    pub width: u32,
    pub tile_h: u32,
    // Phase offsets the first tile so scrolling long-note bodies stay visually continuous.
    pub phase: u32,
    pub tint: Tint,
    pub z_order: i32,
}
#[derive(Debug, Clone)]
pub enum RenderCommand {
    Sprite(SpriteCommand),
    LnBody(LnBodyCommand),
}
impl RenderCommand {
    #[inline]
    pub fn z_order(&self) -> i32 {
        match self {
            RenderCommand::Sprite(s) => s.z_order,
            RenderCommand::LnBody(b) => b.z_order,
        }
    }
}
#[derive(Debug, Default)]
pub struct SpritePlanner {
    commands: Vec<RenderCommand>,
}
impl SpritePlanner {
    pub fn new() -> Self {
        Self {
            commands: Vec::with_capacity(512),
        }
    }
    pub fn clear(&mut self) {
        self.commands.clear();
    }
    pub fn add_sprite(&mut self, cmd: SpriteCommand) {
        self.commands.push(RenderCommand::Sprite(cmd));
    }
    pub fn add_ln_body(&mut self, cmd: LnBodyCommand) {
        self.commands.push(RenderCommand::LnBody(cmd));
    }
    pub fn sorted(&mut self) -> &[RenderCommand] {
        self.commands.sort_by_key(|c| c.z_order());
        &self.commands
    }
    pub fn commands_ref(&self) -> &[RenderCommand] {
        &self.commands
    }
    pub fn len(&self) -> usize {
        self.commands.len()
    }
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}
pub mod z_order {
    // Lower values render first; gaps leave room for gameplay and storyboard layers to interleave.
    pub const BACKGROUND: i32 = -1000;
    pub const STORYBOARD_BACKGROUND: i32 = -900;
    pub const STORYBOARD_FAIL: i32 = -890;
    pub const STORYBOARD_PASS: i32 = -880;
    pub const COMBO_BURST: i32 = -1;
    pub const COLUMN_BG: i32 = 0;
    pub const BARLINE: i32 = 5;
    pub const COLUMN_LINE: i32 = 6;
    pub const KEYS: i32 = 10;
    pub const LN_BODY: i32 = 15;
    pub const NORMAL_NOTE: i32 = 20;
    pub const LN_TAIL: i32 = 22;
    pub const LN_HEAD: i32 = 25;
    pub const LIGHTING_N: i32 = 32;
    pub const LIGHTING_L: i32 = 33;
    pub const STAGE_LIGHT: i32 = 34;
    pub const STAGE_HINT: i32 = 31;
    pub const STAGE_EDGE: i32 = 35;
    pub const STAGE_BOTTOM: i32 = 36;
    pub const WARNING_ARROW: i32 = 37;
    pub const PLAYFIELD_COVER: i32 = 150;
    pub const STORYBOARD_FOREGROUND: i32 = -870;
    pub const STORYBOARD_OVERLAY: i32 = 45;
    pub const JUDGMENT_POPUP: i32 = 50;
    pub const KEYS_OVER_NOTES: i32 = 30;
    pub const KEY_PRESS_OVERLAY: i32 = 30;
    pub const HUD: i32 = 200;
}
