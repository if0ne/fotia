use std::{cell::RefCell, path::PathBuf, sync::Arc};

use collections::handle::Handle;
use ra::{
    backend::Backend,
    command::{Barrier, RenderCommandContext, RenderCommandEncoder, RenderEncoder},
    context::{ContextDual, RenderDevice},
    shader::{RasterPipeline, RasterPipelineDesc, RenderShaderContext},
    swapchain::{RenderSwapchainContext, Surface, Swapchain},
    system::{RenderBackend, RenderBackendSettings, RenderSystem},
};
use rhi::{
    backend::{Api, DebugFlags},
    command::CommandType,
    shader::{
        BindingEntry, BindingSet, BindingType, CompiledShader, PipelineLayoutDesc, SamplerType,
        ShaderDesc, StaticSampler,
    },
    swapchain::{PresentMode, SwapchainDesc},
    types::{
        AddressMode, ComparisonFunc, CullMode, DepthOp, DepthStateDesc, Filter, Format,
        InputElementDesc, ResourceState, ShaderType, VertexAttribute, VertexType,
    },
};
use timer::GameTimer;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub mod collections;
pub mod ra;
pub mod rhi;
pub mod timer;

fn main() {
    let console_log = tracing_subscriber::fmt::Layer::new()
        .with_ansi(true)
        .with_writer(std::io::stdout);
    let subscriber = tracing_subscriber::registry().with(console_log);
    let _ = tracing::subscriber::set_global_default(subscriber);

    let rs = Arc::new(RenderSystem::new(&[RenderBackendSettings {
        api: RenderBackend::Dx12,
        debug: if cfg!(debug_assertions) {
            DebugFlags::all()
        } else {
            DebugFlags::empty()
        },
    }]));

    let backend = rs.dx_backend().expect("failed to get directx backend");

    let shaders = ShaderCollection::new(&backend, cfg!(debug_assertions));

    let primary = backend.create_device(0);
    let secondary = backend.create_device(1);

    let group = Arc::new(ContextDual::new(primary, secondary));

    let psos = PsoCollection::new(Arc::clone(&rs), Arc::clone(&group), &shaders);

    let event_loop = winit::event_loop::EventLoop::new().expect("failed to create event loop");

    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

    let mut app = Application {
        timer: GameTimer::default(),

        rs,
        context: group,
        wnd_ctx: None,

        shaders,
        psos,
    };

    event_loop.run_app(&mut app).expect("failed to run app");

    app.context.call(|ctx| ctx.wait_idle());
}

pub struct WindowContext<D: RenderDevice> {
    pub window: winit::window::Window,
    pub wnd: RawWindowHandle,
    pub swapchain: Swapchain<D>,
}

pub struct Application<D: RenderDevice> {
    pub timer: GameTimer,

    pub wnd_ctx: Option<WindowContext<D>>,
    pub rs: Arc<RenderSystem>,
    pub context: Arc<ContextDual<D>>,

    pub shaders: ShaderCollection,
    pub psos: PsoCollection<D>,
}

impl<D: RenderDevice> winit::application::ApplicationHandler for Application<D> {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window_attributes = winit::window::Window::default_attributes()
            .with_title("Hello, Window!")
            .with_inner_size(winit::dpi::PhysicalSize::new(800, 600));

        let window = event_loop.create_window(window_attributes).unwrap();
        /*window
            .set_cursor_grab(winit::window::CursorGrabMode::Confined)
            .expect("failed to lock cursor");
        window.set_cursor_visible(false);*/

        let wnd = window
            .window_handle()
            .map(|h| h.as_raw())
            .expect("failed to get window handle");

        let swapchain = self.context.primary.create_swapchain(
            SwapchainDesc {
                width: 800,
                height: 600,
                present_mode: PresentMode::Mailbox,
                frames: 3,
            },
            &wnd,
            &self.rs.handles,
        );

