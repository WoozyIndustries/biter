use colored::*;
use iroh::client::Iroh;
use iroh::rpc_protocol::ProviderService;
use iroh::sync::{AuthorId, NamespaceId};
use log::{debug, error};
use quic_rpc::ServiceConnection;
use thiserror::Error;

use std::sync::{Arc, Condvar, Mutex};

#[derive(Error, Debug)]
pub enum MemClipError {}

#[derive(Clone)]
pub struct MemClip {
    pub hash: u64,
    pub data: String,
}

impl MemClip {
    pub fn new(data_string: String) -> MemClip {
        MemClip {
            hash: xxh3::hash64(data_string.as_bytes()),
            data: data_string,
        }
    }
}

/// Set the value of the memclip from `Bytes` data.
async fn set_bytes(bytes: Bytes, memclip_pair: &Arc<(Mutex<MemClip>, Condvar)>) -> Result<Error> {
    let (memclip, _cvar) = *memclip_pair;
    let mut mc = memclip.lock().unwrap();
    match String::from_utf8(bytes.to_vec()) {
        Ok(s) => {
            *mc = MemClip::new(s);
            debug!("memclip set to: {  }", bytes.on_purple())
        }
        Err(err) => {
            error!(
                "error occurred during document sync (string conversion): {}",
                err.to_string().red()
            )
        }
    }
}

/// Dog, just be patient.
/// Waits for a notification on the condvar, locks the memclip mutex, and returns
/// a copy of the data inside, as well as the hash of the data prior to waiting on
/// the Condvar (see its use in `wait_on_memclip`).
fn chill(memclip: &Mutex<MemClip>, cvar: &Condvar) -> (MemClip, u64) {
    let mut mc = memclip.lock().unwrap();
    let old_hash = mc.hash;

    // This function will release the lock until it's notified.
    mc = cvar
        .wait(mc)
        .expect("lock was poisoned 🐍 something got really 🅱ucked 🆙");

    (mc.clone(), old_hash)
}

/// Wait for our Condvar to be notified from the clipboard monitoring thread, and
/// sync memclip to the remote iroh doc.
pub async fn wait_for_updates<C: ServiceConnection<ProviderService>>(
    client: Iroh<C>,
    author: AuthorId,
    doc_id: NamespaceId,
    memclip_pair: Arc<(Mutex<MemClip>, Condvar)>,
) {
    debug!("conditional thread waiting for events...");

    let doc = match client
        .docs
        .open(doc_id)
        .await
        .expect("oh fuuuuuuuc, something went wrong trying to get a handle to the doc")
    {
        Some(d) => d,
        None => panic!(
            "hwhat in DU_Tnation, there ain't no document by the name of {}",
            doc_id.fmt_short().red()
        ),
    };

    let (memclip, cvar) = &*memclip_pair;
    loop {
        let (mc, old_hash) = chill(memclip, cvar);

        // If the memclip has been updated, sync it to our iroh peers.
        // It's important to check this condition each time due to potential spurious
        // wakeups. See https://doc.rust-lang.org/std/sync/struct.Condvar.html#method.wait.
        if mc.hash != old_hash {
            debug!("memclip has been updated, syncing to iroh document...");

            match doc.set_bytes(author, "memclip", mc.data).await {
                Ok(blob_id) => debug!(
                    "synced blob {} from the system clipboard",
                    blob_id.to_hex().cyan()
                ),
                Err(err) => error!(
                    "something went wrong trying to update the remote doc: {}",
                    err.to_string().red()
                ),
            };
        }
    }
}
