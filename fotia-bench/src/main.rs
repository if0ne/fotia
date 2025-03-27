use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::HashMap, time::Duration};
use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, TcpStream},
    process::Command,
};

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

struct BenchmarkResults {
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

impl BenchmarkResults {
    fn new(scene: &str) -> Self {
        Self {
            scene_name: scene.to_string(),
            single_cpu: Vec::with_capacity(5000),
            single_gpu: Vec::with_capacity(5000),
            single_passes: HashMap::new(),
            multi_cpu: Vec::with_capacity(5000),
            multi_primary_gpu: Vec::with_capacity(5000),
            multi_secondary_gpu: Vec::with_capacity(5000),
            multi_primary_passes: HashMap::new(),
            multi_secondary_passes: HashMap::new(),
        }
    }

    fn process_single_frame(&mut self, cpu: Duration, gpu: &Timings) {
        self.single_cpu.push(cpu);
        self.single_gpu.push(gpu.total);

        for (pass, duration) in &gpu.timings {
            self.single_passes
                .entry(pass.to_string())
                .or_default()
                .push(*duration);
        }
    }

    fn process_multi_frame(&mut self, cpu: Duration, primary: &Timings, secondary: &Timings) {
        self.multi_cpu.push(cpu);
        self.multi_primary_gpu.push(primary.total);
        self.multi_secondary_gpu.push(secondary.total);

        for (pass, duration) in &primary.timings {
            self.multi_primary_passes
                .entry(pass.to_string())
                .or_default()
                .push(*duration);
        }

        for (pass, duration) in &secondary.timings {
            self.multi_secondary_passes
                .entry(pass.to_string())
                .or_default()
                .push(*duration);
        }
    }

    fn calculate_stats(&self) {
        println!("\nBenchmark results for scene: {}", self.scene_name);

        // Single GPU stats
        let single_cpu_avg =
            self.single_cpu.iter().sum::<Duration>() / self.single_cpu.len() as u32;
        let single_gpu_avg =
            self.single_gpu.iter().sum::<Duration>() / self.single_gpu.len() as u32;

        println!("\nSingle GPU:");
        println!("Average CPU time: {:?}", single_cpu_avg);
        println!("Average GPU time: {:?}", single_gpu_avg);
        println!("Render passes:");
        self.print_passes(&self.single_passes);

        // Multi GPU stats
        let multi_cpu_avg = self.multi_cpu.iter().sum::<Duration>() / self.multi_cpu.len() as u32;
        let primary_gpu_avg =
            self.multi_primary_gpu.iter().sum::<Duration>() / self.multi_primary_gpu.len() as u32;
        let secondary_gpu_avg = self.multi_secondary_gpu.iter().sum::<Duration>()
            / self.multi_secondary_gpu.len() as u32;

        println!("\nMulti GPU:");
        println!("Average CPU time: {:?}", multi_cpu_avg);
        println!("Primary GPU average: {:?}", primary_gpu_avg);
        println!("Secondary GPU average: {:?}", secondary_gpu_avg);

        println!("\nPrimary GPU passes:");
        self.print_passes(&self.multi_primary_passes);
        println!("\nSecondary GPU passes:");
        self.print_passes(&self.multi_secondary_passes);
    }

    fn print_passes(&self, passes: &HashMap<String, Vec<Duration>>) {
        for (pass, times) in passes {
            let avg = times.iter().sum::<Duration>() / times.len() as u32;
            println!("  {}: {:?} (avg)", pass, avg);
        }
    }
}

async fn benchmark_scene(scene: &str, bench_addr: &str) -> anyhow::Result<()> {
    let mut results = BenchmarkResults::new(scene);
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

    // Группируем сообщения по типам
    let mut single_gpu = Vec::new();
    let mut multi_primary = Vec::new();
    let mut multi_secondary = Vec::new();
    let mut single_cpu_total = Vec::new();
    let mut multi_cpu_total = Vec::new();

    for msg in messages {
        match msg {
            TimingsInfo::PrimarySingleGpu(t) => single_gpu.push(t),
            TimingsInfo::SingleCpuTotal(d) => single_cpu_total.push(d),
            TimingsInfo::PrimaryMultiGpu(t) => multi_primary.push(t),
            TimingsInfo::SecondaryMultiGpu(t) => multi_secondary.push(t),
            TimingsInfo::MultiCpuTotal(d) => multi_cpu_total.push(d),
            _ => {}
        }
    }

    // Обработка Single GPU режима
    let single_frames = single_gpu.len().min(single_cpu_total.len());
    for i in 0..single_frames {
        results.process_single_frame(single_cpu_total[i], &single_gpu[i]);
    }

    // Обработка Multi GPU режима
    let multi_frames = multi_primary
        .len()
        .min(multi_secondary.len())
        .min(multi_cpu_total.len());

    for i in 0..multi_frames {
        results.process_multi_frame(multi_cpu_total[i], &multi_primary[i], &multi_secondary[i]);
    }

    app.kill().await?;
    results.calculate_stats();
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let scenes = vec!["./assets/scenes/issum_the_town_on_capital_isle/scene.gltf".to_string()];

    let bench_addr = "127.0.0.1:7878";

    for scene in scenes {
        println!("Starting benchmark for scene: {}", scene);
        if let Err(e) = benchmark_scene(&scene, bench_addr).await {
            eprintln!("Error benchmarking {}: {}", scene, e);
        }
    }

    Ok(())
}
