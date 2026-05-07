//! GPU model cache, texture page management, and draw call preparation.

use std::collections::HashMap;
use std::sync::Arc;

use eframe::wgpu::util::DeviceExt;
use rustc_hash::FxHashMap;

use super::pie_mesh::{ModelInstance, ModelMesh};

/// GPU resources for a single 3D model (PIE).
pub struct GpuModel {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub texture_bind_group: wgpu::BindGroup,
    pub aabb_min: [f32; 3],
    pub aabb_max: [f32; 3],
}

/// 4-layer texture array packing diffuse / tcmask / normal / specular for a
/// model. Models that resolve to the same four page names share one array
/// via `PageAtlasKey`, preserving WZ2100's texture-page sharing that keeps
/// model VRAM down.
pub(crate) struct CachedPageAtlas {
    pub view: wgpu::TextureView,
}

/// Cache key for the 4-layer model atlas. `None` slots mean no map for
/// that channel and get a default filler (alpha=0 marker).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PageAtlasKey {
    pub diffuse: Option<String>,
    pub tcmask: Option<String>,
    pub normal: Option<String>,
    pub specular: Option<String>,
}

/// Decoded texture page for GPU upload. The page name doubles as the
/// cache key so models referencing the same page share one upload.
#[derive(Debug)]
pub struct TexturePageRef<'a> {
    pub page_name: &'a str,
    pub rgba: &'a [u8],
    pub width: u32,
    pub height: u32,
}

/// A batched draw call for all instances of one model.
pub struct ModelDrawCall {
    pub model_key: Arc<str>,
    pub instance_count: u32,
}

/// Reusable GPU instance buffer with tracked capacity.
pub(crate) struct InstanceBuffer {
    buffer: wgpu::Buffer,
    /// Capacity in number of instances (not bytes).
    capacity: u32,
}

/// 3D model rendering resources: pipeline, atlas cache, default fallbacks.
pub struct ModelResources {
    pub pipeline: wgpu::RenderPipeline,
    pub texture_layout: wgpu::BindGroupLayout,
    pub cache: HashMap<String, GpuModel>,
    pub draw_calls: Vec<ModelDrawCall>,
    /// Shared 1x1x4 default atlas view used when a model has no textures.
    pub default_atlas_view: wgpu::TextureView,
    /// Most stock PIE models resolve tcmask / normal / specular by
    /// appending suffixes to the diffuse page name, so the same tuple is
    /// reused across many models.
    pub(crate) page_atlas_cache: HashMap<PageAtlasKey, CachedPageAtlas>,
    /// Mirrors the main model pipeline but targets the thumbnail format.
    pub thumb_pipeline: wgpu::RenderPipeline,
    /// Reusable instance buffers; new data is written via
    /// `queue.write_buffer` whenever the existing capacity fits.
    pub(crate) instance_buffers: HashMap<String, InstanceBuffer>,
}

