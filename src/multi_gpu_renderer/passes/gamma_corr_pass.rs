use std::sync::Arc;

use crate::{
    collections::handle::Handle,
    multi_gpu_renderer::pso::PsoCollection,
    ra::{
        command::{Barrier, RenderCommandContext, RenderCommandEncoder, RenderEncoder},
        context::{Context, RenderDevice},
        resources::Texture,
        shader::{
            RasterPipeline, RenderShaderContext, ShaderArgument, ShaderArgumentDesc, ShaderEntry,
        },
        system::RenderSystem,
    },
    rhi::{
        command::{CommandType, Subresource},
        types::{GeomTopology, ResourceState, Scissor, Viewport},
    },
};

pub struct GammaCorrectionPass<D: RenderDevice> {
    pub rs: Arc<RenderSystem>,
    pub ctx: Arc<Context<D>>,
    pub pso: Handle<RasterPipeline>,

    pub argument: Handle<ShaderArgument>,
    pub accum_srv: Handle<Texture>,

    pub extent: [u32; 2],
}

impl<D: RenderDevice> GammaCorrectionPass<D> {
    pub fn new(
        rs: Arc<RenderSystem>,
        ctx: Arc<Context<D>>,
        psos: &PsoCollection<D>,
        accum_srv: Handle<Texture>,
        extent: [u32; 2],
    ) -> Self {
        let argument = rs.create_shader_argument_handle();

        ctx.bind_shader_argument(
            argument,
            ShaderArgumentDesc {
                views: &[ShaderEntry::Srv(accum_srv)],
                samplers: &[],
                dynamic_buffer: None,
            },
        );

        Self {
            rs,
            ctx,
            pso: psos.gamma_corr_pass,
            argument,
            accum_srv,
            extent,
        }
    }

    pub fn render(&self, swapchain_view: Handle<Texture>) {
        let mut cmd = self.ctx.create_encoder(CommandType::Graphics);
        cmd.set_barriers(&[Barrier::Texture(
            self.accum_srv,
            ResourceState::Shader,
            Subresource::Local(None),
        )]);

        {
            let mut encoder = cmd.render("Gamma Correction Pass".into(), &[swapchain_view], None);
            encoder.set_render_pipeline(self.pso);

            encoder.clear_rt(swapchain_view, Some([1.0, 1.0, 1.0, 1.0]));
            encoder.set_viewport(Viewport {
                x: 0.0,
                y: 0.0,
                w: self.extent[0] as f32,
                h: self.extent[1] as f32,
            });
            encoder.set_scissor(Scissor {
                x: 0,
                y: 0,
                w: self.extent[0],
                h: self.extent[1],
            });

            encoder.set_topology(GeomTopology::Triangles);
            encoder.bind_shader_argument(0, self.argument, 0);

            encoder.draw(3, 0);
        }

        self.ctx.enqueue(cmd);
    }

    pub fn resize(&mut self, extent: [u32; 2]) {
        self.extent = extent;

        self.ctx.bind_shader_argument(
            self.argument,
            ShaderArgumentDesc {
                views: &[ShaderEntry::Srv(self.accum_srv)],
                samplers: &[],
                dynamic_buffer: None,
            },
        );
    }
}
