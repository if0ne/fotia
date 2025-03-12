use parking_lot::Mutex;

use crate::collections::handle::{Handle, HandleAllocator};

use super::resources::{Buffer, Sampler, Texture};

#[derive(Debug)]
pub struct HandleContainer {
    pub(super) buffers: Mutex<HandleAllocator<Buffer>>,
    pub(super) textures: Mutex<HandleAllocator<Texture>>,
    pub(super) sampler: Mutex<HandleAllocator<Sampler>>,
}

impl HandleContainer {
    pub(super) fn new() -> Self {
        Self {
            buffers: Mutex::new(HandleAllocator::new()),
            textures: Mutex::new(HandleAllocator::new()),
            sampler: Mutex::new(HandleAllocator::new()),
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
        self.sampler.lock().allocate()
    }

    #[inline]
    pub(super) fn free_sampler_handle(&self, handle: Handle<Sampler>) {
        self.sampler.lock().free(handle);
    }
}