        self.wnd_ctx = Some(WindowContext {
            window,
            wnd,
            swapchain,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            winit::event::WindowEvent::Focused(focused) => {
                if focused {
                    self.timer.start();
                } else {
                    self.timer.stop();
                }
            }
            winit::event::WindowEvent::KeyboardInput { event, .. } => match event.state {
                winit::event::ElementState::Pressed => {
                    if event.physical_key == winit::keyboard::KeyCode::Escape {
                        event_loop.exit();
                    }
                }
                winit::event::ElementState::Released => {}
            },
            winit::event::WindowEvent::MouseInput { state, .. } => match state {
                winit::event::ElementState::Pressed => {}
                winit::event::ElementState::Released => {}
            },
            winit::event::WindowEvent::Resized(size) => {
                if let Some(window) = self.wnd_ctx.as_mut() {
                    self.context.primary.wait_idle();
                    self.context.primary.resize(
                        &mut window.swapchain,
                        [size.width, size.height],
                        &self.rs.handles,
                    );
                }
            }
            winit::event::WindowEvent::RedrawRequested => {
                self.timer.tick();
                self.calculate_frame_stats();

                let Some(wnd) = &mut self.wnd_ctx else {
                    return;
                };

                let frame = wnd.swapchain.next_frame();
                self.context.call_primary(|ctx| {
                    ctx.wait_on_cpu(CommandType::Graphics, frame.last_access);
                    let mut encoder = ctx.create_encoder(CommandType::Graphics);
                    let timings = encoder.begin(ctx);
                    info!("Timings: {:?}", timings);

                    encoder.set_barriers(&[Barrier::Texture(
                        frame.texture,
                        ResourceState::RenderTarget,
                    )]);
                    {
                        let mut encoder = encoder.render("Clear framebuffer".into(), &[], None);
                        encoder.clear_rt(frame.texture, [0.5, 0.32, 0.16, 1.0]);
                    }

                    encoder
                        .set_barriers(&[Barrier::Texture(frame.texture, ResourceState::Present)]);

                    ctx.commit(encoder);
                    frame.last_access = ctx.submit(CommandType::Graphics);
                });

                wnd.swapchain.present();
            }
            winit::event::WindowEvent::CloseRequested => event_loop.exit(),
            _ => (),
        }
    }

