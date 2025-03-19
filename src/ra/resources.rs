use parking_lot::Mutex;

use crate::{
    collections::{handle::Handle, sparse_array::SparseArray},
    rhi::{
        command::{IoCommandBuffer, RenderCommandQueue},
        resources::{BufferDesc, SamplerDesc, TextureDesc, TextureViewDesc},
    },
};

use super::{
    context::{Context, RenderDevice},
    shader::{PipelineLayout, RasterPipeline, ShaderArgument},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Buffer;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Texture;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sampler;

pub trait RenderResourceContext {
    // Resources
    fn bind_buffer(&self, handle: Handle<Buffer>, desc: BufferDesc, init_data: Option<&[u8]>);
    fn unbind_buffer(&self, handle: Handle<Buffer>);

    fn bind_texture(&self, handle: Handle<Texture>, desc: TextureDesc, init_data: Option<&[u8]>);
    fn unbind_texture(&self, handle: Handle<Texture>);

    fn bind_texture_view(
        &self,
        handle: Handle<Texture>,
        texture: Handle<Texture>,
        desc: TextureViewDesc,
    );

    fn open_texture_handle(&self, handle: Handle<Texture>, other: &Self);

    fn bind_sampler(&self, handle: Handle<Sampler>, desc: SamplerDesc);
    fn unbind_sampler(&self, handle: Handle<Sampler>);
}

impl<D: RenderDevice> RenderResourceContext for Context<D> {
    fn bind_buffer(&self, handle: Handle<Buffer>, desc: BufferDesc, init_data: Option<&[u8]>) {
        let buffer = self.gpu.create_buffer(desc);

        if let Some(init_data) = init_data {
            let mut cmd = self.uploader.create_command_buffer(&self.gpu);
            cmd.load_to_buffer(&self.gpu, &buffer, init_data);
            self.uploader.commit(cmd);
            self.uploader.wait_on_cpu(self.uploader.submit(&self.gpu));
        }

        if let Some(buffer) = self.mapper.buffers.lock().set(handle, buffer) {
            self.gpu.destroy_buffer(buffer);
        }
    }

    fn unbind_buffer(&self, handle: Handle<Buffer>) {
        let Some(buffer) = self.mapper.buffers.lock().remove(handle) else {
            return;
        };

        self.gpu.destroy_buffer(buffer);
    }

    fn bind_texture(&self, handle: Handle<Texture>, desc: TextureDesc, init_data: Option<&[u8]>) {
        let texture = self.gpu.create_texture(desc);

        if let Some(init_data) = init_data {
            let mut cmd = self.uploader.create_command_buffer(&self.gpu);
            cmd.load_to_texture(&self.gpu, &texture, init_data);
            self.uploader.commit(cmd);
            self.uploader.wait_on_cpu(self.uploader.submit(&self.gpu));
        }

        if let Some(texture) = self.mapper.textures.lock().set(handle, texture) {
            self.gpu.destroy_texture(texture);
        }
    }

    fn unbind_texture(&self, handle: Handle<Texture>) {
        let Some(texture) = self.mapper.textures.lock().remove(handle) else {
            return;
        };

        self.gpu.destroy_texture(texture);
    }

    fn bind_texture_view(
        &self,
        handle: Handle<Texture>,
        texture: Handle<Texture>,
        desc: TextureViewDesc,
    ) {
        let mut guard = self.mapper.textures.lock();
        let Some(texture) = guard.get(texture) else {
            panic!("texture doesn't exist")
        };

        let texture = self.gpu.create_texture_view(texture, desc);
        if let Some(view) = guard.set(handle, texture) {
            self.gpu.destroy_texture(view);
        }
    }

    fn open_texture_handle(&self, handle: Handle<Texture>, other: &Self) {
        let mut guard = other.mapper.textures.lock();
        let Some(texture) = guard.get(handle) else {
            panic!("texture doesn't exist")
        };

        let texture = self.gpu.open_texture(texture, &other.gpu);
        guard.set(handle, texture);
    }

    fn bind_sampler(&self, handle: Handle<Sampler>, desc: SamplerDesc) {
        let sampler = self.gpu.create_sampler(desc);

        if let Some(sampler) = self.mapper.samplers.lock().set(handle, sampler) {
            self.gpu.destroy_sampler(sampler);
        }
    }

    fn unbind_sampler(&self, handle: Handle<Sampler>) {
        let Some(sampler) = self.mapper.samplers.lock().remove(handle) else {
            return;
        };

        self.gpu.destroy_sampler(sampler);
    }
}

pub(super) struct ResourceMapper<D: RenderDevice> {
    pub(super) buffers: Mutex<SparseArray<Buffer, D::Buffer>>,
    pub(super) textures: Mutex<SparseArray<Texture, D::Texture>>,
    pub(super) samplers: Mutex<SparseArray<Sampler, D::Sampler>>,

    pub(super) pipeline_layouts: Mutex<SparseArray<PipelineLayout, D::PipelineLayout>>,
    pub(super) shader_arguments: Mutex<SparseArray<ShaderArgument, D::ShaderArgument>>,
    pub(super) raster_pipelines: Mutex<SparseArray<RasterPipeline, D::RasterPipeline>>,
}

impl<D: RenderDevice> Default for ResourceMapper<D> {
    fn default() -> Self {
        Self {
            buffers: Mutex::new(SparseArray::new(128)),
            textures: Mutex::new(SparseArray::new(128)),
            samplers: Mutex::new(SparseArray::new(128)),
            pipeline_layouts: Mutex::new(SparseArray::new(128)),
            shader_arguments: Mutex::new(SparseArray::new(1024)),
            raster_pipelines: Mutex::new(SparseArray::new(128)),
        }
    }
}
