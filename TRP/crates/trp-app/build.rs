//! The version the app prints first and shows (`--version`, the panel, a glTF it writes) is
//! read from `[workspace.metadata.trp]` in the workspace's `Cargo.toml` and handed to the code
//! as the environment variable `TRP_VERSION` (`env!("TRP_VERSION")`). It is not the packages'
//! own `version`, which stays at 0.0.0: a package's version is part of what cargo builds it
//! under, so raising it with every change (`docs/manual.md`, "Versions") rebuilt all four
//! crates and relinked every test binary; read this way, a raise rebuilds this crate alone.
use std::{env, fs, path::Path};

fn main() {
    let manifest = Path::new(&env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let text = fs::read_to_string(&manifest).unwrap_or_else(|e| panic!("{}: {e}", manifest.display()));
    let mut in_table = false;
    let mut version = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_table = line == "[workspace.metadata.trp]";
        } else if in_table {
            if let Some(value) = line.strip_prefix("version").and_then(|rest| rest.trim_start().strip_prefix('=')) {
                version = Some(value.trim().trim_matches('"').to_string());
                break;
            }
        }
    }
    let version = version.expect("the workspace's Cargo.toml has no `version` under [workspace.metadata.trp]");
    println!("cargo:rustc-env=TRP_VERSION={version}");
}
