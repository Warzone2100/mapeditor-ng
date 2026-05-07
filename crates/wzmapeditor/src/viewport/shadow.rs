//! Shadow mapping GPU resources, bind group creation, and light-space MVP computation.

use glam::{Mat4, Vec3};

/// Single-cascade shadow map covering the whole map. 2048 balances
/// quality and VRAM for typical map sizes (64x64 to 250x250 tiles).
pub const SHADOW_MAP_SIZE: u32 = 2048;

/// Shadow mapping GPU resources and bind groups.
#[derive(Debug)]
pub struct ShadowResources {
    /// Owns the GPU allocation; sampled via `depth_view`.
    #[expect(dead_code, reason = "must be kept alive to back the depth_view")]
    depth_texture: wgpu::Texture,
    /// View into the shadow depth texture for sampling.
    pub depth_view: wgpu::TextureView,
    /// Bind group exposing the shadow map to terrain/model shaders (group 2).
    pub bind_group: wgpu::BindGroup,
    /// Uniform buffer holding the light-space MVP for the shadow vertex shader.
    pub uniform_buffer: wgpu::Buffer,
    /// Bind group for the shadow pass vertex shader (group 0).
    pub uniform_bind_group: wgpu::BindGroup,
}

/// Light-space MVP passed to the shadow depth pass vertex shader.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ShadowUniforms {
    pub light_mvp: [[f32; 4]; 4],
}

/// Create shadow depth texture and associated bind groups.
pub fn create_shadow_resources(
    device: &wgpu::Device,
    shadow_bind_group_layout: &wgpu::BindGroupLayout,
    shadow_uniform_layout: &wgpu::BindGroupLayout,
) -> ShadowResources {
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shadow_depth_texture"),
        size: wgpu::Extent3d {
            width: SHADOW_MAP_SIZE,
            height: SHADOW_MAP_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

    let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("shadow_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        compare: Some(wgpu::CompareFunction::LessEqual),
        ..Default::default()
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("shadow_bind_group"),
        layout: shadow_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&depth_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&shadow_sampler),
            },
        ],
    });

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("shadow_uniform_buffer"),
        size: size_of::<ShadowUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("shadow_uniform_bind_group"),
        layout: shadow_uniform_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });

    ShadowResources {
        depth_texture,
        depth_view,
        bind_group,
        uniform_buffer,
        uniform_bind_group,
    }
}

/// Light-space MVP for shadow mapping. Orthographic from the sun,
/// covering the whole map; the up vector avoids degeneracy near vertical.
pub fn compute_shadow_mvp(sun_direction: [f32; 3], map_dims: (u32, u32)) -> Mat4 {
    let sun_dir = Vec3::new(sun_direction[0], sun_direction[1], sun_direction[2]).normalize();

    let tile_size = 128.0_f32;
    let map_w = map_dims.0 as f32 * tile_size;
    let map_h = map_dims.1 as f32 * tile_size;
    // Mid-range height keeps the shadow frustum over elevated terrain.
    let center = Vec3::new(map_w * 0.5, 200.0, map_h * 0.5);
    let extent = map_w.max(map_h);

    let light_pos = center + sun_dir * extent;

    // Use Z as up when the sun is near-vertical to avoid a degenerate look_at.
    let up = if sun_dir.y.abs() > 0.9 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let light_view = Mat4::look_at_rh(light_pos, center, up);

    let half = extent * 0.7;
    let light_proj = Mat4::orthographic_rh(-half, half, -half, half, 1.0, extent * 3.0);

    light_proj * light_view
}

/// Caches the shadow MVP across frames since sun direction and map
/// dimensions rarely change.
pub struct CachedShadowMvp {
    mvp: Mat4,
    sun_direction: [f32; 3],
    map_dims: (u32, u32),
}

impl CachedShadowMvp {
    /// Create with initial values.
    pub fn new() -> Self {
        Self {
            mvp: Mat4::IDENTITY,
            sun_direction: [0.0; 3],
            map_dims: (0, 0),
        }
    }

    /// Return the cached MVP, recomputing only on input change. Uses
    /// bitwise equality on f32 arrays so any change to the exact float
    /// representation invalidates the cache.
    #[expect(
        clippy::float_cmp,
        reason = "bitwise equality is intentional for cache invalidation"
    )]
    pub fn get(&mut self, sun_direction: [f32; 3], map_dims: (u32, u32)) -> Mat4 {
        if self.sun_direction != sun_direction || self.map_dims != map_dims {
            self.mvp = compute_shadow_mvp(sun_direction, map_dims);
            self.sun_direction = sun_direction;
            self.map_dims = map_dims;
        }
        self.mvp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_shadow_mvp_reuses_when_unchanged() {
        let mut cache = CachedShadowMvp::new();
        let sun = [0.286, 0.763, 0.572];
        let dims = (64, 64);

        let mvp1 = cache.get(sun, dims);
        let mvp2 = cache.get(sun, dims);
        assert_eq!(mvp1, mvp2);
    }

    #[test]
    fn cached_shadow_mvp_recomputes_on_sun_change() {
        let mut cache = CachedShadowMvp::new();
        let dims = (64, 64);

        let mvp1 = cache.get([0.286, 0.763, 0.572], dims);
        let mvp2 = cache.get([0.5, 0.5, 0.5], dims);
        assert_ne!(mvp1, mvp2);
    }

    #[test]
    fn compute_shadow_mvp_produces_valid_matrix() {
        let mvp = compute_shadow_mvp([0.286, 0.763, 0.572], (64, 64));
        // Should not be identity or zero.
        assert_ne!(mvp, Mat4::IDENTITY);
        assert_ne!(mvp, Mat4::ZERO);
    }
}
