use colored::*;
use iroh::client::Iroh;
use iroh::sync::{AuthorId, NamespaceId};
use log::{debug, error};

use std::sync::{Arc, Condvar, Mutex};

use crate::system_clipboard::MemClip;

/// Wait for our Condvar to be notified from the clipboard monitoring thread, and
/// sync memclip to the remote iroh doc.
pub async fn wait_on_memclip<C>(
    client: Iroh<C>,
    author: AuthorId,
    doc_id: NamespaceId,
    memclip_pair: Arc<(Mutex<MemClip>, Condvar)>,
) {
    debug!("conditional thread waiting for events...");

    let doc = match client.docs.open(doc_id).expect(
        "oh fuuuuuuuc, something went wrong trying to get a handle to doc {}",
        doc_id.fmt_short().red(),
    ) {
        Some(d) => d,
        None => panic!(
            "hwhat in DU_Tnation, there ain't no document by the name of {}",
            doc_id.fmt_short().red()
        ),
    };

    let (memclip, cvar) = &*memclip_pair;
    loop {
        let mut mc = memclip.lock().unwrap();
        let old_hash = mc.hash;

        // This function will release the lock until it's notified.
        mc = cvar
            .wait(mc)
            .expect("lock was poisoned 🐍 something got really 🅱ucked 🆙");

        // If the memclip has been updated, sync it to our iroh peers.
        // It's important to check this condition each time due to potential spurious
        // wakeups. See https://doc.rust-lang.org/std/sync/struct.Condvar.html#method.wait.
        if mc.hash != old_hash {
            debug!("memclip has been updated, syncing to iroh document...");

            let owned_copy = mc.data.to_owned();
            drop(mc); // Drop our lock to unblock other threads.
            match doc.set_bytes(author, "memclip", owned_copy).await {
                Ok(blob_id) => debug!(
                    "synced blob {} from the system clipboard",
                    blob_id.to_hex().cyan()
                ),
                Err(err) => error!(
                    "something went wrong trying to update the remote doc: {}",
                    err.to_string().red()
                ),
            }
        }
    }
}
