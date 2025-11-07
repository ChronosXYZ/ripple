use log::info;
use num_bigint::BigUint;
use sha2::{Digest, Sha512};
use tokio::{
    sync::{mpsc, oneshot},
    task,
};

pub struct AsyncPoW {}

impl AsyncPoW {
    pub fn do_pow(target: BigUint, initial_hash: Vec<u8>) -> oneshot::Receiver<(BigUint, BigUint)> {
        let (mut sender, receiver) = oneshot::channel();
        let (result_sender, mut result_receiver) = mpsc::channel(1);

        let mut workers = Vec::new();
        let num_of_cores = num_cpus::get(); // TODO make this setting configurable

        for i in 0..num_of_cores {
            let t = target.clone();
            let ih = initial_hash.clone();
            let result_sender = result_sender.clone();
            let (term_tx, mut term_rx) = oneshot::channel();
            task::spawn_blocking(move || {
                info!("PoW has started");

                let mut nonce: BigUint = BigUint::from(i);
                let mut trial_value = BigUint::parse_bytes(b"99999999999999999999", 10).unwrap();
                while trial_value > t && term_rx.try_recv().is_err() {
                    nonce += num_of_cores;
                    let result_hash = Sha512::digest(Sha512::digest(
                        [nonce.to_bytes_be().as_slice(), ih.as_slice()].concat(),
                    ));
                    trial_value = BigUint::from_bytes_be(&result_hash[0..8]);
                }

                let _ = result_sender.blocking_send((trial_value, nonce));

                info!("PoW has ended");
            });
            workers.push(term_tx);
        }

        task::spawn(async move {
            tokio::select! {
                _ = sender.closed() => {
                    log::debug!("cancelling workers");
                    for w in workers.into_iter() {
                        _ = w.send(());
                    }
                    result_receiver.close();
                    return;
                },
                result = result_receiver.recv() => {
                    if let Some(res) = result {
                        log::debug!("cancelling workers");
                        for w in workers.into_iter() {
                            _ = w.send(());
                        }
                        sender.send(res).expect("receiver not to be dropped");
                    }
                }
            }
        });
        receiver
    }
}
