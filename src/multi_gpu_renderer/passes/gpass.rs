use std::sync::Arc;

use hecs::World;

use crate::{
    collections::handle::Handle,
    engine::{GpuMaterialComponent, GpuMeshComponent, GpuTransform, GpuTransformComponent},
    multi_gpu_renderer::{GpuGlobals, pso::PsoCollection},
    ra::{
        command::{Barrier, RenderCommandContext, RenderCommandEncoder, RenderEncoder},
        context::{Context, RenderDevice},
        resources::{RenderResourceContext, Texture},
        shader::{RasterPipeline, ShaderArgument},
        system::RenderSystem,
    },
    rhi::{
        command::{CommandType, Subresource},
        resources::{TextureDesc, TextureUsages, TextureViewDesc, TextureViewType},
        types::{ClearColor, Format, GeomTopology, IndexType, ResourceState, Scissor, Viewport},
    },
};

pub struct GPass<D: RenderDevice> {
    pub rs: Arc<RenderSystem>,
    pub ctx: Arc<Context<D>>,

    pub extent: [u32; 2],

    pub diffuse: Handle<Texture>,
    pub diffuse_srv: Handle<Texture>,

    pub normal: Handle<Texture>,
    pub normal_srv: Handle<Texture>,

    pub material: Handle<Texture>,
    pub material_srv: Handle<Texture>,

    pub accum: Handle<Texture>,
    pub accum_srv: Handle<Texture>,

    pub depth: Handle<Texture>,

    pub pso: Handle<RasterPipeline>,
}

impl<D: RenderDevice> GPass<D> {
    pub fn new(
        rs: Arc<RenderSystem>,
        ctx: Arc<Context<D>>,
        extent: [u32; 2],
        depth: Handle<Texture>,
        psos: &PsoCollection<D>,
    ) -> Self {
        let diffuse = rs.create_texture_handle();
        let diffuse_srv = rs.create_texture_handle();

        ctx.bind_texture(
            diffuse,
            TextureDesc::new_2d(
                extent,
                Format::Rgba32,
                TextureUsages::RenderTarget | TextureUsages::Resource,
            )
            .with_name("Diffuse Texture".into())
            .with_color(ClearColor::Color([1.0, 1.0, 1.0, 1.0])),
            None,
        );

        ctx.bind_texture_view(
            diffuse_srv,
            diffuse,
            TextureViewDesc::default().with_view_type(TextureViewType::ShaderResource),
        );

        let normal = rs.create_texture_handle();
        let normal_srv = rs.create_texture_handle();

        ctx.bind_texture(
            normal,
            TextureDesc::new_2d(
                extent,
                Format::Rgba32,
                TextureUsages::RenderTarget | TextureUsages::Resource,
            )
            .with_name("Normal Texture".into())
            .with_color(ClearColor::Color([1.0, 1.0, 1.0, 1.0])),
            None,
        );

        ctx.bind_texture_view(
            normal_srv,
            normal,
            TextureViewDesc::default().with_view_type(TextureViewType::ShaderResource),
        );

        let material = rs.create_texture_handle();
        let material_srv = rs.create_texture_handle();

        ctx.bind_texture(
            material,
            TextureDesc::new_2d(
                extent,
                Format::Rgba32,
                TextureUsages::RenderTarget | TextureUsages::Resource,
            )
            .with_name("Material Texture".into())
            .with_color(ClearColor::Color([1.0, 1.0, 1.0, 1.0])),
            None,
        );

        ctx.bind_texture_view(
            material_srv,
            material,
            TextureViewDesc::default().with_view_type(TextureViewType::ShaderResource),
        );

        let accum = rs.create_texture_handle();
        let accum_srv = rs.create_texture_handle();

        ctx.bind_texture(
            accum,
            TextureDesc::new_2d(
                extent,
                Format::Rgba32,
                TextureUsages::RenderTarget | TextureUsages::Resource,
            )
            .with_name("Accumulation Texture".into())
            .with_color(ClearColor::Color([1.0, 1.0, 1.0, 1.0])),
            None,
        );

        ctx.bind_texture_view(
            accum_srv,
            accum,
            TextureViewDesc::default().with_view_type(TextureViewType::ShaderResource),
        );

        Self {
            rs,
            ctx,
            extent,
            pso: psos.g_pass,

            depth,
            diffuse,
            diffuse_srv,
            normal,
            normal_srv,
            material,
            material_srv,
            accum,
            accum_srv,
        }
    }

