use std::sync::Arc;

use glam::{vec3, vec4};

use crate::{
    collections::handle::Handle,
    multi_gpu_renderer::{GpuGlobals, csm::Cascades, pso::PsoCollection},
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
        command::{CommandType, Subresource},
        resources::{BufferDesc, BufferUsages},
        types::{GeomTopology, ResourceState, Scissor, Viewport},
    },
};

#[derive(Clone, Debug)]
#[repr(C)]
pub struct GpuDirectionalLight {
    pub strength: glam::Vec3,
    pub _pad: f32,
    pub direction: glam::Vec3,
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct GpuAmbientLight {
    pub color: glam::Vec4,
}

#[derive(Clone, Debug)]
#[repr(C)]
#[repr(align(256))]
pub struct LightData {
    pub dir_light: GpuDirectionalLight,
    pub ambient_light: GpuAmbientLight,
}

pub struct DirectionalLightPass<D: RenderDevice> {
    pub rs: Arc<RenderSystem>,
    pub ctx: Arc<Context<D>>,

    pub extent: [u32; 2],

    pub argument: Handle<ShaderArgument>,
    pub light_data: Handle<Buffer>,

    pub diffuse_srv: Handle<Texture>,
    pub normal_srv: Handle<Texture>,
    pub material_srv: Handle<Texture>,
    pub accum: Handle<Texture>,

    pub pso: Handle<RasterPipeline>,
}

impl<D: RenderDevice> DirectionalLightPass<D> {
    pub fn new(
        rs: Arc<RenderSystem>,
        ctx: Arc<Context<D>>,
        extent: [u32; 2],
        diffuse_srv: Handle<Texture>,
        normal_srv: Handle<Texture>,
        material_srv: Handle<Texture>,
        accum: Handle<Texture>,
        frames_in_flight: usize,
        psos: &PsoCollection<D>,
    ) -> Self {
        let argument = rs.create_shader_argument_handle();
        let light_data = rs.create_buffer_handle();

        ctx.bind_buffer(
            light_data,
            BufferDesc::cpu_to_gpu(
                size_of::<LightData>() * frames_in_flight,
                BufferUsages::Uniform,
            )
            .with_name("Light Date Buffer".into()),
            None,
        );

        ctx.update_buffer(
            light_data,
            0,
            &[
                LightData {
                    dir_light: GpuDirectionalLight {
                        strength: vec3(1.0, 0.81, 0.16),
                        _pad: 0.0,
                        direction: vec3(-1.0, -1.0, -1.0),
                    },
                    ambient_light: GpuAmbientLight {
                        color: vec4(0.3, 0.3, 0.63, 1.0),
                    },
                },
                LightData {
                    dir_light: GpuDirectionalLight {
                        strength: vec3(1.0, 0.81, 0.16),
                        _pad: 0.0,
                        direction: vec3(-1.0, -1.0, -1.0),
                    },
                    ambient_light: GpuAmbientLight {
                        color: vec4(0.3, 0.3, 0.63, 1.0),
                    },
                },
                LightData {
                    dir_light: GpuDirectionalLight {
                        strength: vec3(1.0, 0.81, 0.16),
                        _pad: 0.0,
                        direction: vec3(-1.0, -1.0, -1.0),
                    },
                    ambient_light: GpuAmbientLight {
                        color: vec4(0.3, 0.3, 0.63, 1.0),
                    },
                },
            ],
        );

        ctx.bind_shader_argument(
            argument,
            ShaderArgumentDesc {
                views: &[
                    ShaderEntry::Srv(diffuse_srv),
                    ShaderEntry::Srv(normal_srv),
                    ShaderEntry::Srv(material_srv),
                ],
                samplers: &[],
                dynamic_buffer: Some(light_data),
            },
        );

        Self {
            rs,
            ctx,
            extent,
            pso: psos.directional_light_pass,
            argument,
            light_data,
            accum,
            diffuse_srv,
            normal_srv,
            material_srv,
        }
    }

    pub fn render(
        &self,
        globals: Handle<ShaderArgument>,
        csm: Handle<Texture>,
        csm_data: Handle<ShaderArgument>,
        frame_idx: usize,
        cascade_idx: usize,
    ) {
        let mut cmd = self.ctx.create_encoder(CommandType::Graphics);
        cmd.set_barriers(&[
            Barrier::Texture(
                self.accum,
                ResourceState::RenderTarget,
                Subresource::Local(None),
            ),
            Barrier::Texture(
                self.normal_srv,
                ResourceState::Shader,
                Subresource::Local(None),
            ),
            Barrier::Texture(
                self.diffuse_srv,
                ResourceState::Shader,
                Subresource::Local(None),
            ),
            Barrier::Texture(
                self.material_srv,
                ResourceState::Shader,
                Subresource::Local(None),
            ),
            Barrier::Texture(csm, ResourceState::Shader, Subresource::Local(None)),
        ]);

        {
            let mut encoder = cmd.render("Directional Light Pass".into(), &[self.accum], None);
            encoder.set_render_pipeline(self.pso);

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
            encoder.bind_shader_argument(1, self.argument, 0);
            encoder.bind_shader_argument(2, csm_data, size_of::<Cascades>() * cascade_idx);

            encoder.draw(3, 0);
        }

        cmd.set_barriers(&[Barrier::Texture(
            csm,
            ResourceState::Common,
            Subresource::Local(None),
        )]);

        self.ctx.enqueue(cmd);
    }

    pub fn resize(&mut self, extent: [u32; 2]) {
        self.extent = extent;

        self.ctx.bind_shader_argument(
            self.argument,
            ShaderArgumentDesc {
                views: &[
                    ShaderEntry::Srv(self.diffuse_srv),
                    ShaderEntry::Srv(self.normal_srv),
                    ShaderEntry::Srv(self.material_srv),
                ],
                samplers: &[],
                dynamic_buffer: Some(self.light_data),
            },
        );
    }
}
