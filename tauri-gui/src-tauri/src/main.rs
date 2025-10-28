// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use directories::ProjectDirs;
use ripple_core::network;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let dirs = ProjectDirs::from("net", "pigeoncatcher", "ripple").unwrap();
    let data_dir = dirs.data_dir();

    let (mut client, worker) = network::new(None, data_dir.to_path_buf()).await;

    tauri_gui_lib::run()
}
