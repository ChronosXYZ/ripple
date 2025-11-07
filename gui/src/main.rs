use crate::app::{AppModel, AppModelInit};
use directories::ProjectDirs;
use relm4::RelmApp;
use ripple_core::network;

pub mod app;
mod components;

mod icon_names {
    pub use shipped::*; // Include all shipped icons by default
    include!(concat!(env!("OUT_DIR"), "/icon_names.rs"));
}

fn main() {
    pretty_env_logger::init();

    let dirs = ProjectDirs::from("", "", "ripple").unwrap();
    let data_dir = dirs.data_dir();

    relm4::RELM_THREADS.set(4).unwrap();

    let app = RelmApp::new("net.pigeoncatcher.ripple");
    relm4_icons::initialize_icons(icon_names::GRESOURCE_BYTES, icon_names::RESOURCE_PREFIX);
    app.run_async::<AppModel>(AppModelInit {
        data_dir: data_dir.to_path_buf(),
    });
}
