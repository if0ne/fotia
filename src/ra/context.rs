use crate::{
    collections::handle::Handle,
    rhi::{
        command::{CommandType, RenderCommandDevice},
        resources::{BufferDesc, RenderResourceDevice, SamplerDesc, TextureDesc, TextureViewDesc},
    },
};

use super::resources::{Buffer, ResourceMapper, Sampler, Texture};

pub struct Context<D: RenderResourceDevice, CQ> {
    gpu: D,

    graphics_queue: CQ,
    compute_queue: CQ,
    transfer_queue: CQ,

    mapper: ResourceMapper<D>,
}

impl<D: RenderResourceDevice + RenderCommandDevice> Context<D, D::CommandQueue> {
    pub fn new(gpu: D) -> Self {
        let graphics_queue = gpu.create_command_queue(CommandType::Graphics, None);
        let compute_queue = gpu.create_command_queue(CommandType::Compute, None);
        let transfer_queue = gpu.create_command_queue(CommandType::Transfer, None);

        Self {
            gpu,
            graphics_queue,
            compute_queue,
            transfer_queue,
            mapper: ResourceMapper::default(),
        }
    }
}

pub trait RenderContext {
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

impl<D: RenderResourceDevice, CQ> RenderContext for Context<D, CQ> {
    fn bind_buffer(&self, handle: Handle<Buffer>, desc: BufferDesc, _init_data: Option<&[u8]>) {
        let buffer = self.gpu.create_buffer(desc);
        self.mapper.buffers.lock().set(handle, buffer);
    }

    fn unbind_buffer(&self, handle: Handle<Buffer>) {
        let Some(buffer) = self.mapper.buffers.lock().remove(handle) else {
            return;
        };

        self.gpu.destroy_buffer(buffer);
    }

    fn bind_texture(&self, handle: Handle<Texture>, desc: TextureDesc, _init_data: Option<&[u8]>) {
        let texture = self.gpu.create_texture(desc);
        self.mapper.textures.lock().set(handle, texture);
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
        guard.set(handle, texture);
    }

    fn open_texture_handle(&self, handle: Handle<Texture>, other: &Self) {
        let mut guard = self.mapper.textures.lock();
        let Some(texture) = guard.get(handle) else {
            panic!("texture doesn't exist")
        };

        let texture = self.gpu.open_texture(texture, &other.gpu);
        guard.set(handle, texture);
    }

    fn bind_sampler(&self, handle: Handle<Sampler>, desc: SamplerDesc) {
        let sampler = self.gpu.create_sampler(desc);
        self.mapper.sampler.lock().set(handle, sampler);
    }

    fn unbind_sampler(&self, handle: Handle<Sampler>) {
        let Some(sampler) = self.mapper.sampler.lock().remove(handle) else {
            return;
        };

        self.gpu.destroy_sampler(sampler);
    }
}
