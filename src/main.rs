use ra::{
    context::ContextDual,
    resources::RenderResourceContext,
    system::{RenderBackend, RenderBackendSettings, RenderSystem},
};
use rhi::{
    backend::{Api, DebugFlags},
    resources::{BufferUsages, TextureUsages},
};
use tracing_subscriber::layer::SubscriberExt;

pub mod collections;
pub mod ra;
pub mod rhi;

fn main() {
    let console_log = tracing_subscriber::fmt::Layer::new()
        .with_ansi(true)
        .with_writer(std::io::stdout);
    let subscriber = tracing_subscriber::registry().with(console_log);
    let _ = tracing::subscriber::set_global_default(subscriber);

    let rs = RenderSystem::new(&[RenderBackendSettings {
        api: RenderBackend::Dx12,
        debug: DebugFlags::all(),
    }]);

    let backend = rs.dx_backend().expect("failed to get directx backend");

    let primary = backend.create_device(0);
    let secondary = backend.create_device(1);

    let group = ContextDual::new(primary, secondary);

    let buffer = rs.create_buffer_handle();
    let texture = rs.create_texture_handle();

    group.call(|ctx| {
        ctx.bind_buffer(
            buffer,
            rhi::resources::BufferDesc::cpu_to_gpu(128, BufferUsages::Uniform),
            None,
        );

        ctx.unbind_buffer(buffer);

        ctx.bind_texture(
            texture,
            rhi::resources::TextureDesc::new_2d(
                [800, 600],
                rhi::types::Format::R32,
                TextureUsages::RenderTarget | TextureUsages::Shared,
            ),
            None,
        );

        ctx.unbind_texture(texture);
    });
}
