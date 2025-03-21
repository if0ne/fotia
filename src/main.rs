use std::{cell::RefCell, sync::Arc};

use multi_gpu_renderer::{pso::PsoCollection, shaders::ShaderCollection};
use ra::{
    command::{Barrier, RenderCommandContext, RenderCommandEncoder, RenderEncoder},
    context::{ContextDual, RenderDevice},
    swapchain::{RenderSwapchainContext, Surface, Swapchain},
    system::{RenderBackend, RenderBackendSettings, RenderSystem},
};
use rhi::{
    backend::{Api, DebugFlags},
    command::CommandType,
    swapchain::{PresentMode, SwapchainDesc},
    types::ResourceState,
};
use timer::GameTimer;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub mod collections;
pub mod engine;
pub mod ra;
pub mod rhi;
pub mod timer;

mod multi_gpu_renderer;

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
