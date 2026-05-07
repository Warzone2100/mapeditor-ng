//! Offscreen thumbnail rendering resources and GPU readback.

use super::uniforms::Uniforms;

/// 256 keeps grid thumbnails crisp after egui upscales for high-DPI displays.
pub const THUMB_SIZE: u32 = 256;

/// Designer live-preview size. 512 stays crisp under DPI scaling without
/// blowing up preload memory; the asset browser still uses [`THUMB_SIZE`].
pub const PREVIEW_THUMB_SIZE: u32 = 512;

/// `egui_wgpu::Renderer::register_native_texture` requires `Rgba8UnormSrgb`,
/// so the designer can sample the preview target directly without a
/// GPU-to-CPU-to-GPU round-trip.
pub const THUMB_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// A model to include in a GPU thumbnail render.
pub struct ThumbnailEntry<'a> {
    /// Key in the model cache (`imd_name`).
    pub model_key: &'a str,
    /// Local-space offset (e.g. connector position for weapons).
    pub offset: glam::Vec3,
    /// Team color (RGBA, alpha controls tint strength).
    pub team_color: [f32; 4],
}

/// Offscreen resources for GPU-based model thumbnail rendering.
pub(crate) struct ThumbnailResources {
    pub color_texture: wgpu::Texture,
    pub color_view: wgpu::TextureView,
    /// Owns the GPU allocation; sampled via `depth_view`.
    #[expect(dead_code, reason = "must be kept alive to back the depth_view")]
    depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
    pub staging_buffer: wgpu::Buffer,
    pub uniform_buffer: wgpu::Buffer,
    pub uniform_bind_group: wgpu::BindGroup,
}

/// `sampleable` adds `TEXTURE_BINDING` for targets that egui samples
/// directly (e.g. the designer preview); the asset-browser path reads
/// back via `COPY_SRC`.
pub(crate) fn create_thumbnail_resources(
    device: &wgpu::Device,
    uniform_layout: &wgpu::BindGroupLayout,
    lightmap_view: &wgpu::TextureView,
    lightmap_sampler: &wgpu::Sampler,
    model_sampler: &wgpu::Sampler,
    size: u32,
    sampleable: bool,
    create_uniform_bind_group: impl FnOnce(
        &wgpu::Device,
        &wgpu::BindGroupLayout,
        &wgpu::Buffer,
        &wgpu::TextureView,
        &wgpu::Sampler,
        &wgpu::Sampler,
    ) -> wgpu::BindGroup,
) -> ThumbnailResources {
    let mut usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC;
    if sampleable {
        usage |= wgpu::TextureUsages::TEXTURE_BINDING;
    }
    let color_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("thumb_color"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: THUMB_FORMAT,
        usage,
        view_formats: &[],
    });
    let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());

    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("thumb_depth"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

    // wgpu's COPY_BYTES_PER_ROW_ALIGNMENT is 256. THUMB_SIZE=256 (the
    // minimum here) gives 1024 bytes/row, already aligned, so no padding.
    let bytes_per_row = size * 4;
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("thumb_staging"),
        size: (bytes_per_row * size) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("thumb_uniform"),
        size: size_of::<Uniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let uniform_bind_group = create_uniform_bind_group(
        device,
        uniform_layout,
        &uniform_buffer,
        lightmap_view,
        lightmap_sampler,
        model_sampler,
    );

    ThumbnailResources {
        color_texture,
        color_view,
        depth_texture,
        depth_view,
        staging_buffer,
        uniform_buffer,
        uniform_bind_group,
    }
}

/// Submit `encoder` after appending a staging-buffer copy, block on the
/// readback, and return the decoded pixels. Asset-browser path only.
pub(crate) fn submit_and_read_back(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mut encoder: wgpu::CommandEncoder,
    target: &ThumbnailResources,
    size: u32,
) -> Option<egui::ColorImage> {
    let bytes_per_row = size * 4;
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target.color_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &target.staging_buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(size),
            },
        },
        wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(std::iter::once(encoder.finish()));

    let buffer_slice = target.staging_buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    let _ = device.poll(wgpu::PollType::wait_indefinitely());

    rx.recv().ok()?.ok()?;

    let data = buffer_slice.get_mapped_range();
    let size_usize = size as usize;
    let row_stride = bytes_per_row as usize;
    let mut pixels = Vec::with_capacity(size_usize * size_usize);
    for row in data.chunks_exact(row_stride).take(size_usize) {
        for pixel in row[..size_usize * 4].chunks_exact(4) {
            pixels.push(egui::Color32::from_rgb(pixel[0], pixel[1], pixel[2]));
        }
    }
    drop(data);
    target.staging_buffer.unmap();

    Some(egui::ColorImage::new([size_usize, size_usize], pixels))
}
