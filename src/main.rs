mod system_clipboard;

use anyhow;
use colored::*;
use copypasta::{ClipboardContext, ClipboardProvider};
use inquire::Text;
use iroh::base::ticket::Ticket;
use iroh::client::LiveEvent;
use iroh::node::Node;
use iroh::rpc_protocol::ShareMode;
use iroh_net::key::PublicKey;
use log::{debug, error, info};
use tokio;
use tokio_stream::StreamExt;
use tokio_util::task::LocalPoolHandle;

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use system_clipboard::MemClip;

// Uses two threads:
// 1. Main thread manages iroh node, and syncs clipboard contents with remote peers.
// 2. Secondary thread polls clipboard for changes, and alerts the main thread.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut clipboard = ClipboardContext::new().expect(
        "shiiiiiittttt the clipboard didn't work. what the hell goofy ass OS are you running?",
    );
    let memclip_pair = Arc::new((
        Mutex::new(MemClip::new(clipboard.get_contents().unwrap())),
        Condvar::new(),
    )); // In memory var representing the clipboard contents (for syncing).

    let mcp2 = Arc::clone(&memclip_pair);
    let _cb_thread = thread::spawn(move || system_clipboard::watch(clipboard, mcp2));

    // Create an iroh runtime with one worker thread, reusing the tokio runtime.
    // Set up Iroh with in-memory blob and document stores, and start the node.
    let lp = LocalPoolHandle::new(1);
    let blob_store = iroh::bytes::store::mem::Store::default();
    let doc_store = iroh::sync::store::memory::Store::default();
    let node = Node::builder(blob_store, doc_store)
        .local_pool(&lp)
        .spawn()
        .await?;
    let client = node.client();
    info!("{}", "started iroh node".green());

    let mut devices: HashMap<PublicKey, bool> = HashMap::new(); // To store pub keys of other iroh nodes
                                                                // syncing our document. Stores them as
                                                                // bools to represent whether or not those
                                                                // devices are actively syncing the doc (are
                                                                // online. TODO: add some sort of way to
                                                                // verify the device keys through the UI
                                                                // before adding them.

    // Setup the iroh document.
    let author = client
        .authors
        .create()
        .await
        .expect("⭕ 🚌‼ couldn't create an author. HoOh.");
    let doc = client
        .docs
        .create()
        .await
        .expect("oh 🅱uck. couldn't create a document. HooOh.");

    // moment of 🅱ruth. Can we actually write to this document?
    // let blob_id = doc
    //  .set_bytes(author, "memclip", "you look dusty.")
    //    .await
    //    .expect("⭕l' 🚌 couldn't set the bytes! you gotta help ⭕l' 🚌");
    let doc_ticket = doc
        .share(ShareMode::Write)
        .await
        .expect("could not create doc ticket :( booooooo");

    info!(
        "go check out the document dog: {}",
        doc_ticket.serialize().cyan()
    );
    Text::new("Enter ⭕ to continue").prompt();

    // What does the main thread actually need to do yet?
    // 1. Subscribe to updates from the remote document, and update the memclip accordingly.
    //   * The other thread should then automatically detect those changes and update the
    //   clipboard.
    let mut stream = doc.subscribe().await.expect("well I'll 🦧💨. couldn't subrscibe to the document, I guess something done got all 🚌ed 🆙");
    let poll_frequency = Duration::from_secs(1); // Consider updating this.

    debug!("starting iroh remote content event loop");
    loop {
        while let Some(event) = stream.next().await {
            match event {
                Ok(e) => {
                    match e {
                        LiveEvent::InsertRemote { entry, .. } => {
                            // For now we support 69MB 🤙🥴🤙.
                            if entry.key() == "memclip".as_bytes() && entry.content_len() < 72351744
                            {
                                match entry.content_bytes(&doc).await {
                                    Ok(bytes) => {
                                        let (memclip, _cvar) = &*memclip_pair;
                                        let mut mc = memclip.lock().unwrap();
                                        match String::from_utf8(bytes.to_vec()) {
                                            Ok(s) => {
                                                *mc = MemClip::new(s);
                                                debug!(
                                                    "memclip set to remote content: {}",
                                                    entry.content_hash().to_hex().cyan()
                                                )
                                            }
                                            Err(err) => {
                                                error!(
                                                    "error occurred during document sync: {}",
                                                    err.to_string().red()
                                                )
                                            }
                                        }
                                    }
                                    Err(err) => {
                                        error!(
                                            "error occurred during document sync: {}",
                                            err.to_string().red()
                                        )
                                    }
                                }
                            }
                        }

                        // TODO: Do something UI side to allow validating the public keys
                        // of devices, and giving them some kind of user friendly nickname.
                        LiveEvent::NeighborUp(pub_key) => {
                            devices.insert(pub_key, true);
                        }
                        LiveEvent::NeighborDown(pub_key) => {
                            devices
                                .entry(pub_key)
                                .and_modify(|e| *e = false)
                                .or_insert(false); // Man, I miss Python dictionaries.
                        }

                        _ => {} // Default case, we can ignore other events for now.
                    }
                }
                Err(err) => error!(
                    "something went wrong with a {}: {}",
                    "LiveEvent".magenta(),
                    err.to_string().red()
                ),
            }
        }
        // Wait to see if our Condvar receives any notification from the other thread.
        // This feels more efficient than just sleeping 🥴.
        let (memclip, cvar) = &*memclip_pair;
        let mut mc = memclip.lock().unwrap();
        let old_hash = mc.hash;

        let result = cvar
            .wait_timeout(mc, poll_frequency)
            .expect("lock was poisoned 🐍 something got really 🅱ucked 🆙");
        mc = result.0;

        // If the memclip has been updated, sync it to our iroh peers.
        if mc.hash != old_hash {
            let mc_data = mc.data.clone();
            drop(mc); // drop our lock to unblock other thread.
            match doc.set_bytes(author, "memclip", mc_data.as_bytes()).await {
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

    Ok(())
}
