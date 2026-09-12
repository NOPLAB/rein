//! An off-screen colour + depth target with CPU read-back (tests, thumbnails, XR eyes).

use crate::context::WgpuContext;
use crate::core::render_target::RenderTarget;
use crate::core::texture::DepthTexture;

/// A render target backed by textures instead of a surface.
pub struct OffscreenTarget {
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth: DepthTexture,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
}

impl OffscreenTarget {
    /// Allocate a `width` × `height` target of `format` (readable back to the CPU).
    pub fn new(ctx: &WgpuContext, width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        let color = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen color"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
        let depth = DepthTexture::new(ctx, width, height, Some("offscreen depth"));
        Self {
            color,
            color_view,
            depth,
            width,
            height,
            format,
        }
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Colour format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// The colour texture (e.g. to sample it elsewhere).
    pub fn texture(&self) -> &wgpu::Texture {
        &self.color
    }

    /// The colour view.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.color_view
    }

    /// Borrow as a [`RenderTarget`].
    pub fn target<'a>(&'a self, ctx: &'a WgpuContext) -> RenderTarget<'a> {
        RenderTarget::from_surface(
            ctx,
            &self.color_view,
            Some(&self.depth),
            self.width,
            self.height,
            self.format,
        )
    }

    /// Copy the colour texture to the CPU as tightly packed rows of 4-byte pixels
    /// (in the texture's own channel order, e.g. BGRA for `Bgra8UnormSrgb`). Blocks.
    pub fn read_pixels(&self, ctx: &WgpuContext) -> Vec<u8> {
        let bytes_per_pixel = 4_u32;
        let unpadded = self.width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;
        let size = u64::from(padded) * u64::from(self.height);
        let staging = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("offscreen readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = ctx.create_encoder(Some("offscreen readback"));
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.color,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        ctx.submit([encoder.finish()]);

        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        let _ = ctx.device.poll(wgpu::PollType::wait_indefinitely());
        if !matches!(rx.recv(), Ok(Ok(()))) {
            tracing::error!("offscreen read-back mapping failed");
            return Vec::new();
        }
        let data = slice.get_mapped_range();
        let mut out = Vec::with_capacity((unpadded * self.height) as usize);
        for row in data.chunks_exact(padded as usize) {
            out.extend_from_slice(&row[..unpadded as usize]);
        }
        drop(data);
        staging.unmap();
        out
    }
}
