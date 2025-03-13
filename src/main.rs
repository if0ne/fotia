use ra::{
    context::ContextDual,
    resources::RenderResourceContext,
    system::{RenderBackend, RenderBackendSettings, RenderSystem},
};
use rhi::{
    backend::{Api, DebugFlags},
    resources::TextureUsages,
    types::Format,
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
    let secondary = backend.create_device(2);

    let group = ContextDual::new(primary, secondary);

    let image = image::open("../assets/texture.jpg")
        .expect("failed to load image")
        .to_rgba8();
    let bytes = image.as_raw();

    let texture = rs.create_texture_handle();

    group.call(|ctx| {
        ctx.bind_texture(
            texture,
            rhi::resources::TextureDesc::new_2d(
                [image.width(), image.height()],
                Format::Rgba8,
                TextureUsages::Resource,
            ),
            Some(&bytes),
        );

        ctx.unbind_texture(texture);
    });
}
