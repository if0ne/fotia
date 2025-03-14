use std::path::PathBuf;

use ra::{
    context::ContextDual,
    resources::RenderResourceContext,
    shader::RenderShaderContext,
    system::{RenderBackend, RenderBackendSettings, RenderSystem},
};
use rhi::{
    backend::{Api, DebugFlags},
    resources::TextureUsages,
    shader::BindingSet,
    types::{CullMode, Format, InputElementDesc, ShaderType, VertexAttribute, VertexType},
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

    let layout = rs.create_pipeline_layout_handle();
    let pipeline = rs.create_raster_pipeline_handle();

    let vs = backend.compile_shader(rhi::shader::ShaderDesc {
        ty: ShaderType::Vertex,
        path: PathBuf::from("../assets/test.hlsl"),
        entry_point: "Main".to_string(),
        debug: true,
        defines: vec![],
    });

    group.call_primary(|ctx| {
        ctx.bind_pipeline_layout(
            layout,
            rhi::shader::PipelineLayoutDesc {
                sets: &[BindingSet {
                    entries: &[],
                    use_dynamic_buffer: true,
                }],
                static_samplers: &[],
            },
        );

        ctx.bind_raster_pipeline(
            pipeline,
            ra::shader::RasterPipelineDesc {
                layout: Some(layout),
                input_elements: &[InputElementDesc {
                    semantic: VertexAttribute::Position(0),
                    format: VertexType::Float3,
                    slot: 0,
                }],
                depth_bias: 0,
                slope_bias: 0.0,
                depth_clip: false,
                depth: None,
                render_targets: &[Format::Rgba8],
                cull_mode: CullMode::None,
                vs: &vs,
                shaders: &[],
            },
        );

        ctx.unbind_pipeline_layout(layout);
        ctx.unbind_raster_pipeline(pipeline);
    });
}
