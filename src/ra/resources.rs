use parking_lot::RwLock;

use crate::{
    collections::{handle::Handle, sparse_map::SparseMap},
    rhi::{
        command::{IoCommandBuffer, RenderCommandQueue},
        resources::{Buffer as _, BufferDesc, SamplerDesc, TextureDesc, TextureViewDesc},
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
    fn update_buffer<T: Clone>(&self, handle: Handle<Buffer>, offset: usize, data: &[T]);

    fn bind_texture(&self, handle: Handle<Texture>, desc: TextureDesc, init_data: Option<&[u8]>);
    fn unbind_texture(&self, handle: Handle<Texture>);

    fn bind_texture_view(
        &self,
        handle: Handle<Texture>,
        texture: Handle<Texture>,
        desc: TextureViewDesc,
    );

    fn open_texture_handle(
        &self,
        handle: Handle<Texture>,
        other: &Self,
        overrided_view: Option<TextureViewDesc>,
    );

    fn bind_sampler(&self, handle: Handle<Sampler>, desc: SamplerDesc);
    fn unbind_sampler(&self, handle: Handle<Sampler>);
}

impl<D: RenderDevice> RenderResourceContext for Context<D> {
    fn bind_buffer(&self, handle: Handle<Buffer>, desc: BufferDesc, init_data: Option<&[u8]>) {
        let mut buffer = self.gpu.create_buffer(desc);

        if let Some(init_data) = init_data {
            let mut cmd = self.uploader.create_command_buffer(&self.gpu);
            cmd.load_to_buffer(&self.gpu, &mut buffer, init_data);
            self.uploader.commit(cmd);
            self.uploader.wait_on_cpu(self.uploader.submit(&self.gpu));
        }

        if let Some(buffer) = self.mapper.buffers.write().set(handle, buffer) {
            self.gpu.destroy_buffer(buffer);
        }
    }

    fn unbind_buffer(&self, handle: Handle<Buffer>) {
        let Some(buffer) = self.mapper.buffers.write().remove(handle) else {
            return;
        };

        self.gpu.destroy_buffer(buffer);
    }

    fn update_buffer<T: Clone>(&self, handle: Handle<Buffer>, offset: usize, data: &[T]) {
        let mut guard = self.mapper.buffers.write();
        let Some(buffer) = guard.get_mut(handle) else {
            panic!("buffer doesn't exist")
        };

        let buffer = &mut buffer.map_mut()[offset..(offset + data.len())];
        buffer.clone_from_slice(data);
    }

    fn bind_texture(&self, handle: Handle<Texture>, desc: TextureDesc, init_data: Option<&[u8]>) {
        let texture = self.gpu.create_texture(desc);

        if let Some(init_data) = init_data {
            let mut cmd = self.uploader.create_command_buffer(&self.gpu);
            cmd.load_to_texture(&self.gpu, &texture, init_data);
            self.uploader.commit(cmd);
            self.uploader.wait_on_cpu(self.uploader.submit(&self.gpu));
        }

        if let Some(texture) = self.mapper.textures.write().set(handle, texture) {
            self.gpu.destroy_texture(texture);
        }
    }

    fn unbind_texture(&self, handle: Handle<Texture>) {
        let Some(texture) = self.mapper.textures.write().remove(handle) else {
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
        let mut guard = self.mapper.textures.write();
        let Some(texture) = guard.get(texture) else {
            panic!("texture doesn't exist")
        };

        let texture = self.gpu.create_texture_view(texture, desc);
        if let Some(view) = guard.set(handle, texture) {
            self.gpu.destroy_texture(view);
        }
    }

    fn open_texture_handle(
        &self,
        handle: Handle<Texture>,
        other: &Self,
        overrided_view: Option<TextureViewDesc>,
    ) {
        let guard = other.mapper.textures.write();
        let Some(texture) = guard.get(handle) else {
            panic!("texture doesn't exist")
        };

        let texture = self.gpu.open_texture(texture, &other.gpu, overrided_view);
        let mut self_guard = self.mapper.textures.write();
        if let Some(texture) = self_guard.set(handle, texture) {
            self.gpu.destroy_texture(texture);
        }
    }

    fn bind_sampler(&self, handle: Handle<Sampler>, desc: SamplerDesc) {
        let sampler = self.gpu.create_sampler(desc);

        if let Some(sampler) = self.mapper.samplers.write().set(handle, sampler) {
            self.gpu.destroy_sampler(sampler);
        }
    }

    fn unbind_sampler(&self, handle: Handle<Sampler>) {
        let Some(sampler) = self.mapper.samplers.write().remove(handle) else {
            return;
        };

        self.gpu.destroy_sampler(sampler);
    }
}

pub(super) struct ResourceMapper<D: RenderDevice> {
    pub(super) buffers: RwLock<SparseMap<Buffer, D::Buffer>>,
    pub(super) textures: RwLock<SparseMap<Texture, D::Texture>>,
    pub(super) samplers: RwLock<SparseMap<Sampler, D::Sampler>>,

    pub(super) pipeline_layouts: RwLock<SparseMap<PipelineLayout, D::PipelineLayout>>,
    pub(super) shader_arguments: RwLock<SparseMap<ShaderArgument, D::ShaderArgument>>,
    pub(super) raster_pipelines: RwLock<SparseMap<RasterPipeline, D::RasterPipeline>>,
}

impl<D: RenderDevice> Default for ResourceMapper<D> {
    fn default() -> Self {
        Self {
            buffers: RwLock::new(SparseMap::new(128)),
            textures: RwLock::new(SparseMap::new(128)),
            samplers: RwLock::new(SparseMap::new(128)),
            pipeline_layouts: RwLock::new(SparseMap::new(128)),
            shader_arguments: RwLock::new(SparseMap::new(1024)),
            raster_pipelines: RwLock::new(SparseMap::new(128)),
        }
    }
}
