use glam;

use crate::{
    collections::handle::Handle,
    ra::{
        resources::{Buffer, Texture},
        shader::ShaderArgument,
    },
};

#[derive(Clone, Debug)]
pub struct TransformComponent {
    pub pos: glam::Vec3,
    pub rotation: glam::Quat,
    pub scale: f32,
}

#[derive(Clone, Debug)]
pub struct GpuTransformComponent {
    pub buffer: Handle<Buffer>,
    pub argument: Handle<ShaderArgument>,
}

#[derive(Clone, Debug)]
pub struct MeshComponent {
    pub pos_vb: Handle<Buffer>,
    pub normal_vb: Handle<Buffer>,
    pub uv_vb: Handle<Buffer>,
    pub tangent_vb: Handle<Buffer>,

    pub ib: Handle<Buffer>,

    pub index_count: u32,
    pub start_index_location: u32,
    pub base_vertex_location: u32,
}

#[derive(Clone, Debug)]
pub struct MaterialComponent {
    pub diffuse_color: [f32; 4],
    pub fresnel_r0: f32,
    pub roughness: f32,

    pub diffuse_map: Handle<Texture>,
    pub normal_map: Handle<Texture>,
}

#[derive(Clone, Debug)]
pub struct GpuMaterialComponent {
    pub buffer: Handle<Buffer>,
    pub argument: Handle<ShaderArgument>,
}
