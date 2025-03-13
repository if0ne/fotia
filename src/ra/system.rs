use std::sync::Arc;

use crate::{
    collections::handle::Handle,
    rhi::{backend::DebugFlags, dx12::backend::DxBackend},
};

use super::{
    backend::Backend,
    container::HandleContainer,
    resources::{Buffer, Sampler, Texture},
};

#[derive(Debug)]
pub struct RenderSystem {
    pub(super) handles: HandleContainer,

    pub(super) dx_backend: Option<Arc<Backend<DxBackend>>>,
}

impl RenderSystem {
    pub fn new(backend_settings: &[RenderBackendSettings]) -> Self {
        let dx_backend = backend_settings
            .iter()
            .find(|b| b.api == RenderBackend::Dx12)
            .and_then(|settings| Some(Arc::new(Backend::new(DxBackend::new(settings.debug)))));

        Self {
            dx_backend,
            handles: HandleContainer::new(),
        }
    }

    #[inline]
    pub fn dx_backend(&self) -> Option<Arc<Backend<DxBackend>>> {
        self.dx_backend.clone()
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
