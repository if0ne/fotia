use std::{borrow::Cow, fmt::Debug, path::Path};

use super::{
    resources::RenderResourceDevice,
    types::{
        AddressMode, ComparisonFunc, CullMode, DepthStateDesc, Filter, Format, InputElementDesc,
        ShaderType,
    },
};

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct CompiledShader {
    pub raw: Vec<u8>,
    pub ty: ShaderType,
}

pub trait RenderShaderDevice: RenderResourceDevice {
    type PipelineLayout: Send + Sync + Debug + 'static;
    type ShaderArgument: Send + Sync + Debug + 'static;

    type RasterPipeline: Send + Sync + Debug + 'static;

    fn create_pipeline_layout(&self, desc: PipelineLayoutDesc<'_>) -> Self::PipelineLayout;
    fn destroy_pipeline_layout(&self, layout: Self::PipelineLayout);

    fn create_shader_argument<
        'a,
        V: IntoIterator<Item = ShaderEntry<'a, Self>>,
        S: IntoIterator<Item = &'a Self::Sampler>,
    >(
        &self,
        desc: ShaderArgumentDesc<'a, Self, V, S>,
    ) -> Self::ShaderArgument;

    fn destroy_shader_argument(&self, argument: Self::ShaderArgument);

    fn create_raster_pipeline(&self, desc: RasterPipelineDesc<'_, Self>) -> Self::RasterPipeline;
    fn destroy_raster_pipeline(&self, pipeline: Self::RasterPipeline);
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
    pub nums: u32,
}

impl BindingEntry {
    pub fn new(ty: BindingType, nums: u32) -> Self {
        Self { ty, nums }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BindingSet<'a> {
    pub entries: &'a [BindingEntry],
    pub use_dynamic_buffer: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StaticSampler {
    pub ty: SamplerType,
    pub address_mode: AddressMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SamplerType {
    Sample(Filter),
    Comparasion(ComparisonFunc),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PipelineLayoutDesc<'a> {
    pub sets: &'a [BindingSet<'a>],
    pub static_samplers: &'a [StaticSampler],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShaderEntry<'a, D: RenderResourceDevice> {
    Cbv(&'a D::Buffer, usize),
    Srv(&'a D::Texture),
    Uav(&'a D::Texture),
}

#[derive(Clone, Debug)]
pub struct ShaderArgumentDesc<
    'a,
    D: RenderResourceDevice,
    V: IntoIterator<Item = ShaderEntry<'a, D>>,
    S: IntoIterator<Item = &'a D::Sampler>,
> {
    pub views: V,
    pub samplers: S,
    pub dynamic_buffer: Option<&'a D::Buffer>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShaderDesc<'a, P: AsRef<Path>> {
    pub ty: ShaderType,
    pub path: P,
    pub entry_point: Cow<'a, str>,
    pub debug: bool,
    pub defines: Vec<(Cow<'a, str>, Cow<'a, str>)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RasterPipelineDesc<'a, D: RenderShaderDevice> {
    pub layout: Option<&'a D::PipelineLayout>,
    pub input_elements: &'a [InputElementDesc],
    pub depth_bias: i32,
    pub slope_bias: f32,
    pub depth_clip: bool,
    pub depth: Option<DepthStateDesc>,
    pub render_targets: &'a [Format],
    pub cull_mode: CullMode,

    pub vs: &'a CompiledShader,
    pub shaders: &'a [&'a CompiledShader],
}
