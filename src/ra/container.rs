use parking_lot::Mutex;

use crate::collections::handle::{Handle, HandleAllocator};

use super::{
    resources::{Buffer, Sampler, Texture},
    shader::{PipelineLayout, RasterPipeline, ShaderArgument},
};

#[derive(Debug)]
pub struct HandleContainer {
    pub(super) buffers: Mutex<HandleAllocator<Buffer>>,
    pub(super) textures: Mutex<HandleAllocator<Texture>>,
    pub(super) samplers: Mutex<HandleAllocator<Sampler>>,

    pub(super) pipeline_layouts: Mutex<HandleAllocator<PipelineLayout>>,
    pub(super) shader_arguments: Mutex<HandleAllocator<ShaderArgument>>,
    pub(super) raster_pipeline: Mutex<HandleAllocator<RasterPipeline>>,
}

impl HandleContainer {
    pub(super) fn new() -> Self {
        Self {
            buffers: Mutex::new(HandleAllocator::new()),
            textures: Mutex::new(HandleAllocator::new()),
            samplers: Mutex::new(HandleAllocator::new()),
            pipeline_layouts: Mutex::new(HandleAllocator::new()),
            shader_arguments: Mutex::new(HandleAllocator::new()),
            raster_pipeline: Mutex::new(HandleAllocator::new()),
        }
    }

    #[inline]
    pub(super) fn create_buffer_handle(&self) -> Handle<Buffer> {
        self.buffers.lock().allocate()
    }

    #[inline]
    pub(super) fn free_buffer_handle(&self, handle: Handle<Buffer>) {
        self.buffers.lock().free(handle);
    }

    #[inline]
    pub(super) fn create_texture_handle(&self) -> Handle<Texture> {
        self.textures.lock().allocate()
    }

    #[inline]
    pub(super) fn free_texture_handle(&self, handle: Handle<Texture>) {
        self.textures.lock().free(handle);
    }

    #[inline]
    pub(super) fn create_sampler_handle(&self) -> Handle<Sampler> {
        self.samplers.lock().allocate()
    }

    #[inline]
    pub(super) fn free_sampler_handle(&self, handle: Handle<Sampler>) {
        self.samplers.lock().free(handle);
    }

    #[inline]
    pub(super) fn create_pipeline_layout_handle(&self) -> Handle<PipelineLayout> {
        self.pipeline_layouts.lock().allocate()
    }

    #[inline]
    pub(super) fn free_pipeline_layout_handle(&self, handle: Handle<PipelineLayout>) {
        self.pipeline_layouts.lock().free(handle);
    }

    #[inline]
    pub(super) fn create_shader_argument_handle(&self) -> Handle<ShaderArgument> {
        self.shader_arguments.lock().allocate()
    }

    #[inline]
    pub(super) fn free_shader_argument_handle(&self, handle: Handle<ShaderArgument>) {
        self.shader_arguments.lock().free(handle);
    }

    #[inline]
    pub(super) fn create_raster_pipeline_handle(&self) -> Handle<RasterPipeline> {
        self.raster_pipeline.lock().allocate()
    }

    #[inline]
    pub(super) fn free_raster_pipeline_handle(&self, handle: Handle<RasterPipeline>) {
        self.raster_pipeline.lock().free(handle);
    }
}
