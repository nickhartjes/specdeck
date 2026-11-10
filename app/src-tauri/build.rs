use tauri_build::{AppManifest, Attributes};

fn main() {
    // The app's commands, granted per window by name in capabilities/ (`confine-window-navigation`
    // design decision 3). With an app manifest Tauri checks every command against them.
    println!("cargo:rerun-if-changed=commands.txt");
    let list = std::fs::read_to_string("commands.txt").expect("commands.txt");
    let commands: Vec<&'static str> = list
        .leak()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    let manifest = AppManifest::new().commands(commands.leak());
    tauri_build::try_build(Attributes::new().app_manifest(manifest)).expect("tauri-build");
}
