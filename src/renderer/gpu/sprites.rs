use bytemuck::{Pod, Zeroable};
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SpriteInstance {
    // This layout is shared with WGSL vertex attributes, so field order and padding are part of the ABI.
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub uv_rect: [f32; 4],
    pub color: [f32; 4],
    pub origin: [f32; 2],
    pub rotation: f32,
    pub _pad: f32,
}
impl SpriteInstance {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            position: [x, y],
            size: [w, h],
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
            origin: [0.0, 0.0],
            rotation: 0.0,
            _pad: 0.0,
        }
    }
    pub fn with_uv(mut self, u_min: f32, v_min: f32, u_max: f32, v_max: f32) -> Self {
        self.uv_rect = [u_min, v_min, u_max, v_max];
        self
    }
    pub fn with_origin(mut self, ox: f32, oy: f32) -> Self {
        self.origin = [ox, oy];
        self
    }
    pub fn with_rotation(mut self, rotation: f32) -> Self {
        self.rotation = rotation;
        self
    }
    pub fn with_color(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.color = [r, g, b, a];
        self
    }
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.color[3] = opacity;
        self
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SpriteBlendMode {
    #[default]
    Alpha,
    Additive,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Uniforms {
    pub screen_size: [f32; 2],
    pub _padding: [f32; 2],
}
impl Uniforms {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            screen_size: [width, height],
            _padding: [0.0, 0.0],
        }
    }
}
// Keep per-draw instance buffers below conservative WebGPU limits across integrated GPUs.
pub const MAX_INSTANCES: usize = 32768;
pub fn sprite_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<SpriteInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: 8,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: 16,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 32,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 48,
                shader_location: 4,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: 56,
                shader_location: 5,
                format: wgpu::VertexFormat::Float32,
            },
        ],
    }
}
pub struct SpriteBatch {
    pub texture_key: usize,
    pub start: u32,
    pub count: u32,
}
#[derive(Default)]
pub struct BatchBuilder {
    pub instances: Vec<SpriteInstance>,
    pub batches: Vec<(usize, u32, u32)>,
}
impl BatchBuilder {
    pub fn new() -> Self {
        Self {
            instances: Vec::with_capacity(MAX_INSTANCES),
            batches: Vec::with_capacity(64),
        }
    }
    pub fn clear(&mut self) {
        self.instances.clear();
        self.batches.clear();
    }
    pub fn add_batch(&mut self, texture_key: usize, sprites: &[SpriteInstance]) {
        if sprites.is_empty() {
            return;
        }
        let start = self.instances.len() as u32;
        self.instances.extend_from_slice(sprites);
        let count = sprites.len() as u32;
        self.batches.push((texture_key, start, count));
    }
    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }
}
