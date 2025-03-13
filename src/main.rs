use ra::system::{RenderBackend, RenderBackendSettings, RenderSystem};
use rhi::backend::{Api, DebugFlags};

pub mod collections;
pub mod ra;
pub mod rhi;

fn main() {
    let rs = RenderSystem::new(&[RenderBackendSettings {
        api: RenderBackend::Dx12,
        debug: DebugFlags::all(),
    }]);

    let backend = rs.dx_backend().expect("failed to get directx backend");

    for adapter in backend.enumerate_devices() {
        println!("Adapter: {:?}", adapter);
    }
}
