use crate::{
    collections::handle::Handle,
    rhi::{
        command::RenderCommandDevice,
        resources::RenderResourceDevice,
        shader::{PipelineLayoutDesc, RenderShaderDevice},
    },
};

use super::context::Context;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PipelineLayout;

pub trait RenderShaderContext {
    fn bind_pipeline_layout(&self, handle: Handle<PipelineLayout>, desc: PipelineLayoutDesc<'_>);
    fn unbind_pipeline_layout(&self, handle: Handle<PipelineLayout>);
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
}
