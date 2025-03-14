use crate::{
    collections::handle::Handle,
    rhi::{
        command::RenderCommandDevice,
        resources::RenderResourceDevice,
        shader::{CompiledShader, PipelineLayoutDesc, RenderShaderDevice},
        types::{CullMode, DepthStateDesc, Format, InputElementDesc},
    },
};

use super::context::Context;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PipelineLayout;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RasterPipeline;

pub trait RenderShaderContext {
    fn bind_pipeline_layout(&self, handle: Handle<PipelineLayout>, desc: PipelineLayoutDesc<'_>);
    fn unbind_pipeline_layout(&self, handle: Handle<PipelineLayout>);

    fn bind_raster_pipeline(&self, handle: Handle<RasterPipeline>, desc: RasterPipelineDesc<'_>);
    fn unbind_raster_pipeline(&self, handle: Handle<RasterPipeline>);
}

impl<D: RenderResourceDevice + RenderShaderDevice + RenderCommandDevice> RenderShaderContext
    for Context<D>
{
    fn bind_pipeline_layout(&self, handle: Handle<PipelineLayout>, desc: PipelineLayoutDesc<'_>) {
        let layout = self.gpu.create_pipeline_layout(desc);

        if let Some(layout) = self.mapper.pipeline_layouts.lock().set(handle, layout) {
            self.gpu.destroy_pipeline_layout(layout);
        }
    }

    fn unbind_pipeline_layout(&self, handle: Handle<PipelineLayout>) {
        let Some(layout) = self.mapper.pipeline_layouts.lock().remove(handle) else {
            return;
        };

        self.gpu.destroy_pipeline_layout(layout);
    }

    fn bind_raster_pipeline(&self, handle: Handle<RasterPipeline>, desc: RasterPipelineDesc<'_>) {
        let guard = self.mapper.pipeline_layouts.lock();
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

        if let Some(pipeline) = self.mapper.raster_pipelines.lock().set(handle, pipeline) {
            self.gpu.destroy_raster_pipeline(pipeline);
        }
    }

    fn unbind_raster_pipeline(&self, handle: Handle<RasterPipeline>) {
        let Some(pipeline) = self.mapper.raster_pipelines.lock().remove(handle) else {
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
    pub shaders: &'a [CompiledShader],
}