    #[allow(clippy::single_match)]
    fn device_event(
        &mut self,
        _: &winit::event_loop::ActiveEventLoop,
        _: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        match event {
            winit::event::DeviceEvent::MouseMotion { .. } => {}
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &winit::event_loop::ActiveEventLoop) {
        if let Some(context) = self.wnd_ctx.as_ref() {
            context.window.request_redraw();
        }
    }
}

impl<D: RenderDevice> Application<D> {
    fn calculate_frame_stats(&self) {
        thread_local! {
            static FRAME_COUNT: RefCell<i32> = Default::default();
            static TIME_ELAPSED: RefCell<f32> = Default::default();
        }

        FRAME_COUNT.with_borrow_mut(|frame_cnt| {
            *frame_cnt += 1;
        });

        TIME_ELAPSED.with_borrow_mut(|time_elapsed| {
            if self.timer.total_time() - *time_elapsed > 1.0 {
                FRAME_COUNT.with_borrow_mut(|frame_count| {
                    let fps = *frame_count as f32;
                    let mspf = 1000.0 / fps;

                    if let Some(ref context) = self.wnd_ctx {
                        context
                            .window
                            .set_title(&format!("{} Fps: {fps} Ms: {mspf}", "Fotia"))
                    }

                    *frame_count = 0;
                    *time_elapsed += 1.0;
                });
            }
        })
    }
}

pub struct ShaderCollection {
    pub csm: CompiledShader,
    pub fullscreen: CompiledShader,
    pub directional_light_pass: CompiledShader,
    pub gamma_corr_pass: CompiledShader,
    pub zpass: CompiledShader,
    pub gpass_vs: CompiledShader,
    pub gpass_ps: CompiledShader,
}

impl ShaderCollection {
    pub fn new<A: Api<Device: RenderDevice>>(api: &Backend<A>, debug: bool) -> Self {
        let asset_path = PathBuf::from("../assets/shaders");

        let csm = api.compile_shader(&ShaderDesc {
            ty: ShaderType::Vertex,
            path: asset_path.join("Csm.hlsl"),
            entry_point: "Main".into(),
            debug,
            defines: vec![],
        });

        let fullscreen = api.compile_shader(&ShaderDesc {
            ty: ShaderType::Vertex,
            path: asset_path.join("FullscreenVS.hlsl"),
            entry_point: "Main".into(),
            debug,
            defines: vec![],
        });

        let directional_light_pass = api.compile_shader(&ShaderDesc {
            ty: ShaderType::Pixel,
            path: asset_path.join("DirectionalLight.hlsl"),
            entry_point: "Main".into(),
            debug,
            defines: vec![],
        });

        let gamma_corr_pass = api.compile_shader(&ShaderDesc {
            ty: ShaderType::Pixel,
            path: asset_path.join("GammaCorr.hlsl"),
            entry_point: "Main".into(),
            debug,
            defines: vec![],
        });

        let zpass = api.compile_shader(&ShaderDesc {
            ty: ShaderType::Vertex,
            path: asset_path.join("Zpass.hlsl"),
            entry_point: "Main".into(),
            debug,
            defines: vec![],
        });

        let gpass_vs = api.compile_shader(&ShaderDesc {
            ty: ShaderType::Vertex,
            path: asset_path.join("GPass.hlsl"),
            entry_point: "VSMain".into(),
            debug,
            defines: vec![],
        });

        let gpass_ps = api.compile_shader(&ShaderDesc {
            ty: ShaderType::Pixel,
            path: asset_path.join("GPass.hlsl"),
            entry_point: "PSMain".into(),
            debug,
            defines: vec![],
        });

        Self {
            csm,
            fullscreen,
            directional_light_pass,
            gamma_corr_pass,
            zpass,
            gpass_vs,
            gpass_ps,
        }
    }
}

pub struct PsoCollection<D: RenderDevice> {
    rs: Arc<RenderSystem>,
    group: Arc<ContextDual<D>>,
    pub zpass: Handle<RasterPipeline>,
    pub csm_pass: Handle<RasterPipeline>,
    pub directional_light_pass: Handle<RasterPipeline>,
    pub gamma_corr_pass: Handle<RasterPipeline>,
    pub g_pass: Handle<RasterPipeline>,
}

impl<D: RenderDevice> PsoCollection<D> {
    pub fn new(
        rs: Arc<RenderSystem>,
        group: Arc<ContextDual<D>>,
        shaders: &ShaderCollection,
    ) -> Self {
        let zpass = rs.create_raster_pipeline_handle();
        let csm_pass = rs.create_raster_pipeline_handle();
        let directional_light_pass = rs.create_raster_pipeline_handle();
        let gamma_corr_pass = rs.create_raster_pipeline_handle();
        let g_pass = rs.create_raster_pipeline_handle();

        group.parallel(|ctx| {
            // ZPass
            let zpass_layout = rs.create_pipeline_layout_handle();

            ctx.bind_pipeline_layout(
                zpass_layout,
                PipelineLayoutDesc {
                    sets: &[
                        BindingSet {
                            entries: &[],
                            use_dynamic_buffer: true,
                        },
                        BindingSet {
                            entries: &[],
                            use_dynamic_buffer: true,
                        },
                    ],
                    static_samplers: &[],
                },
            );

            ctx.bind_raster_pipeline(
                zpass,
                RasterPipelineDesc {
                    layout: Some(zpass_layout),
                    input_elements: &[InputElementDesc {
                        semantic: VertexAttribute::Position(0),
                        format: VertexType::Float3,
                    }],
                    depth_bias: 0,
                    slope_bias: 0.0,
                    depth_clip: true,
                    depth: Some(DepthStateDesc {
                        op: DepthOp::LessEqual,
                        format: Format::D24S8,
                        read_only: false,
                    }),
                    render_targets: &[],
                    cull_mode: CullMode::Back,
                    vs: &shaders.zpass,
                    shaders: &[],
                },
            );

            rs.free_pipeline_layout_handle(zpass_layout);

            // CSM Pass
            let csm_layout = rs.create_pipeline_layout_handle();

            ctx.bind_pipeline_layout(
                csm_layout,
                PipelineLayoutDesc {
                    sets: &[
                        BindingSet {
                            entries: &[],
                            use_dynamic_buffer: true,
                        },
                        BindingSet {
                            entries: &[],
                            use_dynamic_buffer: true,
                        },
                    ],
                    static_samplers: &[],
                },
            );

            ctx.bind_raster_pipeline(
                csm_pass,
                RasterPipelineDesc {
                    layout: Some(csm_layout),
                    input_elements: &[InputElementDesc {
                        semantic: VertexAttribute::Position(0),
                        format: VertexType::Float3,
                    }],
                    depth_bias: 10000,
                    slope_bias: 5.0,
                    depth_clip: false,
                    depth: Some(DepthStateDesc {
                        op: DepthOp::LessEqual,
                        format: Format::D32,
                        read_only: false,
                    }),
                    render_targets: &[],
                    cull_mode: CullMode::Back,
                    vs: &shaders.csm,
                    shaders: &[],
                },
            );

            rs.free_pipeline_layout_handle(csm_layout);

            // Directional Light Pass
            let directional_light_layout = rs.create_pipeline_layout_handle();

            ctx.bind_pipeline_layout(
                directional_light_layout,
                PipelineLayoutDesc {
                    sets: &[
                        BindingSet {
                            entries: &[
                                BindingEntry::new(BindingType::Srv, 1),
                                BindingEntry::new(BindingType::Srv, 1),
                                BindingEntry::new(BindingType::Srv, 1),
                                BindingEntry::new(BindingType::Srv, 1),
                            ],
                            use_dynamic_buffer: true,
                        },
                        BindingSet {
                            entries: &[],
                            use_dynamic_buffer: true,
                        },
                        BindingSet {
                            entries: &[],
                            use_dynamic_buffer: true,
                        },
                    ],
                    static_samplers: &[StaticSampler {
                        ty: SamplerType::Comparasion(ComparisonFunc::LessEqual),
                        address_mode: AddressMode::Clamp,
                    }],
                },
            );

            ctx.bind_raster_pipeline(
                directional_light_pass,
                RasterPipelineDesc {
                    layout: Some(directional_light_layout),
                    input_elements: &[
                        InputElementDesc {
                            semantic: VertexAttribute::Position(0),
                            format: VertexType::Float3,
                        },
                        InputElementDesc {
                            semantic: VertexAttribute::Uv(0),
                            format: VertexType::Float3,
                        },
                    ],
                    depth_bias: 0,
                    slope_bias: 0.0,
                    depth_clip: false,
                    depth: None,
                    render_targets: &[Format::Rgba32],
                    cull_mode: CullMode::None,
                    vs: &shaders.fullscreen,
                    shaders: &[&shaders.directional_light_pass],
                },
            );

            rs.free_pipeline_layout_handle(directional_light_layout);

            // Gamme Correction Pass
            let gamme_corr_layout = rs.create_pipeline_layout_handle();

            ctx.bind_pipeline_layout(
                gamme_corr_layout,
                PipelineLayoutDesc {
                    sets: &[BindingSet {
                        entries: &[BindingEntry::new(BindingType::Srv, 1)],
                        use_dynamic_buffer: true,
                    }],
                    static_samplers: &[StaticSampler {
                        ty: SamplerType::Sample(Filter::Linear),
                        address_mode: AddressMode::Clamp,
                    }],
                },
            );

            ctx.bind_raster_pipeline(
                gamma_corr_pass,
                RasterPipelineDesc {
                    layout: Some(gamme_corr_layout),
                    input_elements: &[
                        InputElementDesc {
                            semantic: VertexAttribute::Position(0),
                            format: VertexType::Float3,
                        },
                        InputElementDesc {
                            semantic: VertexAttribute::Uv(0),
                            format: VertexType::Float3,
                        },
                    ],
                    depth_bias: 0,
                    slope_bias: 0.0,
                    depth_clip: false,
                    depth: None,
                    render_targets: &[Format::Rgba8Unorm],
                    cull_mode: CullMode::None,
                    vs: &shaders.fullscreen,
                    shaders: &[&shaders.gamma_corr_pass],
                },
            );

            rs.free_pipeline_layout_handle(gamme_corr_layout);

            // G Pass
            let gpass_layout = rs.create_pipeline_layout_handle();

            ctx.bind_pipeline_layout(
                gpass_layout,
                PipelineLayoutDesc {
                    sets: &[
                        BindingSet {
                            entries: &[],
                            use_dynamic_buffer: true,
                        },
                        BindingSet {
                            entries: &[
                                BindingEntry::new(BindingType::Srv, 1),
                                BindingEntry::new(BindingType::Srv, 1),
                            ],
                            use_dynamic_buffer: true,
                        },
                        BindingSet {
                            entries: &[],
                            use_dynamic_buffer: true,
                        },
                    ],
                    static_samplers: &[StaticSampler {
                        ty: SamplerType::Sample(Filter::Linear),
                        address_mode: AddressMode::Clamp,
                    }],
                },
            );

            ctx.bind_raster_pipeline(
                g_pass,
                RasterPipelineDesc {
                    layout: Some(gpass_layout),
                    input_elements: &[
                        InputElementDesc {
                            semantic: VertexAttribute::Position(0),
                            format: VertexType::Float3,
                        },
                        InputElementDesc {
                            semantic: VertexAttribute::Normal(0),
                            format: VertexType::Float3,
                        },
                        InputElementDesc {
                            semantic: VertexAttribute::Uv(0),
                            format: VertexType::Float3,
                        },
                        InputElementDesc {
                            semantic: VertexAttribute::Tangent(0),
                            format: VertexType::Float4,
                        },
                    ],
                    depth_bias: 0,
                    slope_bias: 0.0,
                    depth_clip: true,
                    depth: Some(DepthStateDesc {
                        op: DepthOp::Equal,
                        format: Format::D24S8,
                        read_only: true,
                    }),
                    render_targets: &[Format::Rgba32, Format::Rgba32, Format::Rgba32],
                    cull_mode: CullMode::Back,
                    vs: &shaders.gpass_vs,
                    shaders: &[&shaders.gpass_ps],
                },
            );

            rs.free_pipeline_layout_handle(gpass_layout);
        });

        Self {
            rs,
            group,
            zpass,
            csm_pass,
            directional_light_pass,
            gamma_corr_pass,
            g_pass,
        }
    }
}

impl<D: RenderDevice> Drop for PsoCollection<D> {
    fn drop(&mut self) {
        self.group.parallel(|ctx| {
            ctx.unbind_raster_pipeline(self.zpass);
            ctx.unbind_raster_pipeline(self.gamma_corr_pass);
            ctx.unbind_raster_pipeline(self.g_pass);
            ctx.unbind_raster_pipeline(self.directional_light_pass);
            ctx.unbind_raster_pipeline(self.csm_pass);
        });

        self.rs.free_raster_pipeline_handle(self.zpass);
        self.rs.free_raster_pipeline_handle(self.gamma_corr_pass);
        self.rs.free_raster_pipeline_handle(self.g_pass);
        self.rs
            .free_raster_pipeline_handle(self.directional_light_pass);
        self.rs.free_raster_pipeline_handle(self.csm_pass);
    }
}
