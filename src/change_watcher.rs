use actix_web_actors::ws;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, Debouncer, DebouncedEvent};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedSender};
use actix::AsyncContext;
use actix::StreamHandler;
use notify::{RecursiveMode,  ReadDirectoryChangesWatcher};

pub fn get_folder_watcher(tx: UnboundedSender<Vec<DebouncedEvent>>) -> Debouncer<ReadDirectoryChangesWatcher> {
    // Select recommended watcher for debouncer.
    // Using a callback here, could also be a channel.
    let debouncer = new_debouncer(Duration::from_millis(500), move |res: DebounceEventResult| {
        match res {
            Ok(changes) => { 
                match tx.send(changes) {
                    Ok(()) => {
                        println!("Sent event")
                    },
                    Err(e) => {
                        eprintln!("Error sending file change event: {}", e);
                    }
                }
            },
            Err(e) => {
                eprintln!("File change event error: {}", e);
            }
        }
        
    }).unwrap();

    debouncer
}


#[derive(serde::Serialize)]
struct FileChange {
    path: String,
}

#[derive(serde::Serialize)]
struct FileChanges {
    changes: Vec<FileChange>,
}

pub struct WatcherWsActor {
    pub rx: mpsc::UnboundedReceiver<Vec<DebouncedEvent>>,
    pub debouncer: Debouncer<ReadDirectoryChangesWatcher>
}

impl actix::Actor for WatcherWsActor {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // Start sending file change events to the WebSocket client
        ctx.run_interval(Duration::from_secs(1), |act, ctx| {
            if let Ok(events) = act.rx.try_recv() {
                let changes_vec: Vec<FileChange> = events.into_iter()
                    .filter_map(|e| e.path.to_str()
                    .map(|s| FileChange{ path: String::from(s)})).collect();
                let changes = FileChanges {changes: changes_vec};
                let string_data = serde_json::to_string(&changes).unwrap();
                ctx.text(string_data);
            }
        });
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct WatchFolder {
    watch: bool,
    recursive: bool,
    path: String,
}
//{"watch": true, "path": "Q:\\"}
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WatcherWsActor {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(msg)) => {
                let Ok(watch_command) = serde_json::from_slice::<WatchFolder>(&msg.as_bytes())
                    else {
                        eprintln!("Error: Invalid watcher command: {}", msg);
                        return
                    };

                if watch_command.watch {
                    let mode = if watch_command.recursive {RecursiveMode::Recursive} else { RecursiveMode::NonRecursive};
                    if let Err(e) = self.debouncer.watcher().watch(&PathBuf::from(watch_command.path.clone()), mode) {
                        eprintln!("Error watching folder ({}): {}", watch_command.path.clone(), e)
                    }
                } else {
                    if let Err(e) = self.debouncer.watcher().unwatch(&PathBuf::from(watch_command.path.clone())) {
                        eprintln!("Error unwatching folder ({}): {}", watch_command.path, e)
                    } 
                }
                ()
            },
            _ => (),
        }
    }
}
