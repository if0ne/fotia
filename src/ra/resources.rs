use parking_lot::Mutex;

use crate::{
    collections::{handle::Handle, sparse_array::SparseArray},
    rhi::{
        command::{IoCommandBuffer, RenderCommandDevice, RenderCommandQueue},
        resources::{BufferDesc, RenderResourceDevice, SamplerDesc, TextureDesc, TextureViewDesc},
    },
};

use super::context::Context;

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

impl<D: RenderResourceDevice + RenderCommandDevice> RenderResourceContext for Context<D> {
    fn bind_buffer(&self, handle: Handle<Buffer>, desc: BufferDesc, init_data: Option<&[u8]>) {
        let buffer = self.gpu.create_buffer(desc);

        if let Some(init_data) = init_data {
            let cmd = self.uploader.create_command_buffer();
            cmd.load_to_buffer(&buffer, init_data);
            self.uploader.commit(cmd);
            self.uploader.wait_on_cpu(self.uploader.submit());
        }

        self.mapper.buffers.lock().set(handle, buffer);
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
            let cmd = self.uploader.create_command_buffer();
            cmd.load_to_texture(&texture, init_data);
            self.uploader.commit(cmd);
            self.uploader.wait_on_cpu(self.uploader.submit());
        }

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

pub(super) struct ResourceMapper<D: RenderResourceDevice> {
    pub(super) buffers: Mutex<SparseArray<Buffer, D::Buffer>>,
    pub(super) textures: Mutex<SparseArray<Texture, D::Texture>>,
    pub(super) sampler: Mutex<SparseArray<Sampler, D::Sampler>>,
}

impl<D: RenderResourceDevice> Default for ResourceMapper<D> {
    fn default() -> Self {
        Self {
            buffers: Mutex::new(SparseArray::new(128)),
            textures: Mutex::new(SparseArray::new(128)),
            sampler: Mutex::new(SparseArray::new(128)),
        }
    }
}
