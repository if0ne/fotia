use std::sync::Arc;

use hecs::World;
use smallvec::SmallVec;

use crate::{
    collections::{handle::Handle, rwc_ring_buffer::RwcRingBuffer},
    engine::{GpuMeshComponent, GpuTransform, GpuTransformComponent, camera::Camera},
    multi_gpu_renderer::{
        csm::{Cascade, CascadedShadowMaps, Cascades},
        pso::PsoCollection,
    },
    ra::{
        command::{Barrier, RenderCommandContext, RenderCommandEncoder, RenderEncoder},
        context::{ContextDual, RenderDevice},
        resources::{Buffer, RenderResourceContext, Texture},
        shader::{
            RasterPipeline, RenderShaderContext, ShaderArgument, ShaderArgumentDesc, ShaderEntry,
        },
        system::RenderSystem,
    },
    rhi::{
        command::{CommandType, Subresource},
        resources::{
            BufferDesc, BufferUsages, TextureDesc, TextureUsages, TextureViewDesc, TextureViewType,
        },
        types::{ClearColor, Format, GeomTopology, IndexType, ResourceState, Scissor, Viewport},
    },
};

pub struct MultiCascadedShadowMapsPass<D: RenderDevice> {
    pub rs: Arc<RenderSystem>,
    pub group: Arc<ContextDual<D>>,

    pub size: u32,

    pub csm: CascadedShadowMaps,

    pub gpu_csm_buffer: Handle<Buffer>,
    pub argument: SmallVec<[Handle<ShaderArgument>; 4]>,

    pub gpu_csm_proj_view_buffer: Handle<Buffer>,
    pub local_argument: Handle<ShaderArgument>,

    pub shared: RwcRingBuffer<Handle<Texture>, 4>,

    pub pso: Handle<RasterPipeline>,
}

impl<D: RenderDevice> MultiCascadedShadowMapsPass<D> {
    pub fn new(
        rs: Arc<RenderSystem>,
        group: Arc<ContextDual<D>>,
        size: u32,
        lambda: f32,
        psos: &PsoCollection<D>,
        frames_in_flight: usize,
    ) -> Self {
        let texture_count = frames_in_flight.min(3);

        let shared = (0..texture_count)
            .map(|_| rs.create_texture_handle())
            .collect::<SmallVec<_>>();

        let gpu_csm_buffer = rs.create_buffer_handle();
        let gpu_csm_proj_view_buffer = rs.create_buffer_handle();

        let argument = (0..texture_count)
            .map(|_| rs.create_shader_argument_handle())
            .collect::<SmallVec<_>>();
        let local_argument = rs.create_shader_argument_handle();

        group.call_secondary(|ctx| {
            shared.iter().enumerate().for_each(|(i, t)| {
                ctx.bind_texture(
                    *t,
                    TextureDesc::new_2d(
                        [2 * size, 2 * size],
                        Format::D32,
                        TextureUsages::DepthTarget
                            | TextureUsages::Resource
                            | TextureUsages::Shared,
                    )
                    .with_name(format!("Shared Cascaded Shadow Maps {i}").into())
                    .with_color(ClearColor::Depth(1.0)),
                    None,
                );
            });

            ctx.bind_buffer(
                gpu_csm_proj_view_buffer,
                BufferDesc::cpu_to_gpu(
                    size_of::<Cascade>() * texture_count * 4,
                    BufferUsages::Uniform,
                )
                .with_name("CSM Proj View Buffer".into()),
                None,
            );

            ctx.bind_shader_argument(
                local_argument,
                ShaderArgumentDesc {
                    views: &[],
                    samplers: &[],
                    dynamic_buffer: Some(gpu_csm_proj_view_buffer),
                },
            );
        });

        group.call_primary(|ctx| {
            shared.iter().enumerate().for_each(|(_, t)| {
                ctx.open_texture_handle(
                    *t,
                    &group.secondary,
                    Some(
                        TextureViewDesc::default()
                            .with_view_type(TextureViewType::ShaderResource)
                            .with_format(Format::R32),
                    ),
                );
            });

            ctx.bind_buffer(
                gpu_csm_buffer,
                BufferDesc::cpu_to_gpu(
                    size_of::<Cascades>() * texture_count,
                    BufferUsages::Uniform,
                )
                .with_name("CSM Buffer".into()),
                None,
            );

            shared.iter().zip(argument.iter()).for_each(|(srv, arg)| {
                ctx.bind_shader_argument(
                    *arg,
                    ShaderArgumentDesc {
                        views: &[ShaderEntry::Srv(*srv)],
                        samplers: &[],
                        dynamic_buffer: Some(gpu_csm_buffer),
                    },
                );
            });
        });

        Self {
            rs,
            group,
            size,
            csm: CascadedShadowMaps::new(lambda),
            gpu_csm_buffer,
            argument,
            gpu_csm_proj_view_buffer,
            local_argument,
            pso: psos.csm_pass,
            shared: RwcRingBuffer::new(shared),
        }
    }

    pub fn update(&mut self, camera: &Camera, light_dir: glam::Vec3) {
        self.csm.update(camera, light_dir);

        self.group.call_primary(|ctx| {
            ctx.update_buffer(
                self.gpu_csm_buffer,
                self.shared.head,
                &[self.csm.cascades.clone()],
            );
        });

        self.group.call_secondary(|ctx| {
            for i in 0..4 {
                ctx.update_buffer(
                    self.gpu_csm_proj_view_buffer,
                    4 * self.shared.head + i,
                    &[Cascade {
                        proj_view: self.csm.cascades.cascade_proj_views[i],
                    }],
                );
            }
        });
    }

    pub fn render(&self, world: &World) {
        self.group.call_secondary(|ctx| {
            let mut cmd = ctx.create_encoder(CommandType::Graphics);
            cmd.set_barriers(&[Barrier::Texture(
                *self.shared.head_data(),
                ResourceState::DepthWrite,
                Subresource::Local(None),
            )]);

            {
                let mut encoder = cmd.render(
                    "Cascaded Shadow Maps".into(),
                    &[],
                    Some(*self.shared.head_data()),
                );
                encoder.set_render_pipeline(self.pso);
                encoder.clear_depth(*self.shared.head_data(), None);
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
                        size_of::<Cascade>() * (self.shared.head * 4 + i as usize),
                    );

                    for (_, (transform, mesh)) in world
                        .query::<(&GpuTransformComponent, &GpuMeshComponent)>()
                        .iter()
                    {
                        encoder.bind_shader_argument(
                            1,
                            transform.argument,
                            size_of::<GpuTransform>() * self.shared.head,
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

            ctx.enqueue(cmd);
        });
    }
}
