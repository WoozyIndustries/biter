mod memclip;
mod system_clip;

use anyhow;
use colored::*;
use copypasta::{ClipboardContext, ClipboardProvider};
use iroh::base::ticket::Ticket;
use iroh::client::LiveEvent;
use iroh::net::key::PublicKey;
use iroh::node::Node;
use iroh::rpc_protocol::{Hash, ShareMode};
use iroh::sync::ContentStatus;
use iroh::ticket::DocTicket;
use log::{debug, error, info};
use structopt::StructOpt;
use tokio;
use tokio_stream::StreamExt;
use tokio_util::task::LocalPoolHandle;

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use memclip::MemClip;

// Glossary
// --------------------------------------------------------------------------------
// * iroh document: see https://iroh.computer/docs/layers/documents.
//
// * memclip: an in-memory `String` for keeping track of clipboard changes between
//            our system and the remote clipboard (iroh document).
//

// Design
// --------------------------------------------------------------------------------
// Uses three threads:
//
// 1. Main thread manages iroh node, and subscribes to events on an iroh doc:
//    * Keeps track of peers we are syncing with.
//    * Updates our memclip (in-memory clipboard for syncing between threads) when
//      peers write to it.
//
// 2. Clipboard thread (`cb_thread`) watches for changes to the system clipboard
//    and syncs them to our memclip. It then notifies the `cv_thread` via a `Condvar`.
//
// 3. The conditional thread (`cv_thread`) waits for notifications on a `Condvar`.
//    When notified, the memclip is checked and the data is synced to the iroh doc
//    if it is actually new.
//    See https://doc.rust-lang.org/std/sync/struct.Condvar.html for more info.
//

/// Command line options.
#[derive(StructOpt, Debug)]
#[structopt(name = "biter", about = "biter: copier, follower")]
struct Opt {
    #[structopt(subcommand)]
    cmd: SubCommand,
}

/// CLI sub-commands.
#[derive(StructOpt, Debug)]
enum SubCommand {
    #[structopt(
        name = "start",
        about = "start a new biter session" // TODO: implement a rejoin feature for a previous sesh.
    )]
    Start(StartOptions),

    #[structopt(name = "join", about = "join an existing biter sesh")]
    Join(JoinOptions),
}

/// CLI options for `SubCommand::Start`.
#[derive(StructOpt, Debug)]
struct StartOptions {
    // Not yet implemented
    #[structopt(short = "n", long, help = "force creation of a new biter session")]
    new: bool,
}

/// CLI options for `SubCommand::Join`.
#[derive(StructOpt, Debug)]
struct JoinOptions {
    #[structopt(help = "an iroh doc ticket for an existing session")]
    ticket: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // RUST_LOG=biter=debug
    env_logger::init();
    let args = Opt::from_args();

    let mut clipboard = ClipboardContext::new().expect(
        "shiiiiiittttt the clipboard didn't work. what the hell goofy ass OS are you running?",
    );
    let memclip_pair = Arc::new((
        Mutex::new(MemClip::new(clipboard.get_contents().unwrap())),
        Condvar::new(),
    )); // In memory var representing the clipboard contents (for syncing).

    // Start the clipboard thread.
    let mc2 = Arc::clone(&memclip_pair);
    let _cb_thread = thread::spawn(|| system_clipboard::watch(clipboard, mc2));

    // Create an iroh runtime with one worker thread, reusing the tokio runtime.
    // Set up Iroh with in-memory blob and document stores, and start the node.
    info!("starting iroh node...");
    let lp = LocalPoolHandle::new(1);
    let blob_store = iroh::bytes::store::mem::Store::default();
    let doc_store = iroh::sync::store::memory::Store::default();
    let node = Node::builder(blob_store, doc_store)
        .local_pool(&lp)
        .spawn()
        .await?;
    let client = node.client();
    info!("{}", "started iroh node".green());

    // Not really used yet.
    let mut devices: HashMap<PublicKey, bool> = HashMap::new(); // To store pub keys of other iroh nodes
                                                                // syncing our document. Stores them as
                                                                // bools to represent whether or not those
                                                                // devices are actively syncing the doc (are
                                                                // online. TODO: add some sort of way to
                                                                // verify the device keys through the UI
                                                                // before adding them.

    // Initialize author info. TODO: persist this after shutdown.
    let author = client
        .authors
        .create()
        .await
        .expect("⭕ 🚌‼ couldn't create an author. HoOh.");
    info!(
        "your device key is: {}; use this to verify when syncing with other devices",
        author.fmt_short().magenta()
    );

