pub type RenderDeviceId = usize;

pub trait Api {
    type Device;

    fn enumerate_devices(&self) -> impl Iterator<Item = &RenderDeviceInfo> + '_;
    fn create_device(&self, index: RenderDeviceId) -> Self::Device;
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
}
