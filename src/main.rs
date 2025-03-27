pub mod collections;
pub mod engine;
pub mod ra;
pub mod rhi;
pub mod timer;

mod multi_gpu_renderer;
mod settings;

use std::{cell::RefCell, collections::HashMap, io::Write, sync::Arc};

use collections::handle::Handle;
use engine::{
    camera::{Camera, FpsController},
    gltf::GltfScene,
};
use glam::{vec2, vec3};
use hecs::World;
use multi_gpu_renderer::{
    GpuGlobals, TexturePlaceholders, create_multi_gpu_scene,
    graphs::{multi_gpu::MultiGpuShadows, single_gpu::SingleGpuShadows},
    pso::PsoCollection,
    shaders::ShaderCollection,
};
use ra::{
    command::{Barrier, RenderCommandContext, RenderCommandEncoder},
    context::{ContextDual, RenderDevice},
    resources::{Buffer, RenderResourceContext},
    shader::{RenderShaderContext, ShaderArgument},
    swapchain::{RenderSwapchainContext, Surface, Swapchain},
    system::{RenderBackend, RenderBackendSettings, RenderSystem},
};
use rhi::{
    backend::{Api, DebugFlags, RenderDeviceInfo},
    command::{CommandType, Subresource},
    dx12::device::DxDevice,
    resources::BufferUsages,
    swapchain::{PresentMode, SwapchainDesc},
    types::{ResourceState, Timings},
};
use serde::{Deserialize, Serialize};
use settings::{RenderSettings, read_settings};
use timer::GameTimer;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use winit::{
    keyboard::{KeyCode, PhysicalKey},
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TimingsInfo {
    GpuInfo {
        primary: RenderDeviceInfo,
        secondary: RenderDeviceInfo,
    },
    PrimarySingleGpu(Timings),
    PrimaryMultiGpu(Timings),
    SecondaryMultiGpu(Timings),
    SingleCpuTotal(std::time::Duration),
    MultiCpuTotal(std::time::Duration),
    End,
}

pub struct WindowContext<D: RenderDevice> {
    pub window: winit::window::Window,
    pub wnd: RawWindowHandle,
    pub swapchain: Swapchain<D>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RenderMode {
    SingleGpu,
    MultiGpu,
}

impl RenderMode {
    pub fn next(&mut self) {
        match *self {
            RenderMode::SingleGpu => *self = RenderMode::MultiGpu,
            RenderMode::MultiGpu => *self = RenderMode::SingleGpu,
        }
    }
}

pub struct Application<D: RenderDevice> {
    pub title: String,
    pub width: u32,
    pub height: u32,

    pub timer: GameTimer,

    pub world: World,

    pub wnd_ctx: Option<WindowContext<D>>,
    pub rs: Arc<RenderSystem>,
    pub context: Arc<ContextDual<D>>,

    pub shaders: ShaderCollection,
    pub psos: PsoCollection<D>,

    pub single_gpu: SingleGpuShadows<D>,
    pub multi_gpu: MultiGpuShadows<D>,
    pub render_mode: RenderMode,

    pub keys: HashMap<PhysicalKey, bool>,

    pub frames_in_flight: usize,
    pub frame_idx: usize,
    pub total_frames: usize,

    pub camera: Camera,
    pub fps_controller: FpsController,

    pub buffer: Handle<Buffer>,
    pub global_argument: Handle<ShaderArgument>,

    pub placeholders: TexturePlaceholders,

    pub bench_sender: Option<std::sync::mpsc::Sender<TimingsInfo>>,
    pub is_bench_mode: bool,
}

fn main() {
    unsafe {
        std::env::set_var("RUST_BACKTRACE", "1");
    }
    let console_log = tracing_subscriber::fmt::Layer::new()
        .with_ansi(true)
        .with_writer(std::io::stdout);
    let subscriber = tracing_subscriber::registry().with(console_log);
    let _ = tracing::subscriber::set_global_default(subscriber);

    let event_loop = winit::event_loop::EventLoop::new().expect("failed to create event loop");
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

    let settings = read_settings();

    let (sdr, thread) = if let Some(addr) = &settings.bench_addr {
        let (sdr, rcv) = std::sync::mpsc::channel();
        let mut connection = std::net::TcpStream::connect(addr).expect("wrong TCP-address");

        let thread = std::thread::spawn(move || {
            let mut timings = vec![];
            while let Ok(data) = rcv.recv_timeout(std::time::Duration::from_secs(30)) {
                if data == TimingsInfo::End {
                    let data = timings.drain(..).collect::<Vec<_>>();
                    let json = serde_json::to_string(&data).expect("failed to serialize");

                    connection
                        .write_all(json.as_bytes())
                        .expect("failed to send data to server");
                    return;
                }

                timings.push(data);
            }
        });

        (Some(sdr), Some(thread))
    } else {
        (None, None)
    };

    let mut app = Application::new(settings, sdr.clone());
    event_loop.run_app(&mut app).expect("failed to run app");
    if let Some(sdr) = sdr {
        sdr.send(TimingsInfo::End).expect("failed to send message");
    }

    if let Some(thread) = thread {
        thread.join().expect("failed to join");
    }
}

impl Application<DxDevice> {
    fn new(
        settings: RenderSettings,
        sender: Option<std::sync::mpsc::Sender<TimingsInfo>>,
    ) -> Application<DxDevice> {
        let rs = Arc::new(RenderSystem::new(&[RenderBackendSettings {
            api: RenderBackend::Dx12,
            debug: if cfg!(debug_assertions) {
                DebugFlags::all()
            } else {
                DebugFlags::empty()
            },
        }]));

        let backend = rs.dx_backend().expect("failed to get directx backend");

        let shaders = ShaderCollection::new(&backend, cfg!(debug_assertions), &settings);

        if let Some(sender) = &sender {
            let mut info = backend.enumerate_devices().take(2).cloned();
            sender
                .send(TimingsInfo::GpuInfo {
                    primary: info.next().expect("failed to get gpu info"),
                    secondary: info.next().expect("failed to get gpu info"),
                })
                .expect("failed to send");
        }

        let primary = Arc::new(backend.create_device(0));
        let secondary = Arc::new(backend.create_device(1));

        let group = Arc::new(ContextDual::new(primary, secondary));

        let psos = PsoCollection::new(Arc::clone(&rs), Arc::clone(&group), &shaders);

        let single_gpu = SingleGpuShadows::new(
            Arc::clone(&rs),
            Arc::clone(&group.primary),
            [settings.width, settings.height],
            &psos,
            &settings,
        );

        let multi_gpu = MultiGpuShadows::new(
            Arc::clone(&rs),
            Arc::clone(&group),
            [settings.width, settings.height],
            &psos,
            &settings,
            sender.clone(),
        );

        let mut world = World::new();

        let fps_controller = FpsController::new(0.003, 100.0);

        let camera = Camera {
            far: settings.camera_far,
            near: 0.1,
            fov: 90.0f32.to_radians(),
            aspect_ratio: settings.width as f32 / settings.height as f32,
            view: glam::Mat4::IDENTITY,
        };

        let buffer = rs.create_buffer_handle();
        let global_argument = rs.create_shader_argument_handle();

        group.call(|ctx| {
            ctx.bind_buffer(
                buffer,
                rhi::resources::BufferDesc::cpu_to_gpu(
                    size_of::<GpuGlobals>() * settings.frames_in_flight,
                    BufferUsages::Uniform,
                )
                .with_name("Global data".into()),
                None,
            );

            ctx.bind_shader_argument(
                global_argument,
                ra::shader::ShaderArgumentDesc {
                    views: &[],
                    samplers: &[],
                    dynamic_buffer: Some(buffer),
                },
            );
        });

        let placeholders = TexturePlaceholders::new(&rs, &group);

        let scene = GltfScene::load(&settings.scene_path);
        create_multi_gpu_scene(scene, &mut world, &rs, &group, &settings, &placeholders);

        Application {
            title: format!("Fotia Render Mode: {:?}", RenderMode::SingleGpu),
            width: settings.width,
            height: settings.height,

            timer: GameTimer::default(),

            rs,
            context: group,
            wnd_ctx: None,

            shaders,
            psos,
            single_gpu,
            multi_gpu,
            render_mode: RenderMode::SingleGpu,

            world,
            frames_in_flight: settings.frames_in_flight,
            frame_idx: 0,
            camera,
            fps_controller,

            buffer,
            global_argument,
            placeholders,

            keys: HashMap::new(),
            is_bench_mode: sender.is_some(),
            total_frames: 0,
            bench_sender: sender,
        }
    }
}

impl<D: RenderDevice> Application<D> {
    fn update(&mut self) {
        let mut direction = glam::Vec3::ZERO;

        if self
            .keys
            .get(&PhysicalKey::Code(KeyCode::KeyW))
            .is_some_and(|v| *v)
        {
            direction.z += 1.0;
        }

        if self
            .keys
            .get(&PhysicalKey::Code(KeyCode::KeyS))
            .is_some_and(|v| *v)
        {
            direction.z -= 1.0;
        }

        if self
            .keys
            .get(&PhysicalKey::Code(KeyCode::KeyD))
            .is_some_and(|v| *v)
        {
            direction.x += 1.0;
        }

        if self
            .keys
            .get(&PhysicalKey::Code(KeyCode::KeyA))
            .is_some_and(|v| *v)
        {
            direction.x -= 1.0;
        }

        direction = direction.normalize();

        if direction.length() > f32::EPSILON {
            self.fps_controller.update_position(
                self.timer.delta_time(),
                &mut self.camera,
                direction,
            );
        }

        let view = self.camera.view();
        let proj = self.camera.proj();

        self.context.call(|ctx| {
            ctx.update_buffer(
                self.buffer,
                self.frame_idx,
                &[GpuGlobals {
                    view,
                    proj,
                    proj_view: proj * view,
                    inv_view: view.inverse(),
                    inv_proj: proj.inverse(),
                    inv_proj_view: (proj * view).inverse(),
                    eye_pos: self.fps_controller.position,
                    _pad0: 0.0,
                    screen_dim: vec2(self.width as f32, self.height as f32),
                    _pad1: Default::default(),
                }],
            );
        });

        if let RenderMode::SingleGpu = self.render_mode {
            self.single_gpu.update(
                &self.camera,
                glam::Vec3::new(-1.0, -1.0, -1.0),
                self.frame_idx,
            );
        }
    }

    fn render(&mut self) {
        let Some(wnd) = &mut self.wnd_ctx else {
            return;
        };

        let frame = wnd.swapchain.next_frame();
        self.context.call_primary(|ctx| {
            ctx.wait_on_cpu(CommandType::Graphics, frame.last_access);
            let mut encoder = ctx.create_encoder(CommandType::Graphics);
            let timings = encoder.begin(ctx);

            if let Some(sdr) = &mut self.bench_sender {
                if let Some(timings) = timings {
                    match self.render_mode {
                        RenderMode::SingleGpu => sdr
                            .send(TimingsInfo::PrimarySingleGpu(timings))
                            .expect("failed to send"),
                        RenderMode::MultiGpu => sdr
                            .send(TimingsInfo::PrimaryMultiGpu(timings))
                            .expect("failed to send"),
                    }
                }
            } else {
                info!("Timings: {:?}", timings);
            }

            encoder.set_barriers(&[Barrier::Texture(
                frame.texture,
                ResourceState::RenderTarget,
                Subresource::Local(None),
            )]);
            ctx.enqueue(encoder);

            let time = std::time::Instant::now();

            match self.render_mode {
                RenderMode::SingleGpu => {
                    self.single_gpu.render(
                        &self.world,
                        self.global_argument,
                        frame.texture,
                        self.frame_idx,
                    );
                }
                RenderMode::MultiGpu => {
                    self.multi_gpu.render(
                        &self.world,
                        self.global_argument,
                        frame.texture,
                        &self.camera,
                        vec3(-1.0, -1.0, -1.0),
                        self.frame_idx,
                    );
                }
            }

            let mut encoder = ctx.create_encoder(CommandType::Graphics);
            encoder.set_barriers(&[Barrier::Texture(
                frame.texture,
                ResourceState::Present,
                Subresource::Local(None),
            )]);

            ctx.commit(encoder);
            frame.last_access = ctx.submit(CommandType::Graphics);

            if let Some(sdr) = &mut self.bench_sender {
                match self.render_mode {
                    RenderMode::SingleGpu => sdr
                        .send(TimingsInfo::SingleCpuTotal(time.elapsed()))
                        .expect("failed to send"),
                    RenderMode::MultiGpu => sdr
                        .send(TimingsInfo::MultiCpuTotal(time.elapsed()))
                        .expect("failed to send"),
                }
            } else {
                info!("CPU TIME: {:?}", time.elapsed());
            }
        });

        wnd.swapchain.present();

        self.frame_idx = (self.frame_idx + 1) % self.frames_in_flight;
    }

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
                            .set_title(&format!("{} Fps: {fps} Ms: {mspf}", self.title))
                    }

                    *frame_count = 0;
                    *time_elapsed += 1.0;
                });
            }
        })
    }
}

