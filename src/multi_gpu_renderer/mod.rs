use glam::vec3;
use hecs::World;

use crate::{
    collections::handle::Handle,
    engine::{
        GpuMaterial, GpuMaterialComponent, GpuMeshComponent, GpuTransform, GpuTransformComponent,
        gltf::{GltfScene, ImageSource},
    },
    ra::{
        context::{ContextDual, RenderDevice},
        resources::{RenderResourceContext, Texture},
        shader::{RenderShaderContext, ShaderArgumentDesc, ShaderEntry},
        system::RenderSystem,
    },
    rhi::{
        resources::{BufferDesc, BufferUsages, MemoryLocation, TextureDesc, TextureUsages},
        types::Format,
    },
};

pub mod csm;
pub mod graphs;
pub mod passes;
pub mod pso;
pub mod shaders;

#[derive(Clone, Debug, Default)]
#[repr(C)]
#[repr(align(256))]
pub struct GpuGlobals {
    pub view: glam::Mat4,
    pub proj: glam::Mat4,
    pub proj_view: glam::Mat4,
    pub inv_view: glam::Mat4,
    pub inv_proj: glam::Mat4,
    pub inv_proj_view: glam::Mat4,

    pub eye_pos: glam::Vec3,
    pub _pad0: f32,

    pub screen_dim: glam::Vec2,
    pub _pad1: glam::Vec2,
}

pub struct TexturePlaceholders {
    pub diffuse: Handle<Texture>,
    pub normal: Handle<Texture>,
}

impl TexturePlaceholders {
    pub fn new<D: RenderDevice>(rs: &RenderSystem, group: &ContextDual<D>) -> Self {
        let diffuse = rs.create_texture_handle();
        let normal = rs.create_texture_handle();

        group.parallel(|ctx| {
            ctx.bind_texture(
                diffuse,
                TextureDesc::new_2d([1, 1], Format::Rgba8, TextureUsages::Resource)
                    .with_name("Diffuse Placeholder".into()),
                Some(&[255, 255, 255, 255]),
            );
        });

        group.parallel(|ctx| {
            ctx.bind_texture(
                normal,
                TextureDesc::new_2d([1, 1], Format::Rgba8, TextureUsages::Resource)
                    .with_name("Normal Placeholder".into()),
                Some(&[127, 127, 255, 255]),
            );
        });

        Self { diffuse, normal }
    }
}

