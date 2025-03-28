use std::path::PathBuf;

use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct CliRenderSettings {
    #[arg(long)]
    pub width: Option<u32>,

    #[arg(long)]
    pub height: Option<u32>,

    #[arg(long)]
    pub cascades_count: Option<usize>,

    #[arg(long)]
    pub cascade_size: Option<u32>,

    #[arg(long)]
    pub scene_path: Option<String>,

    #[arg(long)]
    pub asset_path: Option<String>,

    #[arg(long)]
    pub scene_scale: Option<f32>,

    #[arg(long)]
    pub bench_addr: Option<String>,

    #[arg(long)]
    pub bench_frames: Option<usize>,

    #[arg(long)]
    pub frames_in_flight: Option<usize>,

    #[arg(long)]
    pub camera_far: Option<f32>,

    #[arg(long)]
    pub shadows_far: Option<f32>,

    #[arg(long)]
    pub cascades_lambda: Option<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TomlRenderSettings {
    #[serde(default = "default_width")]
    pub width: u32,

    #[serde(default = "default_height")]
    pub height: u32,

    #[serde(default = "default_cascades_count")]
    pub cascades_count: usize,

    #[serde(default = "default_cascade_size")]
    pub cascade_size: u32,

    pub scene_path: Option<String>,

    pub asset_path: Option<String>,

    #[serde(default = "default_scene_scale")]
    pub scene_scale: f32,

    pub bench_addr: Option<String>,

    #[serde(default = "default_bench_frames")]
    pub bench_frames: usize,

    #[serde(default = "default_frames_in_flight")]
    pub frames_in_flight: usize,

    #[serde(default = "default_camera_far")]
    pub camera_far: f32,

    pub shadows_far: Option<f32>,

    #[serde(default = "default_cascades_lambda")]
    pub cascades_lambda: f32,
}

#[derive(Clone, Debug)]
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub cascades_count: usize,
    pub cascade_size: u32,
    pub scene_path: PathBuf,
    pub asset_path: PathBuf,
    pub scene_scale: f32,
    pub bench_addr: Option<String>,
    pub bench_frames: usize,
    pub frames_in_flight: usize,
    pub camera_far: f32,
    pub shadows_far: Option<f32>,
    pub cascades_lambda: f32,
}

pub fn read_settings() -> RenderSettings {
    let cli = CliRenderSettings::parse();

    if let Ok(content) = std::fs::read_to_string("config.toml") {
        if let Ok(toml) = toml::from_str(&content) {
            return merge_settings(cli, toml);
        }
    }

    RenderSettings {
        width: cli.width.unwrap_or_else(default_width),
        height: cli.width.unwrap_or_else(default_height),
        cascades_count: cli.cascades_count.unwrap_or_else(default_cascades_count),
        cascade_size: cli.cascade_size.unwrap_or_else(default_cascade_size),
        scene_path: cli.scene_path.expect("failed to get scene path").into(),
        asset_path: cli.asset_path.expect("failed to get asset path").into(),
        scene_scale: cli.scene_scale.unwrap_or_else(default_scene_scale),
        bench_addr: cli.bench_addr,
        frames_in_flight: cli
            .frames_in_flight
            .unwrap_or_else(default_frames_in_flight),
        camera_far: cli.camera_far.unwrap_or_else(default_camera_far),
        shadows_far: cli.shadows_far,
        cascades_lambda: cli.cascades_lambda.unwrap_or_else(default_cascades_lambda),
        bench_frames: cli.bench_frames.unwrap_or_else(default_bench_frames),
    }
}

pub fn merge_settings(cli: CliRenderSettings, toml: TomlRenderSettings) -> RenderSettings {
    RenderSettings {
        width: cli.width.unwrap_or(toml.width),
        height: cli.height.unwrap_or(toml.height),
        cascades_count: cli.cascades_count.unwrap_or(toml.cascades_count),
        cascade_size: cli.cascade_size.unwrap_or(toml.cascade_size),
        scene_path: cli
            .scene_path
            .or(toml.scene_path)
            .expect("failed to get scene path")
            .into(),
        asset_path: cli
            .asset_path
            .or(toml.asset_path)
            .expect("failed to get asset path")
            .into(),
        scene_scale: cli.scene_scale.unwrap_or(toml.scene_scale),
        bench_addr: cli.bench_addr.or(toml.bench_addr),
        frames_in_flight: cli.frames_in_flight.unwrap_or(toml.frames_in_flight),
        camera_far: cli.camera_far.unwrap_or(toml.camera_far),
        shadows_far: cli.shadows_far.or(toml.shadows_far),
        cascades_lambda: cli.cascades_lambda.unwrap_or(toml.cascades_lambda),
        bench_frames: cli.bench_frames.unwrap_or(toml.bench_frames),
    }
}

fn default_width() -> u32 {
    800
}

fn default_height() -> u32 {
    600
}

fn default_frames_in_flight() -> usize {
    3
}

fn default_scene_scale() -> f32 {
    1.0
}

fn default_cascades_count() -> usize {
    4
}

fn default_cascade_size() -> u32 {
    2048
}

fn default_camera_far() -> f32 {
    1000.0
}

fn default_cascades_lambda() -> f32 {
    0.5
}

fn default_bench_frames() -> usize {
    5000
}
