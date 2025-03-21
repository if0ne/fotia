use std::sync::Arc;

use crate::{
    collections::handle::Handle,
    multi_gpu_renderer::pso::PsoCollection,
    ra::{
        command::{RenderCommandContext, RenderCommandEncoder, RenderEncoder},
        context::{Context, RenderDevice},
        resources::Texture,
        shader::{RasterPipeline, RenderShaderContext, ShaderArgument, ShaderArgumentDesc},
        system::RenderSystem,
    },
    rhi::{
        command::CommandType,
        types::{GeomTopology, Viewport},
    },
};

pub struct GammaCorrectionPass<D: RenderDevice> {
    pub rs: Arc<RenderSystem>,
    pub ctx: Arc<Context<D>>,
    pub pso: Handle<RasterPipeline>,

    pub argument: Handle<ShaderArgument>,

    pub extent: [u32; 2],
}

impl<D: RenderDevice> GammaCorrectionPass<D> {
    pub fn new(
        rs: Arc<RenderSystem>,
        ctx: Arc<Context<D>>,
        psos: &PsoCollection<D>,
        extent: [u32; 2],
    ) -> Self {
        let argument = rs.create_shader_argument_handle();

        ctx.bind_shader_argument(
            argument,
            ShaderArgumentDesc {
                views: &[],
                samplers: &[],
                dynamic_buffer: None,
            },
        );

        Self {
            rs,
            ctx,
            pso: psos.gamma_corr_pass,
            argument,
            extent,
        }
    }

    pub fn render(&self, swapchain_view: Handle<Texture>) {
        let mut cmd = self.ctx.create_encoder(CommandType::Graphics);
        //cmd.set_barriers(&[gbuffer.accum -> PixelShaderResource]);

        {
            let mut encoder = cmd.render("Gamma Correction Pass".into(), &[swapchain_view], None);
            encoder.clear_rt(swapchain_view, [1.0, 1.0, 1.0, 1.0]);
            encoder.set_viewport(Viewport {
                x: 0.0,
                y: 0.0,
                w: self.extent[0] as f32,
                h: self.extent[1] as f32,
            });
            encoder.set_render_pipeline(self.pso);
            encoder.set_topology(GeomTopology::Triangles);
            encoder.bind_shader_argument(0, self.argument, 0);

            encoder.draw(3, 0);
        }

        self.ctx.enqueue(cmd);
    }

    pub fn resize(&mut self, extent: [u32; 2]) {
        self.extent = extent;

        self.ctx.unbind_shader_argument(self.argument);

        self.ctx.bind_shader_argument(
            self.argument,
            ShaderArgumentDesc {
                views: &[],
                samplers: &[],
                dynamic_buffer: None,
            },
        );
    }
}