pub fn create_multi_gpu_scene<D: RenderDevice>(
    scene: GltfScene,
    world: &mut World,
    rs: &RenderSystem,
    group: &ContextDual<D>,
    frames_in_flight: usize,
    dummy: &TexturePlaceholders,
) {
    let prepared = scene.prepare(rs);

    group.parallel(|ctx| {
        ctx.bind_buffer(
            prepared.positions,
            BufferDesc {
                name: Some("Position Vertex Buffer".into()),
                size: size_of_val(&scene.positions[..]),
                stride: size_of::<[f32; 3]>(),
                usage: BufferUsages::Vertex,
                memory_location: MemoryLocation::GpuToGpu,
            },
            Some(bytemuck::cast_slice(&scene.positions)),
        );

        for (buffer, argument) in prepared.submeshes.iter() {
            let data = (0..frames_in_flight)
                .map(|_| GpuTransform {
                    mat: glam::Mat4::from_scale(vec3(5.0, 5.0, 5.0)),
                })
                .collect::<Vec<_>>();

            ctx.bind_buffer(
                *buffer,
                BufferDesc {
                    name: Some("Object Position".into()),
                    size: frames_in_flight * size_of::<GpuTransform>(),
                    stride: 0,
                    usage: BufferUsages::Uniform,
                    memory_location: MemoryLocation::CpuToGpu,
                },
                None,
            );

            ctx.update_buffer(*buffer, 0, &data);

            ctx.bind_shader_argument(
                *argument,
                ShaderArgumentDesc {
                    views: &[],
                    samplers: &[],
                    dynamic_buffer: Some(*buffer),
                },
            );
        }
    });

    group.call_primary(|ctx| {
        ctx.bind_buffer(
            prepared.normals,
            BufferDesc {
                name: Some("Normal Vertex Buffer".into()),
                size: size_of_val(&scene.normals[..]),
                stride: size_of::<[f32; 3]>(),
                usage: BufferUsages::Vertex,
                memory_location: MemoryLocation::GpuToGpu,
            },
            Some(bytemuck::cast_slice(&scene.normals)),
        );

        ctx.bind_buffer(
            prepared.uvs,
            BufferDesc {
                name: Some("Uv Vertex Buffer".into()),
                size: size_of_val(&scene.uvs[..]),
                stride: size_of::<[f32; 2]>(),
                usage: BufferUsages::Vertex,
                memory_location: MemoryLocation::GpuToGpu,
            },
            Some(bytemuck::cast_slice(&scene.uvs)),
        );

        ctx.bind_buffer(
            prepared.tangents,
            BufferDesc {
                name: Some("Tangents Vertex Buffer".into()),
                size: size_of_val(&scene.tangents[..]),
                stride: size_of::<[f32; 4]>(),
                usage: BufferUsages::Vertex,
                memory_location: MemoryLocation::GpuToGpu,
            },
            Some(bytemuck::cast_slice(&scene.tangents)),
        );

        ctx.bind_buffer(
            prepared.indices,
            BufferDesc {
                name: Some("Index Buffer".into()),
                size: size_of_val(&scene.indices[..]),
                stride: size_of::<u32>(),
                usage: BufferUsages::Index,
                memory_location: MemoryLocation::GpuToGpu,
            },
            Some(bytemuck::cast_slice(&scene.indices)),
        );

        for (handle, image) in prepared.images.iter().zip(scene.images.iter()) {
            match image {
                ImageSource::Path(path) => {
                    let image = image::open(&path).expect("failed to load png").to_rgba8();

                    let filename = path
                        .file_stem()
                        .map(|n| n.to_string_lossy())
                        .map(|n| n.to_string())
                        .expect("failed to get filename");

                    ctx.bind_texture(
                        *handle,
                        TextureDesc::new_2d(
                            [image.width(), image.height()],
                            Format::Rgba8,
                            TextureUsages::Resource,
                        )
                        .with_name(filename.into()),
                        Some(image.as_raw()),
                    );
                }
                ImageSource::Data(_) => {
                    todo!()
                }
            }
        }

        for ((buffer, argument), material) in prepared.materials.iter().zip(scene.materials.iter())
        {
            ctx.bind_buffer(
                *buffer,
                BufferDesc {
                    name: Some("Object Material".into()),
                    size: size_of::<GpuMaterial>(),
                    stride: 0,
                    usage: BufferUsages::Uniform,
                    memory_location: MemoryLocation::CpuToGpu,
                },
                None,
            );

            ctx.update_buffer(
                *buffer,
                0,
                &[GpuMaterial {
                    diffuse_color: material.diffuse_color,
                    fresnel_r0: material.fresnel_r0,
                    roughness: material.roughness,
                }],
            );

            ctx.bind_shader_argument(
                *argument,
                ShaderArgumentDesc {
                    views: &[
                        // ShaderEntry::Srv(
                        //     material
                        //         .diffuse_map
                        //         .map(|idx| prepared.images[idx])
                        //         .unwrap_or(dummy.diffuse),
                        // ),
                        // ShaderEntry::Srv(
                        //     material
                        //         .normal_map
                        //         .map(|idx| prepared.images[idx])
                        //         .unwrap_or(dummy.normal),
                        // ),
                        ShaderEntry::Srv(dummy.diffuse),
                        ShaderEntry::Srv(dummy.normal),
                    ],
                    samplers: &[],
                    dynamic_buffer: Some(*buffer),
                },
            );
        }
    });

    for (mesh, (buffer, argument)) in scene.sub_meshes.iter().zip(prepared.submeshes) {
        let material = prepared.materials[mesh.material_idx];

        world.spawn((
            GpuTransformComponent { buffer, argument },
            GpuMeshComponent {
                pos_vb: prepared.positions,
                normal_vb: prepared.normals,
                uv_vb: prepared.uvs,
                tangent_vb: prepared.tangents,
                ib: prepared.indices,
                index_count: mesh.index_count,
                start_index_location: mesh.start_index_location,
                base_vertex_location: mesh.base_vertex_location,
            },
            GpuMaterialComponent {
                buffer: material.0,
                argument: material.1,
            },
        ));
    }
}
