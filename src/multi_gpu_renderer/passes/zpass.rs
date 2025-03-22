use std::sync::Arc;

use hecs::World;

use crate::{
    collections::handle::Handle,
    engine::{GpuTransform, GpuTransformComponent, MeshComponent},
    multi_gpu_renderer::{GpuGlobals, pso::PsoCollection},
    ra::{
        command::{Barrier, RenderCommandContext, RenderCommandEncoder, RenderEncoder},
        context::{Context, RenderDevice},
        resources::{RenderResourceContext, Texture},
        shader::{RasterPipeline, ShaderArgument},
        system::RenderSystem,
    },
    rhi::{
        command::CommandType,
        resources::{TextureDesc, TextureUsages},
        types::{ClearColor, Format, GeomTopology, IndexType, ResourceState, Scissor, Viewport},
    },
};

pub struct ZPass<D: RenderDevice> {
    pub rs: Arc<RenderSystem>,
    pub ctx: Arc<Context<D>>,

    pub extent: [u32; 2],
    pub depth: Handle<Texture>,
    pub pso: Handle<RasterPipeline>,
}

impl<D: RenderDevice> ZPass<D> {
    pub fn new(
        rs: Arc<RenderSystem>,
        ctx: Arc<Context<D>>,
        extent: [u32; 2],
        psos: &PsoCollection<D>,
    ) -> Self {
        let depth = rs.create_texture_handle();

        ctx.bind_texture(
            depth,
            TextureDesc::new_2d(extent, Format::D24S8, TextureUsages::DepthTarget)
                .with_name("Prepass Depth".into())
                .with_color(ClearColor::Depth(1.0)),
            None,
        );

        Self {
            rs,
            ctx,
            extent,
            depth,
            pso: psos.zpass,
        }
    }

    pub fn render(&self, globals: Handle<ShaderArgument>, frame_idx: usize, world: &World) {
        let mut cmd = self.ctx.create_encoder(CommandType::Graphics);
        cmd.set_barriers(&[Barrier::Texture(self.depth, ResourceState::DepthWrite)]);

        {
            let mut encoder = cmd.render("Z Prepass".into(), &[], Some(self.depth));
            encoder.set_render_pipeline(self.pso);

            encoder.clear_depth(self.depth, None);
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
            encoder.bind_shader_argument(0, globals, size_of::<GpuGlobals>() * frame_idx);

            for (_, (transform, mesh)) in world
                .query::<(&GpuTransformComponent, &MeshComponent)>()
                .iter()
            {
                encoder.bind_shader_argument(
                    1,
                    transform.argument,
                    size_of::<GpuTransform>() * frame_idx,
                );
                encoder.bind_vertex_buffer(mesh.pos_vb, 0);
                encoder.bind_index_buffer(mesh.ib, IndexType::U16);
                encoder.draw_indexed(
                    mesh.index_count,
                    mesh.start_index_location,
                    mesh.base_vertex_location,
                );
            }
        }

        self.ctx.enqueue(cmd);
    }

    pub fn resize(&mut self, extent: [u32; 2]) {
        self.ctx.bind_texture(
            self.depth,
            TextureDesc::new_2d(extent, Format::D24S8, TextureUsages::DepthTarget)
                .with_name("Prepass Depth".into())
                .with_color(ClearColor::Depth(1.0)),
            None,
        );

        self.extent = extent;
    }
}
