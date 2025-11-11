
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=proto");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let descriptor_path = out_dir.join("descriptor.bin");

    let mut config = prost_build::Config::new();
    config.file_descriptor_set_path(&descriptor_path);
    config.out_dir(out_dir.clone());

    let proto_dir = PathBuf::from("proto");
    let mut protos: Vec<PathBuf> = vec![];
    if proto_dir.exists() {
        for entry in walkdir::WalkDir::new(&proto_dir) {
            let entry = entry.unwrap();
            if entry.path().extension().and_then(|s| s.to_str()) == Some("proto") {
                protos.push(entry.into_path());
            }
        }
    }

    if !protos.is_empty() {
        config.compile_protos(&protos, &["proto"]).expect("Failed to compile protos");
    }

    let descriptor_target = PathBuf::from("src/descriptor.bin");
    let _ = fs::copy(&descriptor_path, &descriptor_target);
}


