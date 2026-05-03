#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuPreference {
    High,
    Low,
    Auto,
}
impl std::str::FromStr for GpuPreference {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "high" => Ok(Self::High),
            "low" => Ok(Self::Low),
            "auto" => Ok(Self::Auto),
            _ => Err(format!("unknown gpu preference: {s}")),
        }
    }
}
pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter: wgpu::Adapter,
}
impl GpuContext {
    pub async fn new(
        preference: GpuPreference,
        adapter_hint: Option<&str>,
    ) -> Result<Self, GpuError> {
        let instance = wgpu::Instance::default();
        let hint = adapter_hint.map(|h| h.to_ascii_lowercase());
        let mut adapters: Vec<wgpu::Adapter> = instance
            .enumerate_adapters(wgpu::Backends::all())
            .into_iter()
            .collect();
        if let Some(ref h) = hint {
            adapters.retain(|a: &wgpu::Adapter| a.get_info().name.to_ascii_lowercase().contains(h));
        }
        let mut ordered: Vec<wgpu::Adapter> = Vec::new();
        let prefer_integrated = matches!(preference, GpuPreference::Low | GpuPreference::Auto);
        let prefer_discrete = matches!(preference, GpuPreference::High);
        // Reorder enumerated adapters by preference without discarding fallback device types.
        let push_by_type =
            |list: &mut Vec<wgpu::Adapter>, ty: wgpu::DeviceType, src: &mut Vec<wgpu::Adapter>| {
                let mut i = 0;
                while i < src.len() {
                    if src[i].get_info().device_type == ty {
                        list.push(src.remove(i));
                    } else {
                        i += 1;
                    }
                }
            };
        let mut remaining = adapters;
        if prefer_integrated {
            push_by_type(
                &mut ordered,
                wgpu::DeviceType::IntegratedGpu,
                &mut remaining,
            );
            push_by_type(&mut ordered, wgpu::DeviceType::DiscreteGpu, &mut remaining);
        } else if prefer_discrete {
            push_by_type(&mut ordered, wgpu::DeviceType::DiscreteGpu, &mut remaining);
            push_by_type(
                &mut ordered,
                wgpu::DeviceType::IntegratedGpu,
                &mut remaining,
            );
        }
        ordered.extend(remaining);
        let adapter = if let Some(adapter) = ordered.into_iter().next() {
            adapter
        } else {
            if hint.is_some() {
                println!("   warn: gpu adapter hint not found, using default selection");
            }
            instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                })
                .await
                .map_err(|_| GpuError::NoAdapter)?
        };
        let (device, queue): (wgpu::Device, wgpu::Queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|e: wgpu::RequestDeviceError| GpuError::DeviceRequest(e.to_string()))?;
        let info = adapter.get_info();
        println!(
            "   [gpu] adapter: {} ({:?}, {:?})",
            info.name, info.device_type, info.backend
        );
        Ok(Self {
            device,
            queue,
            adapter,
        })
    }
    pub fn adapter_info(&self) -> wgpu::AdapterInfo {
        self.adapter.get_info()
    }
    pub fn check_features(&self) -> bool {
        true
    }
}
#[derive(Debug)]
pub enum GpuError {
    NoAdapter,
    DeviceRequest(String),
}
impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuError::NoAdapter => write!(f, "No suitable GPU adapter found"),
            GpuError::DeviceRequest(e) => write!(f, "Failed to create device: {}", e),
        }
    }
}
impl std::error::Error for GpuError {}
pub struct OffscreenTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}
impl OffscreenTexture {
    pub fn new(ctx: &GpuContext, width: u32, height: u32) -> Self {
        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Offscreen Render Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            width,
            height,
        }
    }
    pub fn bytes_per_row(&self) -> u32 {
        let unpadded = self.width * 4;
        let align = 256;
        // COPY_BYTES_PER_ROW_ALIGNMENT requires readback rows to be 256-byte aligned.
        unpadded.div_ceil(align) * align
    }
}
pub struct ReadbackBuffer {
    pub buffer: wgpu::Buffer,
    pub width: u32,
    pub height: u32,
    pub bytes_per_row: u32,
}
impl ReadbackBuffer {
    pub fn new(ctx: &GpuContext, width: u32, height: u32) -> Self {
        let bytes_per_row = {
            let unpadded = width * 4;
            let align = 256;
            // Readback buffers use padded rows even though the final RGBA frame is tightly packed.
            unpadded.div_ceil(align) * align
        };
        let size = (bytes_per_row * height) as u64;
        let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Readback Buffer"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            buffer,
            width,
            height,
            bytes_per_row,
        }
    }
}
