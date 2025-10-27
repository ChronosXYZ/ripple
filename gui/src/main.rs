use crate::app::AppModel;
use async_std::task;
use directories::ProjectDirs;
use relm4::RelmApp;
use ripple_core::network;

pub mod app;
mod components;
pub mod state;

mod icon_names {
    pub use shipped::*; // Include all shipped icons by default
    include!(concat!(env!("OUT_DIR"), "/icon_names.rs"));
}

fn main() {
    pretty_env_logger::init();

    let dirs = ProjectDirs::from("", "", "ripple").unwrap();
    let data_dir = dirs.data_dir();

    let (mut client, worker) = network::new(None, data_dir.to_path_buf());

    task::spawn(worker.run());

    task::block_on(client.start_listening("/ip4/0.0.0.0/tcp/34064".parse().unwrap()))
        .expect("listening not to fail");

    state::STATE.write_inner().client = Some(client);
    relm4::RELM_THREADS.set(4).unwrap();

    let app = RelmApp::new("net.pigeoncatcher.ripple");
    relm4_icons::initialize_icons(icon_names::GRESOURCE_BYTES, icon_names::RESOURCE_PREFIX);
    app.run::<AppModel>(());
}
