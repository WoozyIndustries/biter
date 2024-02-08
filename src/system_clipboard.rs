use copypasta::{ClipboardContext, ClipboardProvider};
use log::debug;
use twox_hash::xxh3;

use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

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

/// Loops indefinitely, checking the system clipboard for changes.
/// This sucks. Swap this out for something that's not polling (unfortunately that
/// will take a lot of reading to accomplish, but perhaps you can make a PR into
/// some of the project's dependency libraries). The memclip part of the memclip pair
/// param is used to check whether a remote peer updated the iroh document, and update
/// the system clipboard accordingly.
pub fn watch(
    mut clipboard: ClipboardContext,
    memclip_pair: Arc<(Mutex<MemClip>, Condvar)>,
) -> String {
    let wait_time = Duration::from_secs(1); // Wait 1 second between cb checks. Perhaps lower this.
    let mut data = clipboard.get_contents().unwrap();
    let mut clip_hash = xxh3::hash64(data.as_bytes());

    loop {
        thread::sleep(wait_time);
        let (memclip, cvar) = &*memclip_pair;
        let mut mc = memclip.lock().unwrap();

        let old_hash = clip_hash;
        data = clipboard.get_contents().unwrap();
        clip_hash = xxh3::hash64(data.as_bytes());

        // If the clipboard content is new, and not sent from a remote device.
        if clip_hash != old_hash && clip_hash != mc.hash {
            *mc = MemClip::new(data); // Update the synced clipboard with data from system clip.
            cvar.notify_one(); // Notify the main thread that memclip was changed.
            debug!("memclip synced with clipboard change: twox={}", clip_hash);
        } else if clip_hash == old_hash && clip_hash != mc.hash {
            // If the clipboard isn't new, but the memclip has been changed:
            clipboard.set_contents((*mc.data).to_string()).unwrap();
            debug!(
                "system clipboard synced from remote change: twox={}",
                mc.hash
            );
        }

        drop(mc); // drop our lock while we sleep.
    }
}
