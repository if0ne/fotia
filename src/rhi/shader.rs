use super::{
    resources::RenderResourceDevice,
    types::{AddressMode, Filter},
};

pub trait RenderShaderDevice: RenderResourceDevice {
    type PipelineLayout;
    type ShaderArgument;

    fn create_pipeline_layout(&self, desc: PipelineLayoutDesc<'_>) -> Self::PipelineLayout;
    fn destroy_pipeline_layout(&self, layout: Self::PipelineLayout);

    fn create_shader_argument(
        &self,
        desc: ShaderArgumentDesc<'_, '_, Self>,
    ) -> Self::ShaderArgument;

    fn destroy_shader_argument(&self, argument: Self::ShaderArgument);
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum BindingType {
    Cbv,
    Uav,
    Srv,
    Sampler,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct BindingEntry {
    pub ty: BindingType,
    pub slot: u32,
    pub nums: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BindingSet<'a> {
    pub entries: &'a [BindingEntry],
    pub space: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StaticSampler {
    pub slot: u32,
    pub filter: Filter,
    pub address_mode: AddressMode,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PipelineLayoutDesc<'a> {
    pub sets: &'a [BindingSet<'a>],
    pub static_samplers: &'a [StaticSampler],
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Binding<T> {
    pub binding: T,
    pub slot: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShaderArgumentDesc<'a, 'b, D: RenderResourceDevice> {
    pub textures: &'a [&'b Binding<D::Texture>],
    pub samplers: &'a [&'b Binding<D::Sampler>],
    pub dynamic_buffer: Option<&'b Binding<D::Buffer>>,
}
