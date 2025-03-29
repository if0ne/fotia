use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct BenchSettings {
    pub width: u32,
    pub height: u32,
    pub port: u16,
    pub scenes: Vec<String>,
    pub bench_frames: usize,
}

pub async fn read_settings() -> BenchSettings {
    let content = tokio::fs::read_to_string("bench.toml")
        .await
        .expect("failed to open config");

    toml::from_str(&content).expect("failed to parse toml")
}
