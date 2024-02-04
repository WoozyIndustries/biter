use copypasta::{ClipboardContext, ClipboardProvider};
use tokio::loom::std::sync::Condvar;
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
pub fn watch(
    mut clipboard: ClipboardContext,
    memclip_pair: Arc<Mutex<((String, Condvar))>>,
) -> String {
    let wait_time = Duration::from_secs(1); // Wait 1 second between cb checks. Perhaps lower this.
    let mut data = clipboard.get_contents().unwrap();
    let mut hash = xxh3::hash64(data.as_bytes());

    loop {
        thread::sleep(wait_time);
        let (mem_clip, cvar) = &*memclip_pair;
        let mut mc = memclip_pair.lock().unwrap();
        let mc_hash = xxh3::hash64(mc.as_bytes());

        let old_hash = hash;
        data = clipboard.get_contents().unwrap();
        clip_hash = xxh3::hash64(data.as_bytes());

        // If the clipboard content is new, and not sent from a remote device, .
        if clip_hash != old_hash && clip_hash != mc_hash {
            *mc = data; // Update the synced clipboard with data from system clip.
            cvar.notify_one(); // Notify the main thread that mem_clip was changed.
        }
    }
}
