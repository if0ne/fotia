use std::path::Path;

use crate::rhi::{
    backend::{Api, RenderDeviceId, RenderDeviceInfo},
    shader::{CompiledShader, ShaderDesc},
};

use super::context::{Context, RenderDevice};

#[derive(Debug)]
pub struct Backend<A: Api> {
    api: A,
}

impl<A: Api> Backend<A> {
    pub fn new(api: A) -> Self {
        Self { api }
    }
}

impl<A: Api<Device: RenderDevice>> Api for Backend<A> {
    type Device = Context<A::Device>;

    fn enumerate_devices(&self) -> impl Iterator<Item = &RenderDeviceInfo> + '_ {
        self.api.enumerate_devices()
    }

    fn create_device(&self, index: RenderDeviceId) -> Self::Device {
        let gpu = self.api.create_device(index);
        Context::new(gpu)
    }

    fn compile_shader<P: AsRef<Path>>(&self, desc: &ShaderDesc<'_, P>) -> CompiledShader {
        self.api.compile_shader(desc)
    }
}