    pub fn render(&self, globals: Handle<ShaderArgument>, frame_idx: usize, world: &World) {
        let mut cmd = self.ctx.create_encoder(CommandType::Graphics);
        cmd.set_barriers(&[
            Barrier::Texture(
                self.diffuse,
                ResourceState::RenderTarget,
                Subresource::Local(None),
            ),
            Barrier::Texture(
                self.normal,
                ResourceState::RenderTarget,
                Subresource::Local(None),
            ),
            Barrier::Texture(
                self.material,
                ResourceState::RenderTarget,
                Subresource::Local(None),
            ),
            Barrier::Texture(
                self.accum,
                ResourceState::RenderTarget,
                Subresource::Local(None),
            ),
            Barrier::Texture(
                self.depth,
                ResourceState::DepthRead,
                Subresource::Local(None),
            ),
        ]);

        {
            let mut encoder = cmd.render(
                "GPass".into(),
                &[self.diffuse, self.normal, self.material],
                Some(self.depth),
            );
            encoder.set_render_pipeline(self.pso);

            encoder.clear_rt(self.diffuse, None);
            encoder.clear_rt(self.normal, None);
            encoder.clear_rt(self.material, None);
            encoder.clear_rt(self.accum, None);

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

            for (_, (transform, mesh, material)) in world
                .query::<(
                    &GpuTransformComponent,
                    &GpuMeshComponent,
                    &GpuMaterialComponent,
                )>()
                .iter()
            {
                encoder.bind_shader_argument(1, material.argument, 0);

                encoder.bind_shader_argument(
                    2,
                    transform.argument,
                    size_of::<GpuTransform>() * frame_idx,
                );
                encoder.bind_vertex_buffer(mesh.pos_vb, 0);
                encoder.bind_vertex_buffer(mesh.normal_vb, 1);
                encoder.bind_vertex_buffer(mesh.uv_vb, 2);
                encoder.bind_vertex_buffer(mesh.tangent_vb, 3);
                encoder.bind_index_buffer(mesh.ib, IndexType::U32);
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
            self.diffuse,
            TextureDesc::new_2d(
                extent,
                Format::Rgba32,
                TextureUsages::RenderTarget | TextureUsages::Resource,
            )
            .with_name("Diffuse Texture".into())
            .with_color(ClearColor::Color([0.0, 0.0, 0.0, 1.0])),
            None,
        );

        self.ctx.bind_texture_view(
            self.diffuse_srv,
            self.diffuse,
            TextureViewDesc::default().with_view_type(TextureViewType::ShaderResource),
        );

        self.ctx.bind_texture(
            self.normal,
            TextureDesc::new_2d(
                extent,
                Format::Rgba32,
                TextureUsages::RenderTarget | TextureUsages::Resource,
            )
            .with_name("Normal Texture".into())
            .with_color(ClearColor::Color([0.0, 0.0, 0.0, 1.0])),
            None,
        );

        self.ctx.bind_texture_view(
            self.normal_srv,
            self.normal,
            TextureViewDesc::default().with_view_type(TextureViewType::ShaderResource),
        );

        self.ctx.bind_texture(
            self.material,
            TextureDesc::new_2d(
                extent,
                Format::Rgba32,
                TextureUsages::RenderTarget | TextureUsages::Resource,
            )
            .with_name("Material Texture".into())
            .with_color(ClearColor::Color([0.0, 0.0, 0.0, 1.0])),
            None,
        );

        self.ctx.bind_texture_view(
            self.material_srv,
            self.material,
            TextureViewDesc::default().with_view_type(TextureViewType::ShaderResource),
        );

        self.ctx.bind_texture(
            self.accum,
            TextureDesc::new_2d(
                extent,
                Format::Rgba32,
                TextureUsages::RenderTarget | TextureUsages::Resource,
            )
            .with_name("Accum Texture".into())
            .with_color(ClearColor::Color([0.0, 0.0, 0.0, 1.0])),
            None,
        );

        self.ctx.bind_texture_view(
            self.accum_srv,
            self.accum,
            TextureViewDesc::default().with_view_type(TextureViewType::ShaderResource),
        );

        self.extent = extent;
    }
}
