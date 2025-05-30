use crate::{
    collections::handle::Handle,
    rhi::{
        self,
        shader::{CompiledShader, PipelineLayoutDesc},
        types::{CullMode, DepthStateDesc, Format, InputElementDesc},
    },
};

use super::{
    context::{Context, RenderDevice},
    resources::{Buffer, Sampler, Texture},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PipelineLayout;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ShaderArgument;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RasterPipeline;

pub trait RenderShaderContext {
    fn bind_pipeline_layout(&self, handle: Handle<PipelineLayout>, desc: PipelineLayoutDesc<'_>);
    fn unbind_pipeline_layout(&self, handle: Handle<PipelineLayout>);

    fn bind_shader_argument(&self, handle: Handle<ShaderArgument>, desc: ShaderArgumentDesc<'_>);
    fn unbind_shader_argument(&self, handle: Handle<ShaderArgument>);

    fn bind_raster_pipeline(&self, handle: Handle<RasterPipeline>, desc: RasterPipelineDesc<'_>);
    fn unbind_raster_pipeline(&self, handle: Handle<RasterPipeline>);
}

impl<D: RenderDevice> RenderShaderContext for Context<D> {
    fn bind_pipeline_layout(&self, handle: Handle<PipelineLayout>, desc: PipelineLayoutDesc<'_>) {
        let layout = self.gpu.create_pipeline_layout(desc);

        if let Some(layout) = self.mapper.pipeline_layouts.write().set(handle, layout) {
            self.gpu.destroy_pipeline_layout(layout);
        }
    }

    fn unbind_pipeline_layout(&self, handle: Handle<PipelineLayout>) {
        let Some(layout) = self.mapper.pipeline_layouts.write().remove(handle) else {
            return;
        };

        self.gpu.destroy_pipeline_layout(layout);
    }

    fn bind_shader_argument(&self, handle: Handle<ShaderArgument>, desc: ShaderArgumentDesc<'_>) {
        let buffers = self.mapper.buffers.write();
        let textures = self.mapper.textures.write();
        let samplers = self.mapper.samplers.write();

        let views = desc.views.iter().map(|e| match e {
            ShaderEntry::Cbv(handle, size) => rhi::shader::ShaderEntry::Cbv(
                buffers.get(*handle).expect("failed to get buffer"),
                *size,
            ),
            ShaderEntry::Srv(handle) => {
                rhi::shader::ShaderEntry::Srv(textures.get(*handle).expect("failed to get texture"))
            }
            ShaderEntry::Uav(handle) => {
                rhi::shader::ShaderEntry::Uav(textures.get(*handle).expect("failed to get texture"))
            }
        });

        let samplers = desc
            .samplers
            .iter()
            .map(|s| samplers.get(*s).expect("failed to get sampler"));

        let dynamic_buffer = desc
            .dynamic_buffer
            .map(|b| buffers.get(b).expect("failed to get buffer"));

        let desc = rhi::shader::ShaderArgumentDesc {
            views,
            samplers,
            dynamic_buffer,
        };

        let argument = self.gpu.create_shader_argument(desc);

        if let Some(argument) = self.mapper.shader_arguments.write().set(handle, argument) {
            self.gpu.destroy_shader_argument(argument);
        }
    }

    fn unbind_shader_argument(&self, handle: Handle<ShaderArgument>) {
        let Some(argument) = self.mapper.shader_arguments.write().remove(handle) else {
            return;
        };

        self.gpu.destroy_shader_argument(argument);
    }

    fn bind_raster_pipeline(&self, handle: Handle<RasterPipeline>, desc: RasterPipelineDesc<'_>) {
        let guard = self.mapper.pipeline_layouts.write();
        let layout = desc
            .layout
            .map(|h| guard.get(h).expect("failed to get pipeline layout"));

        let desc = crate::rhi::shader::RasterPipelineDesc {
            layout,
            input_elements: desc.input_elements,
            depth_bias: desc.depth_bias,
            slope_bias: desc.slope_bias,
            depth_clip: desc.depth_clip,
            depth: desc.depth,
            render_targets: desc.render_targets,
            cull_mode: desc.cull_mode,
            vs: desc.vs,
            shaders: desc.shaders,
        };

        let pipeline = self.gpu.create_raster_pipeline(desc);

        if let Some(pipeline) = self.mapper.raster_pipelines.write().set(handle, pipeline) {
            self.gpu.destroy_raster_pipeline(pipeline);
        }
    }

    fn unbind_raster_pipeline(&self, handle: Handle<RasterPipeline>) {
        let Some(pipeline) = self.mapper.raster_pipelines.write().remove(handle) else {
            return;
        };

        self.gpu.destroy_raster_pipeline(pipeline);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RasterPipelineDesc<'a> {
    pub layout: Option<Handle<PipelineLayout>>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShaderEntry {
    Cbv(Handle<Buffer>, usize),
    Srv(Handle<Texture>),
    Uav(Handle<Texture>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShaderArgumentDesc<'a> {
    pub views: &'a [ShaderEntry],
    pub samplers: &'a [Handle<Sampler>],
    pub dynamic_buffer: Option<Handle<Buffer>>,
}
