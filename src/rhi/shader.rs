use std::{collections::HashSet, path::PathBuf};

use super::{
    resources::RenderResourceDevice,
    types::{AddressMode, CullMode, DepthStateDesc, Filter, Format, InputElementDesc, ShaderType},
};

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct CompiledShader {
    pub raw: Vec<u8>,
    pub desc: ShaderDesc,
}

pub trait RenderShaderDevice: RenderResourceDevice {
    type PipelineLayout;
    type ShaderArgument;

    type RasterPipeline;

    fn create_pipeline_layout(&self, desc: PipelineLayoutDesc<'_>) -> Self::PipelineLayout;
    fn destroy_pipeline_layout(&self, layout: Self::PipelineLayout);

    fn create_shader_argument(
        &self,
        desc: ShaderArgumentDesc<'_, '_, Self>,
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
    pub filter: Filter,
    pub address_mode: AddressMode,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PipelineLayoutDesc<'a> {
    pub sets: &'a [BindingSet<'a>],
    pub static_samplers: &'a [StaticSampler],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShaderArgument<'a, D: RenderResourceDevice> {
    Cbv(&'a D::Buffer, usize),
    Srv(&'a D::Texture),
    Uav(&'a D::Texture),
    Sample(&'a D::Sampler),
}

#[derive(Clone, Debug)]
pub struct ShaderArgumentDesc<'a, 'b, D: RenderResourceDevice> {
    pub arguments: &'a [ShaderArgument<'b, D>],
    pub dynamic_buffer: Option<&'b D::Buffer>,
}

#[derive(Clone, Debug, Eq)]
pub struct ShaderDesc {
    pub ty: ShaderType,
    pub path: PathBuf,
    pub entry_point: String,
    pub debug: bool,
    pub defines: Vec<(String, String)>,
}

impl std::hash::Hash for ShaderDesc {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let mut set: HashSet<&(String, String)> = HashSet::new();
        for pair in &self.defines {
            set.insert(pair);
        }

        let mut sorted: Vec<_> = set.iter().collect();
        sorted.sort_by(|a, b| a.cmp(b));

        for pair in sorted {
            pair.hash(state);
        }
    }
}

impl PartialEq for ShaderDesc {
    fn eq(&self, other: &Self) -> bool {
        let set_self: HashSet<_> = self.defines.iter().collect();
        let set_other: HashSet<_> = other.defines.iter().collect();

        self.ty == other.ty
            && self.path == other.path
            && self.entry_point == other.entry_point
            && self.debug == other.debug
            && set_self == set_other
    }
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
    pub shaders: &'a [CompiledShader],
}
