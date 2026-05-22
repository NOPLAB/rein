//! Texture abstractions
//!
//! Provides convenient wrappers for 2D textures, depth textures, cube maps, and texture arrays.

use crate::context::WgpuContext;

/// A 2D texture with associated view and sampler.
pub struct Texture2D {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) size: wgpu::Extent3d,
    pub(crate) format: wgpu::TextureFormat,
}

impl Texture2D {
    /// Create a new empty texture.
    pub fn new(
        ctx: &WgpuContext,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        label: Option<&str>,
    ) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: label.map(|l| format!("{l} sampler")).as_deref(),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
            size,
            format,
        }
    }

    /// Create a texture from RGBA8 image data.
    pub fn from_rgba8(
        ctx: &WgpuContext,
        width: u32,
        height: u32,
        data: &[u8],
        label: Option<&str>,
    ) -> Self {
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;

        let texture = Self::new(ctx, width, height, format, usage, label);

        ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            texture.size,
        );

        texture
    }

    /// Get the texture view.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Get the sampler.
    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    /// Get the texture size.
    pub fn size(&self) -> (u32, u32) {
        (self.size.width, self.size.height)
    }

    /// Get the texture format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }
}

/// A depth texture for depth testing.
pub struct DepthTexture {
    #[expect(dead_code, reason = "GPU resource kept alive to back the view; not read directly")]
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) size: wgpu::Extent3d,
}

impl DepthTexture {
    /// The depth format used by this texture.
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    /// Create a new depth texture.
    pub fn new(ctx: &WgpuContext, width: u32, height: u32, label: Option<&str>) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture,
            view,
            size,
        }
    }

    /// Resize the depth texture.
    pub fn resize(&mut self, ctx: &WgpuContext, width: u32, height: u32) {
        if self.size.width != width || self.size.height != height {
            *self = Self::new(ctx, width, height, Some("depth texture"));
        }
    }

    /// Get the texture view.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Get the texture size.
    pub fn size(&self) -> (u32, u32) {
        (self.size.width, self.size.height)
    }
}

/// A cube map texture for environment mapping and reflections.
pub struct TextureCubeMap {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) size: u32,
    pub(crate) format: wgpu::TextureFormat,
}

impl TextureCubeMap {
    /// Create a new empty cube map texture.
    pub fn new(
        ctx: &WgpuContext,
        size: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        label: Option<&str>,
    ) -> Self {
        let extent = wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 6, // 6 faces
        };

        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        let sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: label.map(|l| format!("{l} sampler")).as_deref(),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
            size,
            format,
        }
    }

    /// Write data to a specific face of the cube map.
    /// Face order: +X, -X, +Y, -Y, +Z, -Z (indices 0-5)
    pub fn write_face(&self, ctx: &WgpuContext, face: u32, data: &[u8], bytes_per_row: u32) {
        assert!(face < 6, "Cube map face index must be 0-5");
        ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: face,
                },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(self.size),
            },
            wgpu::Extent3d {
                width: self.size,
                height: self.size,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Get the cube map view.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Get the sampler.
    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    /// Get the face size.
    pub fn size(&self) -> u32 {
        self.size
    }

    /// Get the texture format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }
}

/// A 2D texture array for shadow cascades, layered rendering, etc.
pub struct Texture2DArray {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) layer_views: Vec<wgpu::TextureView>,
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) layers: u32,
    pub(crate) format: wgpu::TextureFormat,
}

impl Texture2DArray {
    /// Create a new texture array.
    pub fn new(
        ctx: &WgpuContext,
        width: u32,
        height: u32,
        layers: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        label: Option<&str>,
    ) -> Self {
        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: layers,
        };

        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        // Create individual layer views
        let layer_views: Vec<_> = (0..layers)
            .map(|i| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: i,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();

        let sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: label.map(|l| format!("{l} sampler")).as_deref(),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Self {
            texture,
            view,
            layer_views,
            sampler,
            width,
            height,
            layers,
            format,
        }
    }

    /// Create a depth texture array (for shadow map cascades).
    pub fn new_depth(ctx: &WgpuContext, width: u32, height: u32, layers: u32) -> Self {
        Self::new(
            ctx,
            width,
            height,
            layers,
            DepthTexture::FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            Some("depth texture array"),
        )
    }

    /// Write data to a specific layer.
    pub fn write_layer(&self, ctx: &WgpuContext, layer: u32, data: &[u8], bytes_per_row: u32) {
        assert!(layer < self.layers, "Layer index out of bounds");
        ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(self.height),
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Get the array view (all layers).
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Get a specific layer's view.
    pub fn layer_view(&self, layer: u32) -> &wgpu::TextureView {
        &self.layer_views[layer as usize]
    }

    /// Get the sampler.
    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    /// Get the texture dimensions.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Get the number of layers.
    pub fn layers(&self) -> u32 {
        self.layers
    }

    /// Get the texture format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Create a comparison sampler for shadow mapping.
    pub fn create_comparison_sampler(ctx: &WgpuContext) -> wgpu::Sampler {
        ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow comparison sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        })
    }
}
