use std::sync::Arc;

use hecs::World;

use crate::{
    collections::handle::Handle,
    engine::{GpuMeshComponent, GpuTransform, GpuTransformComponent, camera::Camera},
    multi_gpu_renderer::{
        csm::{Cascade, CascadedShadowMaps, Cascades},
        pso::PsoCollection,
    },
    ra::{
        command::{Barrier, RenderCommandContext, RenderCommandEncoder, RenderEncoder},
        context::{Context, RenderDevice},
        resources::{Buffer, RenderResourceContext, Texture},
        shader::{
            RasterPipeline, RenderShaderContext, ShaderArgument, ShaderArgumentDesc, ShaderEntry,
        },
        system::RenderSystem,
    },
    rhi::{
        command::CommandType,
        resources::{
            BufferDesc, BufferUsages, TextureDesc, TextureUsages, TextureViewDesc, TextureViewType,
        },
        types::{ClearColor, Format, GeomTopology, IndexType, ResourceState, Scissor, Viewport},
    },
};

pub struct CascadedShadowMapsPass<D: RenderDevice> {
    pub rs: Arc<RenderSystem>,
    pub ctx: Arc<Context<D>>,

    pub size: u32,

    pub csm: CascadedShadowMaps,

    pub gpu_csm_buffer: Handle<Buffer>,
    pub argument: Handle<ShaderArgument>,

    pub gpu_csm_proj_view_buffer: Handle<Buffer>,
    pub local_argument: Handle<ShaderArgument>,

    pub dsv: Handle<Texture>,
    pub srv: Handle<Texture>,

    pub pso: Handle<RasterPipeline>,
}

impl<D: RenderDevice> CascadedShadowMapsPass<D> {
    pub fn new(
        rs: Arc<RenderSystem>,
        ctx: Arc<Context<D>>,
        size: u32,
        lambda: f32,
        psos: &PsoCollection<D>,
        frames_in_flight: usize,
    ) -> Self {
        let dsv = rs.create_texture_handle();
        let srv = rs.create_texture_handle();

        let gpu_csm_buffer = rs.create_buffer_handle();
        let gpu_csm_proj_view_buffer = rs.create_buffer_handle();

        let argument = rs.create_shader_argument_handle();
        let local_argument = rs.create_shader_argument_handle();

        ctx.bind_texture(
            dsv,
            TextureDesc::new_2d(
                [2 * size, 2 * size],
                Format::D32,
                TextureUsages::DepthTarget | TextureUsages::Resource,
            )
            .with_color(ClearColor::Depth(1.0)),
            None,
        );

        ctx.bind_texture_view(
            srv,
            dsv,
            TextureViewDesc::default()
                .with_view_type(TextureViewType::ShaderResource)
                .with_format(Format::R32),
        );

        ctx.bind_buffer(
            gpu_csm_buffer,
            BufferDesc::cpu_to_gpu(
                size_of::<Cascades>() * frames_in_flight,
                BufferUsages::Uniform,
            )
            .with_name("CSM Buffer".into()),
            None,
        );

        ctx.bind_buffer(
            gpu_csm_proj_view_buffer,
            BufferDesc::cpu_to_gpu(
                size_of::<Cascade>() * frames_in_flight * 4,
                BufferUsages::Uniform,
            )
            .with_name("CSM Proj View Buffer".into()),
            None,
        );

        ctx.bind_shader_argument(
            argument,
            ShaderArgumentDesc {
                views: &[ShaderEntry::Srv(srv)],
                samplers: &[],
                dynamic_buffer: Some(gpu_csm_buffer),
            },
        );

        ctx.bind_shader_argument(
            local_argument,
            ShaderArgumentDesc {
                views: &[],
                samplers: &[],
                dynamic_buffer: Some(gpu_csm_proj_view_buffer),
            },
        );

        Self {
            rs,
            ctx,
            size,
            csm: CascadedShadowMaps::new(lambda),
            gpu_csm_buffer,
            argument,
            gpu_csm_proj_view_buffer,
            local_argument,
            dsv,
            srv,
            pso: psos.csm_pass,
        }
    }

    pub fn update(&mut self, camera: &Camera, light_dir: glam::Vec3, frame_index: usize) {
        self.csm.update(camera, light_dir);

        self.ctx.update_buffer(
            self.gpu_csm_buffer,
            frame_index,
            &[self.csm.cascades.clone()],
        );

        for i in 0..4 {
            self.ctx.update_buffer(
                self.gpu_csm_proj_view_buffer,
                4 * frame_index + i,
                &[Cascade {
                    proj_view: self.csm.cascades.cascade_proj_views[i],
                }],
            );
        }
    }

    pub fn render(&self, frame_idx: usize, world: &World) {
        let mut cmd = self.ctx.create_encoder(CommandType::Graphics);
        cmd.set_barriers(&[Barrier::Texture(self.dsv, ResourceState::DepthWrite)]);

        {
            let mut encoder = cmd.render("Cascaded Shadow Maps".into(), &[], Some(self.dsv));
            encoder.set_render_pipeline(self.pso);
            encoder.clear_depth(self.dsv, None);
            encoder.set_topology(GeomTopology::Triangles);

            encoder.set_scissor(Scissor {
                x: 0,
                y: 0,
                w: self.size * 2,
                h: self.size * 2,
            });

            for i in 0..4 {
                let row = i / 2;
                let col = i % 2;
                encoder.set_viewport(Viewport {
                    x: (self.size * col) as f32,
                    y: (self.size * row) as f32,
                    w: self.size as f32,
                    h: self.size as f32,
                });

                encoder.bind_shader_argument(
                    0,
                    self.local_argument,
                    size_of::<Cascade>() * (frame_idx * 4 + i as usize),
                );

                for (_, (transform, mesh)) in world
                    .query::<(&GpuTransformComponent, &GpuMeshComponent)>()
                    .iter()
                {
                    encoder.bind_shader_argument(
                        1,
                        transform.argument,
                        size_of::<GpuTransform>() * frame_idx,
                    );
                    encoder.bind_vertex_buffer(mesh.pos_vb, 0);
                    encoder.bind_index_buffer(mesh.ib, IndexType::U32);
                    encoder.draw_indexed(
                        mesh.index_count,
                        mesh.start_index_location,
                        mesh.base_vertex_location,
                    );
                }
            }
        }

        self.ctx.enqueue(cmd);
    }
}