impl ModelResources {
    /// Upload a model with shared atlas caching.
    ///
    /// Models with the same (diffuse, tcmask, normal, specular) tuple
    /// share one 4-layer atlas to avoid re-uploading texture data.
    #[expect(
        clippy::needless_pass_by_value,
        clippy::too_many_arguments,
        reason = "owned PieModel + many GPU handles; entry point of the upload pipeline"
    )]
    pub fn upload_model(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        key: &str,
        mesh: &ModelMesh,
        diffuse: Option<TexturePageRef<'_>>,
        tcmask: Option<TexturePageRef<'_>>,
        normal: Option<TexturePageRef<'_>>,
        specular: Option<TexturePageRef<'_>>,
    ) {
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            return;
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("model_vb_{key}")),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("model_ib_{key}")),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let atlas_key = PageAtlasKey {
            diffuse: diffuse.as_ref().map(|p| p.page_name.to_string()),
            tcmask: tcmask.as_ref().map(|p| p.page_name.to_string()),
            normal: normal.as_ref().map(|p| p.page_name.to_string()),
            specular: specular.as_ref().map(|p| p.page_name.to_string()),
        };
        self.ensure_page_atlas(
            device,
            queue,
            &atlas_key,
            diffuse.as_ref(),
            tcmask.as_ref(),
            normal.as_ref(),
            specular.as_ref(),
        );

        let atlas_view = self
            .page_atlas_cache
            .get(&atlas_key)
            .map_or(&self.default_atlas_view, |entry| &entry.view);

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("model_bg_{key}")),
            layout: &self.texture_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(atlas_view),
            }],
        });

        self.cache.insert(
            key.to_string(),
            GpuModel {
                vertex_buffer,
                index_buffer,
                index_count: mesh.indices.len() as u32,
                texture_bind_group,
                aabb_min: mesh.aabb_min.to_array(),
                aabb_max: mesh.aabb_max.to_array(),
            },
        );
    }

    /// All four maps must share one `Rgba8Unorm` `Texture2DArray`, so any
    /// size mismatch is reconciled here by nearest-neighbour resizing the
    /// smaller maps up to the largest (w, h). Stock WZ2100 assets already
    /// match, so the resize is usually a no-op.
    fn ensure_page_atlas(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        key: &PageAtlasKey,
        diffuse: Option<&TexturePageRef<'_>>,
        tcmask: Option<&TexturePageRef<'_>>,
        normal: Option<&TexturePageRef<'_>>,
        specular: Option<&TexturePageRef<'_>>,
    ) {
        // Shader's has_normalmap / has_specularmap flags are encoded in
        // alpha: 255 for real maps, 0 for default fillers. Diffuse and
        // tcmask layers preserve their original alpha.
        const DIFFUSE_DEFAULT: [u8; 4] = [255, 255, 255, 255];
        const TCMASK_DEFAULT: [u8; 4] = [0, 0, 0, 0];
        const NORMAL_DEFAULT: [u8; 4] = [128, 128, 255, 0];
        const SPECULAR_DEFAULT: [u8; 4] = [0, 0, 0, 0];

        if self.page_atlas_cache.contains_key(key) {
            return;
        }

        let mut max_w = 1u32;
        let mut max_h = 1u32;
        for page in [diffuse, tcmask, normal, specular].into_iter().flatten() {
            max_w = max_w.max(page.width);
            max_h = max_h.max(page.height);
        }

        let layer_pixels = (max_w as usize) * (max_h as usize) * 4;
        let mut atlas = vec![0u8; layer_pixels * 4];
        let label_for = |slot: usize| match slot {
            0 => "diffuse",
            1 => "tcmask",
            2 => "normal",
            3 => "specular",
            _ => "?",
        };
        for (slot, page, default) in [
            (0usize, diffuse, DIFFUSE_DEFAULT),
            (1, tcmask, TCMASK_DEFAULT),
            (2, normal, NORMAL_DEFAULT),
            (3, specular, SPECULAR_DEFAULT),
        ] {
            let dst = &mut atlas[slot * layer_pixels..(slot + 1) * layer_pixels];
            match page {
                Some(p) => {
                    let force_alpha = match slot {
                        2 | 3 => Some(255u8),
                        _ => None,
                    };
                    if p.width == max_w && p.height == max_h {
                        copy_layer(dst, p.rgba, force_alpha);
                    } else {
                        log::debug!(
                            "model atlas layer {} ({}) resized from {}x{} to {}x{}",
                            slot,
                            label_for(slot),
                            p.width,
                            p.height,
                            max_w,
                            max_h,
                        );
                        nearest_resize(p.rgba, p.width, p.height, dst, max_w, max_h, force_alpha);
                    }
                }
                None => fill_layer(dst, default),
            }
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("model_page_atlas"),
            size: wgpu::Extent3d {
                width: max_w,
                height: max_h,
                depth_or_array_layers: 4,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * max_w),
                rows_per_image: Some(max_h),
            },
            wgpu::Extent3d {
                width: max_w,
                height: max_h,
                depth_or_array_layers: 4,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        self.page_atlas_cache
            .insert(key.clone(), CachedPageAtlas { view });
    }

    /// Prepare draw calls for the current frame's object instances.
    ///
    /// Reuses cached instance buffers via `queue.write_buffer` when capacity
    /// fits, allocating only when the instance count grows.
    pub fn prepare_draw_calls(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances_by_model: &FxHashMap<Arc<str>, Vec<ModelInstance>>,
    ) {
        self.draw_calls.clear();

        for (model_key, instances) in instances_by_model {
            let key_str: &str = model_key.as_ref();
            if instances.is_empty() || !self.cache.contains_key(key_str) {
                continue;
            }

            let count = instances.len() as u32;
            let data = bytemuck::cast_slice(instances);

            let needs_new_buffer = self
                .instance_buffers
                .get(key_str)
                .is_none_or(|ib| ib.capacity < count);

            if needs_new_buffer {
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("instance_buf"),
                    contents: data,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });
                self.instance_buffers.insert(
                    key_str.to_string(),
                    InstanceBuffer {
                        buffer,
                        capacity: count,
                    },
                );
            } else {
                queue.write_buffer(&self.instance_buffers[key_str].buffer, 0, data);
            }

            self.draw_calls.push(ModelDrawCall {
                model_key: Arc::clone(model_key),
                instance_count: count,
            });
        }
    }

    /// Instance buffer for a model key, or `None` if no instances have
    /// been prepared this frame.
    pub fn instance_buffer(&self, model_key: &str) -> Option<&wgpu::Buffer> {
        self.instance_buffers.get(model_key).map(|ib| &ib.buffer)
    }
}

/// Copy an RGBA layer, optionally stamping a fixed alpha (used for the
/// normal / specular present-marker).
fn copy_layer(dst: &mut [u8], src: &[u8], force_alpha: Option<u8>) {
    debug_assert_eq!(dst.len(), src.len());
    if let Some(alpha) = force_alpha {
        for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
            d[0] = s[0];
            d[1] = s[1];
            d[2] = s[2];
            d[3] = alpha;
        }
    } else {
        dst.copy_from_slice(src);
    }
}

fn fill_layer(dst: &mut [u8], pattern: [u8; 4]) {
    for chunk in dst.chunks_exact_mut(4) {
        chunk.copy_from_slice(&pattern);
    }
}

/// Nearest-neighbour resize. Same-size siblings short-circuit to
/// `copy_layer`; this only fires for rare third-party assets that ship a
/// tcmask / normal / specular at a different size to the diffuse.
fn nearest_resize(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
    force_alpha: Option<u8>,
) {
    let dst_w_us = dst_w as usize;
    for y in 0..dst_h as usize {
        let sy = (y * src_h as usize) / dst_h as usize;
        for x in 0..dst_w_us {
            let sx = (x * src_w as usize) / dst_w as usize;
            let s = (sy * src_w as usize + sx) * 4;
            let d = (y * dst_w_us + x) * 4;
            dst[d] = src[s];
            dst[d + 1] = src[s + 1];
            dst[d + 2] = src[s + 2];
            dst[d + 3] = force_alpha.unwrap_or(src[s + 3]);
        }
    }
}
