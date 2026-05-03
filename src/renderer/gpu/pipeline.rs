use super::context::GpuContext;
use super::sprites::{
    sprite_instance_layout, SpriteBlendMode, SpriteInstance, Uniforms, MAX_INSTANCES,
};
use crate::utils::perf;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::mpsc::{self, TryRecvError};
use std::sync::Arc;
use std::time::Instant;
use wgpu::util::DeviceExt;
const NUM_OUTPUT_BUFFERS: usize = 3;
const MAX_CACHED_BIND_GROUPS: usize = 1024;
fn sprite_blend_state() -> wgpu::BlendState {
    wgpu::BlendState::ALPHA_BLENDING
}
fn additive_blend_state() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
    }
}
fn note_composite_blend_state() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
    }
}
fn drain_contiguous_ready_sequences(
    next_ready_sequence: &mut u64,
    ready_by_sequence: &mut BTreeMap<u64, Vec<u8>>,
    ready_frames: &mut VecDeque<Vec<u8>>,
) {
    // GPU readbacks can finish out of order; callers still expect submitted frame order.
    while let Some(frame) = ready_by_sequence.remove(next_ready_sequence) {
        ready_frames.push_back(frame);
        *next_ready_sequence = next_ready_sequence.wrapping_add(1);
    }
}
#[derive(Clone, Copy, PartialEq)]
enum BufferState {
    Available,
    Rendering,
    Reading,
}
#[derive(Clone, Copy)]
enum RenderTargetKind {
    Main,
    Notes,
}
#[derive(Clone, Copy)]
enum PassBufferKind {
    PreNotes,
    Notes,
    Overlay,
}
struct OutputBuffer {
    buffer: wgpu::Buffer,
    state: BufferState,
    sequence: Option<u64>,
    map_rx: Option<mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>>,
    cpu_buffer: Vec<u8>,
}
struct PassInstanceBuffer {
    buffer: wgpu::Buffer,
    capacity: usize,
    label: &'static str,
}
struct BindGroupCacheEntry {
    bind_group: wgpu::BindGroup,
    _texture_view: wgpu::TextureView,
    last_used_frame: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureSampling {
    Nearest,
    Linear,
}
pub type RenderBatch = (
    Arc<wgpu::Texture>,
    TextureSampling,
    SpriteBlendMode,
    Vec<SpriteInstance>,
);
pub struct SpritePipeline {
    pipeline: wgpu::RenderPipeline,
    additive_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    nearest_sampler: wgpu::Sampler,
    linear_sampler: wgpu::Sampler,
    target_texture: wgpu::Texture,
    target_view: wgpu::TextureView,
    note_texture: wgpu::Texture,
    note_view: wgpu::TextureView,
    note_bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    pre_notes_instance_buffer: PassInstanceBuffer,
    notes_instance_buffer: PassInstanceBuffer,
    overlay_instance_buffer: PassInstanceBuffer,
    composite_instance_buffer: wgpu::Buffer,
    output_buffers: Vec<OutputBuffer>,
    current_output_buffer: usize,
    padded_bytes_per_row: u32,
    next_submit_sequence: u64,
    next_ready_sequence: u64,
    pending_readbacks: usize,
    ready_by_sequence: BTreeMap<u64, Vec<u8>>,
    ready_frames: VecDeque<Vec<u8>>,
    bind_group_cache: HashMap<(usize, TextureSampling), BindGroupCacheEntry>,
    frame_id: u64,
    current_frame: Vec<u8>,
    recycled_frames: Vec<Vec<u8>>,
}
impl SpritePipeline {
    fn create_instance_buffer(
        device: &wgpu::Device,
        capacity: usize,
        label: &'static str,
    ) -> PassInstanceBuffer {
        PassInstanceBuffer {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: (capacity * std::mem::size_of::<SpriteInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity,
            label,
        }
    }
    fn create_render_pipeline(
        device: &wgpu::Device,
        pipeline_layout: &wgpu::PipelineLayout,
        shader: &wgpu::ShaderModule,
        label: &str,
        blend: wgpu::BlendState,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                buffers: &[sprite_instance_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }
    pub fn new(ctx: &GpuContext, width: u32, height: u32) -> Self {
        let device = &ctx.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sprite Shader"),
            source: wgpu::ShaderSource::Wgsl(SPRITE_SHADER.into()),
        });
        let nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Sprite Nearest Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Sprite Linear Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Sprite Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Sprite Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = Self::create_render_pipeline(
            device,
            &pipeline_layout,
            &shader,
            "Sprite Render Pipeline",
            sprite_blend_state(),
        );
        let additive_pipeline = Self::create_render_pipeline(
            device,
            &pipeline_layout,
            &shader,
            "Sprite Additive Render Pipeline",
            additive_blend_state(),
        );
        let composite_pipeline = Self::create_render_pipeline(
            device,
            &pipeline_layout,
            &shader,
            "Sprite Composite Pipeline",
            note_composite_blend_state(),
        );
        let uniforms = Uniforms::new(width as f32, height as f32);
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let (target_texture, target_view) =
            Self::create_color_texture(device, width, height, "Render Target", false);
        let (note_texture, note_view) =
            Self::create_color_texture(device, width, height, "Note Render Target", true);
        let note_bind_group = Self::create_texture_bind_group(
            device,
            &bind_group_layout,
            &uniform_buffer,
            &nearest_sampler,
            &note_view,
            "Note Composite Bind Group",
        );
        let max_instances = Self::max_instances_for_device(ctx).max(1);
        let instance_capacity = MAX_INSTANCES.min(max_instances);
        let pre_notes_instance_buffer =
            Self::create_instance_buffer(device, instance_capacity, "Pre-Notes Instance Buffer");
        let notes_instance_buffer =
            Self::create_instance_buffer(device, instance_capacity, "Notes Instance Buffer");
        let overlay_instance_buffer =
            Self::create_instance_buffer(device, instance_capacity, "Overlay Instance Buffer");
        let composite_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Composite Instance Buffer"),
            size: std::mem::size_of::<SpriteInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bytes_per_row = 4 * width;
        let padded_bytes_per_row = (bytes_per_row + 255) & !255;
        let buffer_size = (padded_bytes_per_row * height) as u64;
        let mut output_buffers = Vec::with_capacity(NUM_OUTPUT_BUFFERS);
        let frame_size = (4 * width * height) as usize;
        // Multiple output buffers let rendering continue while previous frames are mapped to CPU memory.
        for i in 0..NUM_OUTPUT_BUFFERS {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("Output Buffer {}", i)),
                size: buffer_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            output_buffers.push(OutputBuffer {
                buffer,
                state: BufferState::Available,
                sequence: None,
                map_rx: None,
                cpu_buffer: vec![0u8; frame_size],
            });
        }
        Self {
            pipeline,
            additive_pipeline,
            composite_pipeline,
            bind_group_layout,
            uniform_buffer,
            nearest_sampler,
            linear_sampler,
            target_texture,
            target_view,
            note_texture,
            note_view,
            note_bind_group,
            width,
            height,
            pre_notes_instance_buffer,
            notes_instance_buffer,
            overlay_instance_buffer,
            composite_instance_buffer,
            output_buffers,
            current_output_buffer: 0,
            padded_bytes_per_row,
            next_submit_sequence: 0,
            next_ready_sequence: 0,
            pending_readbacks: 0,
            ready_by_sequence: BTreeMap::new(),
            ready_frames: VecDeque::new(),
            bind_group_cache: HashMap::with_capacity(MAX_CACHED_BIND_GROUPS),
            frame_id: 0,
            current_frame: vec![0u8; frame_size],
            recycled_frames: Vec::with_capacity(NUM_OUTPUT_BUFFERS),
        }
    }
    fn max_instances_for_device(ctx: &GpuContext) -> usize {
        let bytes_per_instance = std::mem::size_of::<SpriteInstance>() as u64;
        let max_buffer_size = ctx.device.limits().max_buffer_size;
        (max_buffer_size / bytes_per_instance) as usize
    }
    fn instance_buffer_slot(&self, kind: PassBufferKind) -> &PassInstanceBuffer {
        match kind {
            PassBufferKind::PreNotes => &self.pre_notes_instance_buffer,
            PassBufferKind::Notes => &self.notes_instance_buffer,
            PassBufferKind::Overlay => &self.overlay_instance_buffer,
        }
    }
    fn instance_buffer_slot_mut(&mut self, kind: PassBufferKind) -> &mut PassInstanceBuffer {
        match kind {
            PassBufferKind::PreNotes => &mut self.pre_notes_instance_buffer,
            PassBufferKind::Notes => &mut self.notes_instance_buffer,
            PassBufferKind::Overlay => &mut self.overlay_instance_buffer,
        }
    }
    fn ensure_instance_capacity(
        &mut self,
        ctx: &GpuContext,
        kind: PassBufferKind,
        required: usize,
    ) {
        let current_capacity = self.instance_buffer_slot(kind).capacity;
        if required <= current_capacity {
            return;
        }
        let max_instances = Self::max_instances_for_device(ctx).max(1);
        if max_instances <= current_capacity {
            return;
        }
        let mut new_capacity = current_capacity.max(1);
        while new_capacity < required && new_capacity < max_instances {
            new_capacity = new_capacity.saturating_mul(2);
        }
        if new_capacity > max_instances {
            new_capacity = max_instances;
        }
        if new_capacity <= current_capacity {
            return;
        }
        let label = self.instance_buffer_slot(kind).label;
        *self.instance_buffer_slot_mut(kind) =
            Self::create_instance_buffer(&ctx.device, new_capacity, label);
    }
    fn create_color_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        label: &str,
        texture_binding: bool,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let mut usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC;
        if texture_binding {
            usage |= wgpu::TextureUsages::TEXTURE_BINDING;
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }
    fn create_texture_bind_group(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        uniform_buffer: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
        texture_view: &wgpu::TextureView,
        label: &str,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
    pub fn resize(&mut self, ctx: &GpuContext, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;
        let (texture, view) =
            Self::create_color_texture(&ctx.device, width, height, "Render Target", false);
        self.target_texture = texture;
        self.target_view = view;
        let (note_texture, note_view) =
            Self::create_color_texture(&ctx.device, width, height, "Note Render Target", true);
        self.note_texture = note_texture;
        self.note_view = note_view;
        self.note_bind_group = Self::create_texture_bind_group(
            &ctx.device,
            &self.bind_group_layout,
            &self.uniform_buffer,
            &self.nearest_sampler,
            &self.note_view,
            "Note Composite Bind Group",
        );
        self.write_uniforms(ctx);
        let bytes_per_row = 4 * width;
        self.padded_bytes_per_row = (bytes_per_row + 255) & !255;
        let buffer_size = (self.padded_bytes_per_row * height) as u64;
        self.output_buffers.clear();
        let frame_size = (4 * width * height) as usize;
        for i in 0..NUM_OUTPUT_BUFFERS {
            let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("Output Buffer {}", i)),
                size: buffer_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            self.output_buffers.push(OutputBuffer {
                buffer,
                state: BufferState::Available,
                sequence: None,
                map_rx: None,
                cpu_buffer: vec![0u8; frame_size],
            });
        }
        self.current_output_buffer = 0;
        self.next_submit_sequence = 0;
        self.next_ready_sequence = 0;
        self.pending_readbacks = 0;
        self.ready_by_sequence.clear();
        self.ready_frames.clear();
        self.recycled_frames.clear();
        self.current_frame.resize(frame_size, 0);
        self.current_frame.fill(0);
        self.bind_group_cache.clear();
    }
    fn get_or_create_bind_group(
        &mut self,
        device: &wgpu::Device,
        texture: &Arc<wgpu::Texture>,
        sampling: TextureSampling,
    ) -> (usize, TextureSampling) {
        let key = (Arc::as_ptr(texture) as usize, sampling);
        if let Some(entry) = self.bind_group_cache.get_mut(&key) {
            entry.last_used_frame = self.frame_id;
            return key;
        }
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = match sampling {
            TextureSampling::Nearest => &self.nearest_sampler,
            TextureSampling::Linear => &self.linear_sampler,
        };
        // Bind groups are cached by texture identity and sampler because creating them per sprite is costly.
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Cached Sprite Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        self.bind_group_cache.insert(
            key,
            BindGroupCacheEntry {
                bind_group,
                _texture_view: texture_view,
                last_used_frame: self.frame_id,
            },
        );
        key
    }
    pub(crate) fn prune_bind_group_cache(&mut self, max_entries: usize) {
        if self.bind_group_cache.len() <= max_entries {
            return;
        }
        // Prune least-recently-used texture bindings to avoid unbounded cache growth during long renders.
        let mut order: Vec<((usize, TextureSampling), u64)> = self
            .bind_group_cache
            .iter()
            .map(|(key, entry)| (*key, entry.last_used_frame))
            .collect();
        order.sort_by_key(|(_, frame)| *frame);
        let remove_count = self.bind_group_cache.len().saturating_sub(max_entries);
        for (key, _) in order.into_iter().take(remove_count) {
            self.bind_group_cache.remove(&key);
        }
    }
    pub fn clear_bind_group_cache(&mut self) {
        self.bind_group_cache.clear();
    }
    fn frame_size(&self) -> usize {
        (4 * self.width * self.height) as usize
    }
    fn acquire_frame_buffer(&mut self) -> Vec<u8> {
        if let Some(mut frame) = self.recycled_frames.pop() {
            frame.resize(self.frame_size(), 0);
            frame
        } else {
            vec![0u8; self.frame_size()]
        }
    }
    fn zero_current_frame(&mut self) -> &[u8] {
        let frame_size = self.frame_size();
        self.current_frame.resize(frame_size, 0);
        self.current_frame.fill(0);
        self.current_frame.as_slice()
    }
    fn get_next_available_buffer(&mut self, ctx: &GpuContext) -> usize {
        loop {
            self.collect_ready_frames(ctx, false);
            for _ in 0..NUM_OUTPUT_BUFFERS {
                let idx = self.current_output_buffer;
                self.current_output_buffer = (self.current_output_buffer + 1) % NUM_OUTPUT_BUFFERS;
                if self.output_buffers[idx].state == BufferState::Available {
                    return idx;
                }
            }
            // All readback buffers are busy; block until one mapping completes.
            self.collect_ready_frames(ctx, true);
        }
    }
    fn write_uniforms(&self, ctx: &GpuContext) {
        let uniforms = Uniforms::new(self.width as f32, self.height as f32);
        ctx.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }
    fn build_pass_data(
        &mut self,
        ctx: &GpuContext,
        batches: &[RenderBatch],
        pass_buffer: PassBufferKind,
    ) -> (
        Vec<SpriteInstance>,
        Vec<((usize, TextureSampling), SpriteBlendMode, u32, u32)>,
    ) {
        let mut all_instances: Vec<SpriteInstance> = Vec::new();
        let mut batch_ranges: Vec<((usize, TextureSampling), SpriteBlendMode, u32, u32)> =
            Vec::new();
        for (texture, sampling, blend_mode, instances) in batches {
            if instances.is_empty() {
                continue;
            }
            let start = all_instances.len() as u32;
            all_instances.extend_from_slice(instances);
            let count = instances.len() as u32;
            let key = self.get_or_create_bind_group(&ctx.device, texture, *sampling);
            batch_ranges.push((key, *blend_mode, start, count));
        }
        self.ensure_instance_capacity(ctx, pass_buffer, all_instances.len());
        let instance_capacity = self.instance_buffer_slot(pass_buffer).capacity;
        if all_instances.len() > instance_capacity {
            let dropped = all_instances.len() - instance_capacity;
            println!(
                "   warn: instance buffer capacity ({}) exceeded by {}, truncating draw for this frame",
                instance_capacity, dropped
            );
            // Clip batch ranges to the resized device limit instead of overflowing the vertex buffer.
            all_instances.truncate(instance_capacity);
            let max_instances = instance_capacity as u32;
            let mut clipped: Vec<((usize, TextureSampling), SpriteBlendMode, u32, u32)> =
                Vec::with_capacity(batch_ranges.len());
            for (texture_key, blend_mode, start, count) in batch_ranges {
                if start >= max_instances {
                    break;
                }
                let end = start.saturating_add(count);
                let clamped_end = end.min(max_instances);
                if clamped_end > start {
                    clipped.push((texture_key, blend_mode, start, clamped_end - start));
                }
            }
            batch_ranges = clipped;
        }
        (all_instances, batch_ranges)
    }
    fn render_batches_to_target(
        &mut self,
        ctx: &GpuContext,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderTargetKind,
        pass_buffer: PassBufferKind,
        batches: &[RenderBatch],
        load_op: wgpu::LoadOp<wgpu::Color>,
        label: &str,
    ) {
        if batches.is_empty() {
            let target_view = match target {
                RenderTargetKind::Main => &self.target_view,
                RenderTargetKind::Notes => &self.note_view,
            };
            if matches!(load_op, wgpu::LoadOp::Load) {
                return;
            }
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: load_op,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            return;
        }
        self.write_uniforms(ctx);
        let (all_instances, batch_ranges) = self.build_pass_data(ctx, batches, pass_buffer);
        if !all_instances.is_empty() {
            ctx.queue.write_buffer(
                &self.instance_buffer_slot(pass_buffer).buffer,
                0,
                bytemuck::cast_slice(&all_instances),
            );
        }
        let target_view = match target {
            RenderTargetKind::Main => &self.target_view,
            RenderTargetKind::Notes => &self.note_view,
        };
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: load_op,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render_pass.set_vertex_buffer(0, self.instance_buffer_slot(pass_buffer).buffer.slice(..));
        let mut current_blend_mode: Option<SpriteBlendMode> = None;
        for (texture_key, blend_mode, start, count) in &batch_ranges {
            if current_blend_mode != Some(*blend_mode) {
                let pipeline = match blend_mode {
                    SpriteBlendMode::Alpha => &self.pipeline,
                    SpriteBlendMode::Additive => &self.additive_pipeline,
                };
                render_pass.set_pipeline(pipeline);
                current_blend_mode = Some(*blend_mode);
            }
            if let Some(entry) = self.bind_group_cache.get(texture_key) {
                render_pass.set_bind_group(0, &entry.bind_group, &[]);
                render_pass.draw(0..6, *start..(*start + *count));
            }
        }
    }
    fn composite_note_texture(&mut self, ctx: &GpuContext, encoder: &mut wgpu::CommandEncoder) {
        self.write_uniforms(ctx);
        // Notes render into a transparent target first so their premultiplied composite is isolated.
        let composite_instance = [SpriteInstance::new(
            0.0,
            0.0,
            self.width as f32,
            self.height as f32,
        )];
        ctx.queue.write_buffer(
            &self.composite_instance_buffer,
            0,
            bytemuck::cast_slice(&composite_instance),
        );
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Scene Notes Composite Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render_pass.set_pipeline(&self.composite_pipeline);
        render_pass.set_vertex_buffer(0, self.composite_instance_buffer.slice(..));
        render_pass.set_bind_group(0, &self.note_bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
    fn encode_copy_to_output(&self, output_idx: usize, encoder: &mut wgpu::CommandEncoder) {
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.target_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.output_buffers[output_idx].buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }
    fn begin_output_readback(&mut self, output_idx: usize) {
        let buffer_slice = self.output_buffers[output_idx].buffer.slice(..);
        let (tx, rx) = mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.output_buffers[output_idx].state = BufferState::Reading;
        self.output_buffers[output_idx].map_rx = Some(rx);
        self.pending_readbacks = self.pending_readbacks.saturating_add(1);
    }
    fn finalize_ready_frame_order(&mut self) {
        drain_contiguous_ready_sequences(
            &mut self.next_ready_sequence,
            &mut self.ready_by_sequence,
            &mut self.ready_frames,
        );
    }
    fn finish_output_readback(&mut self, buffer_idx: usize, mapped: bool) {
        let replacement = self.acquire_frame_buffer();
        let mut frame =
            std::mem::replace(&mut self.output_buffers[buffer_idx].cpu_buffer, replacement);
        frame.resize(self.frame_size(), 0);
        if mapped {
            let buffer_slice = self.output_buffers[buffer_idx].buffer.slice(..);
            let data = buffer_slice.get_mapped_range();
            let row_bytes = (4 * self.width) as usize;
            let has_padding = self.padded_bytes_per_row != (4 * self.width);
            if has_padding {
                // wgpu readback rows are padded; ffmpeg expects tightly packed RGBA rows.
                for row in 0..self.height as usize {
                    let src_start = row * self.padded_bytes_per_row as usize;
                    let src_end = src_start + row_bytes;
                    let dst_start = row * row_bytes;
                    let dst_end = dst_start + row_bytes;
                    frame[dst_start..dst_end].copy_from_slice(&data[src_start..src_end]);
                }
            } else {
                let frame_len = frame.len();
                frame.copy_from_slice(&data[..frame_len]);
            }
            drop(data);
            self.output_buffers[buffer_idx].buffer.unmap();
        } else {
            frame.fill(0);
        }
        let sequence = self.output_buffers[buffer_idx]
            .sequence
            .take()
            .unwrap_or(self.next_ready_sequence);
        self.output_buffers[buffer_idx].map_rx.take();
        self.output_buffers[buffer_idx].state = BufferState::Available;
        self.pending_readbacks = self.pending_readbacks.saturating_sub(1);
        self.ready_by_sequence.insert(sequence, frame);
        self.finalize_ready_frame_order();
    }
    fn collect_ready_frames(&mut self, ctx: &GpuContext, blocking: bool) {
        if blocking {
            let wait_start = if perf::enabled() {
                Some(Instant::now())
            } else {
                None
            };
            let _ = ctx.device.poll(wgpu::PollType::Wait);
            if let Some(start) = wait_start {
                perf::record("gpu_wait", start.elapsed());
            }
        } else {
            let _ = ctx.device.poll(wgpu::PollType::Poll);
        }
        let mut completed = Vec::new();
        for (idx, output) in self.output_buffers.iter().enumerate() {
            if output.state != BufferState::Reading {
                continue;
            }
            let event = match output.map_rx.as_ref() {
                Some(rx) => match rx.try_recv() {
                    Ok(result) => Some(result.is_ok()),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => Some(false),
                },
                None => Some(false),
            };
            if let Some(mapped) = event {
                completed.push((idx, mapped));
            }
        }
        for (idx, mapped) in completed {
            self.finish_output_readback(idx, mapped);
        }
    }
    fn take_next_ready_frame(&mut self) -> Option<&[u8]> {
        let next = self.ready_frames.pop_front()?;
        let previous = std::mem::replace(&mut self.current_frame, next);
        // Recycle CPU frame allocations because every rendered frame has the same byte length.
        self.recycled_frames.push(previous);
        Some(self.current_frame.as_slice())
    }
    pub fn submit_layered(
        &mut self,
        ctx: &GpuContext,
        before_batches: &[RenderBatch],
        note_batches: &[RenderBatch],
        after_batches: &[RenderBatch],
        clear_color: [f32; 4],
    ) {
        self.frame_id = self.frame_id.wrapping_add(1);
        let output_idx = self.get_next_available_buffer(ctx);
        self.output_buffers[output_idx].state = BufferState::Rendering;
        self.output_buffers[output_idx].sequence = Some(self.next_submit_sequence);
        // Sequence numbers preserve submit order across asynchronous map_async completions.
        self.next_submit_sequence = self.next_submit_sequence.wrapping_add(1);
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame Encoder"),
            });
        self.render_batches_to_target(
            ctx,
            &mut encoder,
            RenderTargetKind::Main,
            PassBufferKind::PreNotes,
            before_batches,
            wgpu::LoadOp::Clear(wgpu::Color {
                r: clear_color[0] as f64,
                g: clear_color[1] as f64,
                b: clear_color[2] as f64,
                a: clear_color[3] as f64,
            }),
            "Scene Pre-Notes Pass",
        );
        self.render_batches_to_target(
            ctx,
            &mut encoder,
            RenderTargetKind::Notes,
            PassBufferKind::Notes,
            note_batches,
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            "Scene Notes Pass",
        );
        if !note_batches.is_empty() {
            self.composite_note_texture(ctx, &mut encoder);
        }
        self.render_batches_to_target(
            ctx,
            &mut encoder,
            RenderTargetKind::Main,
            PassBufferKind::Overlay,
            after_batches,
            wgpu::LoadOp::Load,
            "Scene Overlay Pass",
        );
        self.encode_copy_to_output(output_idx, &mut encoder);
        ctx.queue.submit(std::iter::once(encoder.finish()));
        self.begin_output_readback(output_idx);
    }
    pub fn poll_ready_frame(&mut self, ctx: &GpuContext) -> Option<&[u8]> {
        self.collect_ready_frames(ctx, false);
        self.take_next_ready_frame()
    }
    pub fn drain_ready_frame_blocking(&mut self, ctx: &GpuContext) -> Option<&[u8]> {
        if !self.ready_frames.is_empty() {
            return self.take_next_ready_frame();
        }
        while self.pending_readbacks > 0 {
            self.collect_ready_frames(ctx, true);
            if !self.ready_frames.is_empty() {
                return self.take_next_ready_frame();
            }
        }
        None
    }
    pub fn draw_layered(
        &mut self,
        ctx: &GpuContext,
        before_batches: &[RenderBatch],
        note_batches: &[RenderBatch],
        after_batches: &[RenderBatch],
        clear_color: [f32; 4],
    ) -> &[u8] {
        self.submit_layered(
            ctx,
            before_batches,
            note_batches,
            after_batches,
            clear_color,
        );
        if self.ready_frames.is_empty() && self.pending_readbacks == 0 {
            return self.zero_current_frame();
        }
        self.drain_ready_frame_blocking(ctx)
            .expect("frame should be available after submit")
    }
    pub fn submit_batched(
        &mut self,
        ctx: &GpuContext,
        batches: &[RenderBatch],
        clear_color: [f32; 4],
    ) {
        self.submit_layered(ctx, batches, &[], &[], clear_color)
    }
    pub fn draw_batched(
        &mut self,
        ctx: &GpuContext,
        batches: &[RenderBatch],
        clear_color: [f32; 4],
    ) -> &[u8] {
        self.submit_batched(ctx, batches, clear_color);
        if self.ready_frames.is_empty() && self.pending_readbacks == 0 {
            return self.zero_current_frame();
        }
        self.drain_ready_frame_blocking(ctx)
            .expect("frame should be available after submit")
    }
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    pub fn cache_stats(&self) -> (usize, usize) {
        let len = self.bind_group_cache.len();
        (len, len)
    }
}
const SPRITE_SHADER: &str = r#"
// Uniforms
struct Uniforms {
    screen_size: vec2<f32>,
    _padding: vec2<f32>,
}
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var sprite_texture: texture_2d<f32>;
@group(0) @binding(2) var sprite_sampler: sampler;
// Instance input
struct InstanceInput {
    @location(0) position: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) uv_rect: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) origin: vec2<f32>,
    @location(5) rotation: f32,
}
// Vertex output
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}
// Quad vertices (2 triangles)
var<private> QUAD_VERTICES: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0), // top-left
    vec2<f32>(1.0, 0.0), // top-right
    vec2<f32>(0.0, 1.0), // bottom-left
    vec2<f32>(1.0, 0.0), // top-right
    vec2<f32>(1.0, 1.0), // bottom-right
    vec2<f32>(0.0, 1.0), // bottom-left
);
@vertex
fn vs_main(
    @builtin(vertex_index) vertex_idx: u32,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    let quad_pos = QUAD_VERTICES[vertex_idx];
    // rotate around origin
    let local_pos = quad_pos * instance.size - instance.origin;
    let s = sin(instance.rotation);
    let c = cos(instance.rotation);
    let rotated = vec2<f32>(
        local_pos.x * c - local_pos.y * s,
        local_pos.x * s + local_pos.y * c
    );
    // pixel position (top-left + origin + rotated)
    let pixel_pos = instance.position + instance.origin + rotated;
    // convert to clip space (-1 to 1)
    let clip_x = (pixel_pos.x / uniforms.screen_size.x) * 2.0 - 1.0;
    let clip_y = 1.0 - (pixel_pos.y / uniforms.screen_size.y) * 2.0;
    out.clip_position = vec4<f32>(clip_x, clip_y, 0.0, 1.0);
    // uv interpolation
    out.uv = mix(instance.uv_rect.xy, instance.uv_rect.zw, quad_pos);
    out.color = instance.color;
    return out;
}
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(sprite_texture, sprite_sampler, in.uv);
    return tex_color * in.color;
}
"#;
