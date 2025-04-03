mod settings;

use serde::{Deserialize, Serialize};
use settings::read_settings;
use std::{borrow::Cow, collections::HashMap, process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, net::TcpListener, process::Command};
use tracing::{error, info};
use tracing_subscriber::layer::SubscriberExt;

#[derive(Debug, Serialize, Deserialize)]
enum TimingsInfo {
    GpuInfo {
        primary: RenderDeviceInfo,
        secondary: RenderDeviceInfo,
    },
    PrimarySingleGpu(Timings),
    PrimaryMultiGpu(Timings),
    SecondaryMultiGpu(Timings),
    SingleCpuTotal(Duration),
    MultiCpuTotal(Duration),
}

#[derive(Debug, Serialize, Deserialize)]
struct Timings {
    timings: Vec<(Cow<'static, str>, Duration)>,
    total: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceType {
    Discrete,
    Integrated,
    Cpu,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct RenderDeviceInfo {
    pub name: String,
    pub id: usize,
    pub is_cross_adapter_texture_supported: bool,
    pub is_uma: bool,
    pub ty: DeviceType,
    pub copy_timestamp_support: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct BenchmarkResult {
    pub gpus: Vec<RenderDeviceInfo>,
    pub benchmarks: Vec<SceneBenchmarkResult>,
}

#[derive(Clone, Debug, PartialEq)]
struct SceneBenchmark {
    scene_name: String,
    cascades_size: u32,
    cascades_count: u32,
    single_cpu: Vec<Duration>,
    single_gpu: Vec<Duration>,
    single_passes: HashMap<String, Vec<Duration>>,

    multi_cpu: Vec<Duration>,
    multi_primary_gpu: Vec<Duration>,
    multi_secondary_gpu: Vec<Duration>,

    multi_primary_passes: HashMap<String, Vec<Duration>>,
    multi_secondary_passes: HashMap<String, Vec<Duration>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SceneBenchmarkResult {
    scene_name: String,
    cascades_size: u32,
    cascades_count: u32,
    single_cpu_avg: f32,
    single_gpu_avg: f32,
    single_passes_avg: HashMap<String, f32>,

    multi_cpu_avg: f32,
    multi_primary_gpu_avg: f32,
    multi_secondary_gpu_avg: f32,

    multi_primary_passes_avg: HashMap<String, f32>,
    multi_secondary_passes_avg: HashMap<String, f32>,
}

impl SceneBenchmark {
    fn new(scene: &str, cascades_size: u32, cascades_count: u32) -> Self {
        Self {
            scene_name: scene.to_string(),
            cascades_size,
            cascades_count,
            single_cpu: Vec::new(),
            single_gpu: Vec::new(),
            single_passes: HashMap::new(),
            multi_cpu: Vec::new(),
            multi_primary_gpu: Vec::new(),
            multi_secondary_gpu: Vec::new(),
            multi_primary_passes: HashMap::new(),
            multi_secondary_passes: HashMap::new(),
        }
    }

    fn calculate_result(self) -> SceneBenchmarkResult {
        info!(
            "Calculating benchmark results for scene: {}, cascade size: {}, cascade count: {}",
            self.scene_name, self.cascades_size, self.cascades_count
        );

        let single_cpu_avg =
            self.single_cpu.iter().sum::<Duration>() / self.single_cpu.len() as u32;
        let single_gpu_avg =
            self.single_gpu.iter().sum::<Duration>() / self.single_gpu.len() as u32;

        let single_passes_avg = self
            .single_passes
            .into_iter()
            .map(|(pass, times)| {
                let avg_time = times.iter().sum::<Duration>() / times.len() as u32;
                (pass, avg_time.as_secs_f32() * 1000.0)
            })
            .collect();

        let multi_cpu_avg = self.multi_cpu.iter().sum::<Duration>() / self.multi_cpu.len() as u32;
        let multi_primary_gpu_avg =
            self.multi_primary_gpu.iter().sum::<Duration>() / self.multi_primary_gpu.len() as u32;
        let multi_secondary_gpu_avg = self.multi_secondary_gpu.iter().sum::<Duration>()
            / self.multi_secondary_gpu.len() as u32;

        let multi_primary_passes_avg = self
            .multi_primary_passes
            .into_iter()
            .map(|(pass, times)| {
                let avg_time = times.iter().sum::<Duration>() / times.len() as u32;
                (pass, avg_time.as_secs_f32() * 1000.0)
            })
            .collect();

        let multi_secondary_passes_avg = self
            .multi_secondary_passes
            .into_iter()
            .map(|(pass, times)| {
                let avg_time = times.iter().sum::<Duration>() / times.len() as u32;
                (pass, avg_time.as_secs_f32() * 1000.0)
            })
            .collect();

        SceneBenchmarkResult {
            scene_name: self.scene_name,
            cascades_size: self.cascades_size,
            cascades_count: self.cascades_count,
            single_cpu_avg: single_cpu_avg.as_secs_f32() * 1000.0,
            single_gpu_avg: single_gpu_avg.as_secs_f32() * 1000.0,
            single_passes_avg,
            multi_cpu_avg: multi_cpu_avg.as_secs_f32() * 1000.0,
            multi_primary_gpu_avg: multi_primary_gpu_avg.as_secs_f32() * 1000.0,
            multi_secondary_gpu_avg: multi_secondary_gpu_avg.as_secs_f32() * 1000.0,
            multi_primary_passes_avg,
            multi_secondary_passes_avg,
        }
    }
}

async fn benchmark_scene(
    scene: &str,
    bench_addr: &str,
    listener: &TcpListener,
    width: u32,
    height: u32,
    cascades_size: u32,
    cascades_count: u32,
    bench_frames: usize,
    bench_result: &mut BenchmarkResult,
) -> anyhow::Result<()> {
    let mut bench_scene = SceneBenchmark::new(scene, cascades_size, cascades_count);

    let mut app = Command::new("fotia.exe")
        .arg("--scene-path")
        .arg(scene)
        .arg("--bench-addr")
        .arg(bench_addr)
        .arg("--width")
        .arg(width.to_string())
        .arg("--height")
        .arg(height.to_string())
        .arg("--cascade-size")
        .arg(cascades_size.to_string())
        .arg("--cascades-count")
        .arg(cascades_count.to_string())
        .arg("--bench-frames")
        .arg(bench_frames.to_string())
        .stdin(Stdio::null())
        .spawn()?;

    tokio::select! {
        _ = app.wait() => {

        }
        result =  listener.accept() => {
            let (mut stream, _) = result?;
            app.wait().await?;

            let mut json_data = Vec::new();
            stream.read_to_end(&mut json_data).await?;

            let messages: Vec<TimingsInfo> = serde_json::from_slice(&json_data)?;

            for msg in messages {
                match msg {
                    TimingsInfo::PrimarySingleGpu(t) => {
                        for (pass, duration) in &t.timings {
                            bench_scene
                                .single_passes
                                .entry(pass.to_string())
                                .or_default()
                                .push(*duration);
                        }
                        bench_scene.single_gpu.push(t.total);
                    }
                    TimingsInfo::SingleCpuTotal(d) => bench_scene.single_cpu.push(d),
                    TimingsInfo::PrimaryMultiGpu(t) => {
                        for (pass, duration) in &t.timings {
                            bench_scene
                                .multi_primary_passes
                                .entry(pass.to_string())
                                .or_default()
                                .push(*duration);
                        }
                        bench_scene.multi_primary_gpu.push(t.total);
                    }
                    TimingsInfo::SecondaryMultiGpu(t) => {
                        for (pass, duration) in &t.timings {
                            bench_scene
                                .multi_secondary_passes
                                .entry(pass.to_string())
                                .or_default()
                                .push(*duration);
                        }
                        bench_scene.multi_secondary_gpu.push(t.total);
                    }
                    TimingsInfo::MultiCpuTotal(d) => bench_scene.multi_cpu.push(d),
                    TimingsInfo::GpuInfo { primary, secondary } => {
                        if bench_result.gpus.is_empty() {
                            bench_result.gpus.push(primary);
                            bench_result.gpus.push(secondary);
                        }
                    }
                }
            }

            bench_result.benchmarks.push(bench_scene.calculate_result());

            app.kill().await?;
        }
    };

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    unsafe {
        std::env::set_var("RUST_BACKTRACE", "1");
    }
    let console_log = tracing_subscriber::fmt::Layer::new()
        .with_ansi(true)
        .with_writer(std::io::stdout);
    let subscriber = tracing_subscriber::registry().with(console_log);
    let _ = tracing::subscriber::set_global_default(subscriber);

    let settings = read_settings().await;
    let mut bench_result = BenchmarkResult::default();

    let bench_addr = format!("127.0.0.1:{}", settings.port);
    let listener = TcpListener::bind(&bench_addr).await?;

    for scene in settings.scenes {
        for size in [1024, 2048, 4096] {
            for count in [3, 4] {
                info!(
                    "Starting benchmark for scene: {}, cascade size: {}, cascade count: {}",
                    scene, size, count
                );
                if let Err(e) = benchmark_scene(
                    &scene,
                    &bench_addr,
                    &listener,
                    settings.width,
                    settings.height,
                    size,
                    count,
                    settings.bench_frames,
                    &mut bench_result,
                )
                .await
                {
                    error!("Error benchmarking {}: {}", scene, e);
                }
            }
        }
    }

    let contents = serde_json::to_vec_pretty(&bench_result).expect("failed to serialize");
    tokio::fs::write("result.json", &contents)
        .await
        .expect("faild to write result into file");

    Ok(())
}
