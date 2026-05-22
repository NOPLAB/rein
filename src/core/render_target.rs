//! Render target abstraction
//!
//! Provides a convenient interface for rendering to textures or the screen.

use crate::context::WgpuContext;
use crate::core::render_states::ClearState;
use crate::core::texture::DepthTexture;

/// A render target that can be rendered to.
pub struct RenderTarget<'a> {
    pub(crate) ctx: &'a WgpuContext,
    pub(crate) color_view: &'a wgpu::TextureView,
    pub(crate) depth_view: Option<&'a wgpu::TextureView>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) format: wgpu::TextureFormat,
}

impl<'a> RenderTarget<'a> {
    /// Create a new render target.
    pub fn new(
        ctx: &'a WgpuContext,
        color_view: &'a wgpu::TextureView,
        depth_view: Option<&'a wgpu::TextureView>,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            ctx,
            color_view,
            depth_view,
            width,
            height,
            format,
        }
    }

    /// Create a render target from a surface texture.
    pub fn from_surface(
        ctx: &'a WgpuContext,
        surface_view: &'a wgpu::TextureView,
        depth_texture: Option<&'a DepthTexture>,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            ctx,
            color_view: surface_view,
            depth_view: depth_texture.map(DepthTexture::view),
            width,
            height,
            format,
        }
    }

    /// Get the render target width.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get the render target height.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get the aspect ratio.
    pub fn aspect(&self) -> f32 {
        self.width as f32 / self.height as f32
    }

    /// Get the texture format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Create a render pass with the given clear state.
    pub fn begin_render_pass<'p>(
        &'a self,
        encoder: &'p mut wgpu::CommandEncoder,
        clear: ClearState,
    ) -> wgpu::RenderPass<'p>
    where
        'a: 'p,
    {
        let color_attachment = wgpu::RenderPassColorAttachment {
            view: self.color_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: clear.color_load_op(),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        };

        let depth_attachment = self
            .depth_view
            .map(|view| wgpu::RenderPassDepthStencilAttachment {
                view,
                depth_ops: Some(wgpu::Operations {
                    load: clear.depth_load_op(),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            });

        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render pass"),
            color_attachments: &[Some(color_attachment)],
            depth_stencil_attachment: depth_attachment,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }

    /// Clear the render target with the given clear state.
    pub fn clear(&self, clear: ClearState) {
        let mut encoder = self.ctx.create_encoder(Some("clear encoder"));
        {
            let _pass = self.begin_render_pass(&mut encoder, clear);
            // Pass drops immediately, just clearing
        }
        self.ctx.submit([encoder.finish()]);
    }

    /// Get a reference to the context.
    pub fn context(&self) -> &WgpuContext {
        self.ctx
    }

    /// Get the color view.
    pub fn color_view(&self) -> &wgpu::TextureView {
        self.color_view
    }

    /// Get the depth view if available.
    pub fn depth_view(&self) -> Option<&wgpu::TextureView> {
        self.depth_view
    }
}
