use std::path::Path;

use super::shader::{CompiledShader, ShaderDesc};

pub type RenderDeviceId = usize;

pub trait Api {
    type Device;

    fn enumerate_devices(&self) -> impl Iterator<Item = &RenderDeviceInfo> + '_;
    fn create_device(&self, index: RenderDeviceId) -> Self::Device;

    fn compile_shader<P: AsRef<Path>>(&self, desc: &ShaderDesc<'_, P>) -> CompiledShader;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceType {
    Discrete,
    Integrated,
    Cpu,
}

#[derive(Clone, Debug)]
pub struct RenderDeviceInfo {
    pub name: String,
    pub id: RenderDeviceId,
    pub is_cross_adapter_texture_supported: bool,
    pub is_uma: bool,
    pub ty: DeviceType,
    pub copy_timestamp_support: bool,
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
