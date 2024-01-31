mod system_clipboard;

use anyhow;
use copypasta::{ClipboardContext, ClipboardProvider};
use iroh::node::Node; // For running an iroh node.
use iroh_bytes; // For iroh blobs/collections handling.
use iroh_sync; // For iroh Documents handling.
use tokio;
use tokio_util::task::LocalPoolHandle;

use std::sync::{Arc, Mutex};
use std::thread;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let clipboard = ClipboardContext::new().expect(
        "shiiiiiittttt the clipboard didn't work. what the hell goofy ass OS are you running?",
    );
    let synced_clip = Arc::new(Mutex::new(clipboard.get_contents().unwrap())); // XXHash representing the state of the clipboard.

    let cb2 = Arc::clone(&cb);
    let sc2 = Arc::clone(&clip_state);
    let cb_thread = thread::spawn(move || system_clipboard::watch(cb2, cs2));

    // Create an iroh runtime with one worker thread, reusing the tokio runtime. ?
    let lp = LocalPoolHandle::new(1);

    // Set up Iroh with in-memory blob and document stores, and start the node.
    let blob_store = iroh_bytes::store::mem::Store::default();
    let doc_store = iroh_sync::store::memory::Store::default();
    let node = Node::builder(doc_store).local_pool(&lp).spawn().await?;

    // figure out how tf to create a document, and then set something in it
    // and then try using just the iroh cli to retrieve and write to the document.
    // ya fuck
}
