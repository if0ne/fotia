use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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
    multi_primary_copy_gpu_avg: f32,
    multi_secondary_gpu_avg: f32,

    multi_primary_passes_avg: HashMap<String, f32>,
    multi_secondary_passes_avg: HashMap<String, f32>,
}

fn main() {
    let json_str = std::fs::read_to_string("result.json").expect("failed to read json");
    let data: BenchmarkResult = serde_json::from_str(&json_str).expect("failed to deserialize");

    let configuration = data
        .gpus
        .into_iter()
        .map(|g| g.name)
        .collect::<Vec<_>>()
        .join(" + ");

    let mut writer = csv::Writer::from_path("result.csv").expect("failed to create file");
    writer
        .write_record(&[
            "Configuration",
            "Scene",
            "Cascades size",
            "Cascades count",
            "Single Cpu Avg",
            "Single Gpu Avg",
            "Multi Cpu Avg",
            "Multi Primary Gpu Avg",
            "Multi Primary Copy Gpu Avg",
            "Multi Secondary Gpu Avg",
            "Single Z Prepass",
            "Single GPass",
            "Single Cascaded Shadow Maps",
            "Single Directional Light Pass",
            "Single Gamma Correction Pass",
            "Multi Z Prepass",
            "Multi GPass",
            "Multi Directional Light Pass",
            "Multi Gamma Correction Pass",
            "Multi Cascaded Shadow Maps",
            "Multi Push CSM",
        ])
        .expect("failed to write");

    for bm in data.benchmarks {
        writer
            .write_record(&[
                configuration.clone(),
                bm.scene_name,
                bm.cascades_size.to_string(),
                bm.cascades_count.to_string(),
                bm.single_cpu_avg.to_string(),
                bm.single_gpu_avg.to_string(),
                bm.multi_cpu_avg.to_string(),
                bm.multi_primary_gpu_avg.to_string(),
                bm.multi_primary_copy_gpu_avg.to_string(),
                bm.multi_secondary_gpu_avg.to_string(),
                bm.single_passes_avg.get("Z Prepass").unwrap().to_string(),
                bm.single_passes_avg.get("GPass").unwrap().to_string(),
                bm.single_passes_avg
                    .get("Cascaded Shadow Maps")
                    .unwrap()
                    .to_string(),
                bm.single_passes_avg
                    .get("Directional Light Pass")
                    .unwrap()
                    .to_string(),
                bm.single_passes_avg
                    .get("Gamma Correction Pass")
                    .unwrap()
                    .to_string(),
                bm.multi_primary_passes_avg
                    .get("Z Prepass")
                    .unwrap()
                    .to_string(),
                bm.multi_primary_passes_avg
                    .get("GPass")
                    .unwrap()
                    .to_string(),
                bm.multi_primary_passes_avg
                    .get("Directional Light Pass")
                    .unwrap()
                    .to_string(),
                bm.multi_primary_passes_avg
                    .get("Gamma Correction Pass")
                    .unwrap()
                    .to_string(),
                bm.multi_secondary_passes_avg
                    .get("Cascaded Shadow Maps")
                    .unwrap()
                    .to_string(),
                bm.multi_secondary_passes_avg
                    .get("Push CSM")
                    .unwrap()
                    .to_string(),
            ])
            .expect("failed to write");
    }
    writer.flush().expect("failed to flush");
}
