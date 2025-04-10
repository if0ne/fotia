use std::path::{Path, PathBuf};

use walkdir::WalkDir;

fn main() {
    let assets_path = Path::new("assets");
    if assets_path.exists() {
        println!("cargo:rerun-if-changed=assets");
        for entry in WalkDir::new("assets") {
            if let Ok(entry) = entry {
                if entry.file_type().is_file() {
                    println!("cargo:rerun-if-changed={}", entry.path().display());
                }
            }
        }
    }

    for file in &["config.toml", "bench.toml"] {
        let source = Path::new(file);
        if !source.exists() {
            continue;
        }
        println!("cargo:rerun-if-changed={}", file);
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("failed to get manifest dir");
    let profile = std::env::var("PROFILE").expect("failed to get profile");
    let target_dir = PathBuf::from(&manifest_dir).join("target").join(profile);

    if !target_dir.exists() {
        std::fs::create_dir_all(&target_dir).expect("failed to get dirs");
    }

    let options = fs_extra::dir::CopyOptions::new()
        .overwrite(true)
        .copy_inside(true);
    fs_extra::dir::copy(assets_path, &target_dir, &options).expect("failed to copy");

    for file in &["config.toml", "bench.toml"] {
        let source = Path::new(file);
        let dest = target_dir.join(file);
        std::fs::copy(source, dest).expect("failed to build");
    }
}
