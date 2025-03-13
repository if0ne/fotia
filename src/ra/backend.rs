use crate::rhi::{
    backend::{Api, RenderDeviceId, RenderDeviceInfo},
    command::RenderCommandDevice,
    resources::RenderResourceDevice,
    shader::{CompiledShader, ShaderDesc},
};

use super::context::Context;

pub struct Backend<A: Api> {
    api: A,
}

impl<A: Api<Device: RenderCommandDevice + RenderResourceDevice>> Api for Backend<A> {
    type Device = Context<A::Device>;

    fn enumerate_devices(&self) -> impl Iterator<Item = &RenderDeviceInfo> + '_ {
        self.api.enumerate_devices()
    }

    fn create_device(&self, index: RenderDeviceId) -> Self::Device {
        let gpu = self.api.create_device(index);
        Context::new(gpu)
    }

    fn compile_shader(&self, desc: ShaderDesc) -> CompiledShader {
        self.api.compile_shader(desc)
    }
}
