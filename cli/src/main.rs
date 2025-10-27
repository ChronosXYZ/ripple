use std::{error::Error, path::PathBuf};

use async_std::task;
use clap::Parser;
use ripple_core::network;
use signal_hook::{
    consts::{SIGINT, SIGTERM},
    iterator::Signals,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    data_dir: String,

    #[arg(short, long, default_value_t = String::from("0.0.0.0"))]
    ip: String,

    #[arg(short, long, default_value_t = 34064)]
    port: u16,
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    let args = Args::parse();

    log::debug!("a");
    let (mut client, worker) = network::new(None, PathBuf::from(args.data_dir));

    task::spawn(worker.run());

    client
        .start_listening(
            format!("/ip4/{}/tcp/{}", args.ip, args.port)
                .parse()
                .unwrap(),
        )
        .await
        .expect("listening not to fail");

    log::info!("node has started successfully!");

    let mut signals = Signals::new(&[SIGTERM, SIGINT])?;
    for sig in signals.forever() {
        log::debug!("Received signal {:?}", sig);
        client.shutdown();
        return Ok(());
    }

    Ok(())
}
