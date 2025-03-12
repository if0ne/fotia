use crate::collections::handle::Handle;

use super::{
    container::HandleContainer,
    resources::{Buffer, Sampler, Texture},
};

#[derive(Debug)]
pub struct RenderSystem {
    pub(super) handles: HandleContainer,
}

impl RenderSystem {
    pub fn new(backend_settings: &[RenderBackendSettings]) -> Self {
        Self {
            handles: HandleContainer::new(),
        }
    }

    #[inline]
    pub fn create_buffer_handle(&self) -> Handle<Buffer> {
        self.handles.create_buffer_handle()
    }

    #[inline]
    pub fn free_buffer_handle(&self, handle: Handle<Buffer>) {
        self.handles.free_buffer_handle(handle)
    }

    #[inline]
    pub fn create_texture_handle(&self) -> Handle<Texture> {
        self.handles.create_texture_handle()
    }

    #[inline]
    pub fn free_texture_handle(&self, handle: Handle<Texture>) {
        self.handles.free_texture_handle(handle)
    }

    #[inline]
    pub fn create_sampler_handle(&self) -> Handle<Sampler> {
        self.handles.create_sampler_handle()
    }

    #[inline]
    pub fn free_sampler_handle(&self, handle: Handle<Sampler>) {
        self.handles.free_sampler_handle(handle)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderBackendSettings {
    pub api: RenderBackend,
    pub debug: DebugFlags,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderBackend {
    Dx12,
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct DebugFlags: u32 {
        const CpuValidation = 0x1;
        const GpuValidation = 0x2;
        const RenderDoc = 0x4;
        const Pix = 0x8;
    }
}
