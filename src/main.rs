mod system_clipboard;

use anyhow;
use copypasta::{ClipboardContext, ClipboardProvider};
use inquire::Text;
use iroh::node::Node;
use iroh::rpc_protocol::ShareMode;
use rand::{rngs::OsRng, Rng};
use tokio::{self, loom::std::sync::Condvar};
use tokio_util::task::LocalPoolHandle;

use std::sync::{Arc, Mutex};
use std::thread;

// Uses two threads:
// 1. Main thread manages iroh node, and syncs clipboard contents with remote peers.
// 2. Secondary thread polls clipboard for changes, and alerts the main thread.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut rng = OsRng;
    let mut clipboard = ClipboardContext::new().expect(
        "shiiiiiittttt the clipboard didn't work. what the hell goofy ass OS are you running?",
    );
    let memclip_pair = Arc::new((
        Mutex::new(clipboard.get_contents().unwrap()),
        Condvar::new(),
    )); // In memory var representing the clipboard contents (for syncing).

    let mcp2 = Arc::clone(&memclip_pair);
    let cb_thread = thread::spawn(move || system_clipboard::watch(clipboard, mcp2));

    // Create an iroh runtime with one worker thread, reusing the tokio runtime. ?
    let lp = LocalPoolHandle::new(1);

    // Set up Iroh with in-memory blob and document stores, and start the node.
    let blob_store = iroh::bytes::store::mem::Store::default();
    let doc_store = iroh::sync::store::memory::Store::default();
    let node = Node::builder(blob_store, doc_store)
        .local_pool(&lp)
        .spawn()
        .await?;
    let client = node.client();

    // Create the document.
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
    let blob_id = doc
        .set_bytes(author, "memclip")
        .await
        .expect("⭕l' 🚌 couldn't set the bytes! you gotta help ⭕l' 🚌");
    let doc_ticket = doc
        .share(ShareMode::Write)
        .await
        .expect("could not create doc ticket :( booooooo");

    println!("go check out the document dog: {}", doc_ticket);
    Text::new("Enter ⭕ to continue").prompt();

    // Useful info for syncing an existing document:
    // https://docs.rs/iroh-sync/latest/iroh_sync/net/fn.connect_and_sync.html.
    Ok(())
}
