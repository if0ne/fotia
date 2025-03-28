use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::HashMap, time::Duration};
use tokio::{io::AsyncReadExt, net::TcpListener, process::Command};

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
    single_cpu_avg: Duration,
    single_gpu_avg: Duration,
    single_passes_avg: HashMap<String, Duration>,

    multi_cpu_avg: Duration,
    multi_primary_gpu_avg: Duration,
    multi_secondary_gpu_avg: Duration,

    multi_primary_passes_avg: HashMap<String, Duration>,
    multi_secondary_passes_avg: HashMap<String, Duration>,
}

impl SceneBenchmark {
    fn new(scene: &str) -> Self {
        Self {
            scene_name: scene.to_string(),
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
        println!("\nBenchmark results for scene: {}", self.scene_name);

        let single_cpu_avg =
            self.single_cpu.iter().sum::<Duration>() / self.single_cpu.len() as u32;
        let single_gpu_avg =
            self.single_gpu.iter().sum::<Duration>() / self.single_gpu.len() as u32;

        let single_passes_avg = self
            .single_passes
            .into_iter()
            .map(|(pass, times)| {
                let avg_time = times.iter().sum::<Duration>() / times.len() as u32;
                (pass, avg_time)
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
                dbg!(&pass);
                dbg!(&times.len());
                let avg_time = times.iter().sum::<Duration>() / times.len() as u32;
                (pass, avg_time)
            })
            .collect();

        let multi_secondary_passes_avg = self
            .multi_secondary_passes
            .into_iter()
            .map(|(pass, times)| {
                let avg_time = times.iter().sum::<Duration>() / times.len() as u32;
                (pass, avg_time)
            })
            .collect();

        SceneBenchmarkResult {
            scene_name: self.scene_name,
            single_cpu_avg,
            single_gpu_avg,
            single_passes_avg,
            multi_cpu_avg,
            multi_primary_gpu_avg,
            multi_secondary_gpu_avg,
            multi_primary_passes_avg,
            multi_secondary_passes_avg,
        }
    }
}

async fn benchmark_scene(
    scene: &str,
    bench_addr: &str,
    bench_result: &mut BenchmarkResult,
) -> anyhow::Result<()> {
    let mut bench_scene = SceneBenchmark::new(scene);
    let listener = TcpListener::bind(bench_addr).await?;

    let mut app = Command::new("fotia.exe")
        .arg("--scene-path")
        .arg(scene)
        .arg("--bench-addr")
        .arg(bench_addr)
        .spawn()?;

    let (mut stream, _) = listener.accept().await?;

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

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    unsafe {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    let scenes = vec!["./assets/scenes/issum_the_town_on_capital_isle/scene.gltf".to_string()];
    let mut bench_result = BenchmarkResult::default();

    let bench_addr = "127.0.0.1:7878";

    for scene in scenes {
        println!("Starting benchmark for scene: {}", scene);
        if let Err(e) = benchmark_scene(&scene, bench_addr, &mut bench_result).await {
            eprintln!("Error benchmarking {}: {}", scene, e);
        }
    }

    dbg!(bench_result);

    Ok(())
}
