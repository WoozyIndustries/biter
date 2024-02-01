use copypasta::{ClipboardContext, ClipboardProvider};
use twox_hash::xxh3;

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Loops indefinitely, checking the system clipboard for changes.
/// This sucks. Swap this out for something that's not polling (unfortunately that
/// will take a lot of reading to accomplish, but perhaps you can make a PR into
/// some of the project's dependency libraries).
/// The clip_state param is used to check whether a remote peer updated the iroh
/// document, and update the system clipboard accordingly.
pub fn watch(clipboard: ClipboardContext, synced_clip: Arc<Mutex<u64>>) -> String {
    let wait_time = 1;
    let mut data = clipboard.get_contents().unwrap();
    let mut hash = xxh3::hash64(data.as_bytes());
    println!("the hash of the clipboard contents: {}", data);

    loop {
        thread::sleep(Duration::from_secs(wait_time));

        // implement checking clip state here
        let mut sc = synced_clip.lock().unwrap();
        let sync_clip_hash = xxh3::hash64(sc.as_bytes());

        let old_hash = hash;
        data = clipboard.get_contents().unwrap();
        hash = xxh3::hash64(data.as_bytes());

        // If the clipboard content is new, and not sent from a remote device, .
        if hash != old_hash && hash != sync_clip_hash {
            *sc = data; // Update the synced clipboard with data from system clip.
        }
    }
}