impl<D: RenderDevice> winit::application::ApplicationHandler for Application<D> {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window_attributes = winit::window::Window::default_attributes()
            .with_title(&self.title)
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
                width: self.width,
                height: self.height,
                present_mode: PresentMode::Immediate,
                frames: self.frames_in_flight,
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
                    if self.is_bench_mode {
                        return;
                    }

                    if event.physical_key == winit::keyboard::KeyCode::Escape {
                        event_loop.exit();
                    } else if event.physical_key == KeyCode::Digit1 {
                        self.render_mode = RenderMode::SingleGpu;
                        self.title = format!("Fotia Render Mode: {:?}", self.render_mode);
                    } else if event.physical_key == KeyCode::Digit2 {
                        self.render_mode = RenderMode::MultiGpu;
                        self.title = format!("Fotia Render Mode: {:?}", self.render_mode);
                    }

                    self.keys.insert(event.physical_key, true);
                }
                winit::event::ElementState::Released => {
                    self.keys.insert(event.physical_key, false);
                }
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
                    self.single_gpu.resize([size.width, size.height]);
                    self.multi_gpu.resize([size.width, size.height]);

                    self.width = size.width;
                    self.height = size.height;
                }

                self.camera.resize([size.width, size.height]);
            }
            winit::event::WindowEvent::RedrawRequested => {
                if self.total_frames > 5000 && self.render_mode != RenderMode::MultiGpu {
                    self.render_mode = RenderMode::MultiGpu;
                }

                if self.total_frames > 10000 && self.is_bench_mode {
                    event_loop.exit();
                }

                self.timer.tick();
                self.calculate_frame_stats();

                self.update();
                self.render();

                self.total_frames += 1;
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
            winit::event::DeviceEvent::MouseMotion { delta } => {
                self.fps_controller.update_yaw_pitch(
                    &mut self.camera,
                    delta.0 as f32,
                    delta.1 as f32,
                );
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &winit::event_loop::ActiveEventLoop) {
        if let Some(context) = self.wnd_ctx.as_ref() {
            context.window.request_redraw();
        }
    }
}

impl<D: RenderDevice> Drop for Application<D> {
    fn drop(&mut self) {
        self.context.call(|ctx| ctx.wait_idle());
    }
}
