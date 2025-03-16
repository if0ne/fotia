use ra::{
    context::{ContextDual, RenderDevice},
    swapchain::{RenderSwapchainContext, Swapchain},
    system::{RenderBackend, RenderBackendSettings, RenderSystem},
};
use rhi::{
    backend::{Api, DebugFlags},
    swapchain::{PresentMode, SwapchainDesc},
};
use tracing_subscriber::layer::SubscriberExt;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

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

    let event_loop = winit::event_loop::EventLoop::new().expect("failed to create event loop");

    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

    let mut app = Application {
        rs,
        context: group,
        wnd_ctx: None,
    };

    event_loop.run_app(&mut app).expect("failed to run app");
}

pub struct WindowContext<D: RenderDevice> {
    pub window: winit::window::Window,
    pub wnd: RawWindowHandle,
    pub swapchain: Swapchain<D>,
}

pub struct Application<D: RenderDevice> {
    pub wnd_ctx: Option<WindowContext<D>>,
    pub rs: RenderSystem,
    pub context: ContextDual<D>,
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
            winit::event::WindowEvent::Focused(_) => {}
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
            winit::event::WindowEvent::RedrawRequested => {}
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