    // Setup the iroh document.
    // Note about doc tickets: doc tickets contain lists of peers to join. This
    // makes me think that for persisting biter sessions we actually want to use
    // NamespaceIds, and not the doc tickets. Yeah actually that makes a lot more
    // sense. Oh shit, I think I actually want to create a new doc ticket every time
    // a new peer joins 🤯 (since the ticket contains a list of nodes, see:
    // https://docs.rs/iroh/latest/iroh/ticket/struct.DocTicket.html#).
    let (doc, doc_ticket) = match args.cmd {
        SubCommand::Start(opt) => {
            let d = client
                .docs
                .create()
                .await
                .expect("oh 🅱uck. couldn't create a document. HooOh.");
            let dt = d
                .share(ShareMode::Write)
                .await
                .expect("could not create doc ticket :( booooooo");

            info!(
                "created new iroh document for clipboard sync; ticket: {}",
                dt.serialize().cyan()
            );
            (d, dt)
        }

        // In the case of joining an existing session, we create a new doc ticket
        // to have a ticket with an updated list of nodes.
        SubCommand::Join(opt) => {
            let dt = DocTicket::from_str(&opt.ticket).expect("invalid doc ticket 🦧💨");
            let d = client
                .docs
                .import(dt)
                .await
                .expect("oh 🅱uck. couldn't join that biter sesh 🤙🥴🤙‼");
            let new_dt = d
                .share(ShareMode::Write)
                .await
                .expect("could not update doc ticket :( booooooo");

            info!(
                "joined iroh document {} for clipboard sync; updated doc ticket: {}",
                d.id().fmt_short().cyan(),
                new_dt.serialize().cyan()
            );
            (d, new_dt)
        }
    };

    // Initialize and start the conditional variable thread.
    let mc3 = Arc::clone(&memclip_pair);
    let a2 = author.clone();
    let c2 = client.clone();
    let doc_id = doc.id();
    let _cv_thread = tokio::spawn(async move {
        memclip::wait_for_updates(c2, a2, doc_id, mc3).await;
    });

    let mut stream = doc.subscribe().await.expect("well I'll 🦧💨. couldn't subrscibe to the document, I guess something done got all 🚌ed 🆙");

    debug!("starting iroh remote content event loop");
    let mut watch_for_ready: Option<Hash> = None;
    while let Some(event) = stream.next().await {
        debug!("iroh event: {:?}", event);
        match event {
            Ok(e) => {
                match e {
                    LiveEvent::InsertRemote {
                        from,
                        entry,
                        content_status,
                    } => {
                        // For now we support 69MB 🤙🥴🤙.
                        if entry.key() == "memclip".as_bytes() && entry.content_len() < 72351744 {
                            debug!(
                                "new memclip entry from {} with content hash: {}",
                                entry.author().fmt_short().cyan(),
                                entry.content_hash().to_hex().cyan()
                            );

                            match content_status {
                                // If content isn't ready, store its hash so we know to look for
                                // it's completion event. In the case of a clipboard we only need
                                // the most recent addition.
                                ContentStatus::Incomplete | ContentStatus::Missing => {
                                    watch_for_ready = Some(entry.content_hash())
                                }

                                // If the content is ready, well, go ahead and download that 🅱oy 🤙🥴🤙‼
                                ContentStatus::Complete => match entry.content_bytes(doc).await {
                                    Ok(bytes) => {}
                                    Err(err) => {
                                        error!(
                                            "error occurred during document sync: {}",
                                            err.to_string().red()
                                        )
                                    }
                                },
                            };
                        }
                    }

                    // TODO: verify these events always happen in order.
                    LiveEvent::ContentReady { hash } => {
                        if let Some(h) = watch_for_ready {
                            if hash == h {
                                match client.blobs.read_to_bytes(hash).await {
                                    Ok(bytes) => {
                                        let (memclip, _cvar) = &*memclip_pair;
                                        let mut mc = memclip.lock().unwrap();
                                        match String::from_utf8(bytes.to_vec()) {
                                            Ok(s) => {
                                                *mc = MemClip::new(s);
                                                debug!(
                                                    "memclip set to remote content: {}",
                                                    hash.to_hex().cyan()
                                                )
                                            }
                                            Err(err) => {
                                                error!(
                                                    "error occurred during document sync (string conversion): {}",
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
                    }

                    // TODO: Do something UI side to allow validating the public keys
                    // of devices, and giving them some kind of user friendly nickname.
                    LiveEvent::NeighborUp(pub_key) => {
                        devices.insert(pub_key, true);
                        info!(
                            "new peer device joined document with public key: {}",
                            pub_key.fmt_short().cyan()
                        );
                    }
                    LiveEvent::NeighborDown(pub_key) => {
                        devices
                            .entry(pub_key)
                            .and_modify(|e| *e = false)
                            .or_insert(false); // Man, I miss Python dictionaries.
                        info!(
                            "peer device left the document: {}",
                            pub_key.fmt_short().cyan()
                        );
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

    Ok(())
}
