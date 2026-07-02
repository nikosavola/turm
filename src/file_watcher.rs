use std::{
    fmt,
    fs::File,
    io::{self, Read, Seek},
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use crossbeam::{
    channel::{Receiver, RecvError, SendError, Sender, unbounded},
    select,
};
use notify::{RecursiveMode, Watcher, event::ModifyKind};

use crate::app::AppMessage;

struct FileReader {
    content_tx: Sender<io::Result<String>>,
    kick_rx: Receiver<()>,
    file_path: PathBuf,
    interval: Duration,
    content: String,
    pos: u64,
}

/// Channels connecting the watcher to the currently active `FileReader`
/// thread, or standing in for one when no file is selected.
///
/// A `FileReader` thread is handed a clone of `content_tx` (to report file
/// content back) and a clone of `kick_rx` (to be told when to re-read).
/// `content_rx` and `kick_tx` are the watcher's own ends of those same
/// channels. Replacing a `ReaderConnection` drops its `content_rx` and
/// `kick_tx`: with no receiver left, the old thread's `content_tx.send`
/// starts failing, and with no sender left, its `kick_rx.recv` starts
/// failing too — either is enough to make the orphaned thread exit on its
/// next iteration.
struct ReaderConnection {
    content_tx: Sender<io::Result<String>>,
    content_rx: Receiver<io::Result<String>>,
    kick_tx: Sender<()>,
    kick_rx: Receiver<()>,
}

impl ReaderConnection {
    /// Creates a fresh, unconnected pair of channels. Used both as the
    /// initial placeholder (before any file is selected) and to sever the
    /// previous connection whenever the watched file changes: `select!`
    /// always needs a valid `content_rx` to recv on, even when nothing is
    /// currently being read.
    fn new() -> Self {
        let (content_tx, content_rx) = unbounded();
        let (kick_tx, kick_rx) = unbounded();
        ReaderConnection {
            content_tx,
            content_rx,
            kick_tx,
            kick_rx,
        }
    }
}

struct FileWatcher {
    app: Sender<AppMessage>,
    receiver: Receiver<FileWatcherMessage>,
    file_path: Option<PathBuf>,
    watching: bool, // Whether notify watch was successfully started for file_path
    interval: Duration,
}
pub enum FileWatcherMessage {
    FilePath(Option<PathBuf>),
}

pub struct FileWatcherHandle {
    sender: Sender<FileWatcherMessage>,
    file_path: Option<PathBuf>,
}

pub enum FileWatcherError {
    File(io::Error),
}

impl fmt::Display for FileWatcherError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FileWatcherError::File(e) => write!(f, "Read error: {}", e),
        }
    }
}

impl FileWatcher {
    fn new(
        app: Sender<AppMessage>,
        receiver: Receiver<FileWatcherMessage>,
        interval: Duration,
    ) -> Self {
        FileWatcher {
            app,
            receiver,
            file_path: None,
            watching: false,
            interval,
        }
    }

    fn run(&mut self) -> Result<(), RecvError> {
        let (notify_tx, notify_rx) = unbounded::<()>();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let event = res.unwrap();
            if let notify::EventKind::Modify(ModifyKind::Data(_)) = event.kind {
                notify_tx.send(()).unwrap();
            }
        })
        .unwrap();

        let mut reader = ReaderConnection::new();
        loop {
            select! {
                recv(self.receiver) -> msg => {
                    match msg? {
                        FileWatcherMessage::FilePath(file_path) => {
                            // Dropping the old connection here is what stops the
                            // previous FileReader thread (see ReaderConnection's
                            // doc comment).
                            reader = ReaderConnection::new();

                            if self.watching {
                                let p = self.file_path.as_ref().expect("Inconsistent state");
                                watcher.unwatch(p).unwrap_or_else(|_| panic!("Failed to unwatch {:?}", p));
                            }
                            self.file_path = None;
                            self.watching = false;

                            if let Some(p) = file_path {
                                self.file_path = Some(p.clone());

                                let interval = self.interval;
                                thread::spawn({
                                    let content_tx = reader.content_tx.clone();
                                    let kick_rx = reader.kick_rx.clone();
                                    let p = p.clone();
                                    move || FileReader::new(content_tx, kick_rx, p, interval).run()
                                });

                                self.watching = watcher.watch(Path::new(&p), RecursiveMode::NonRecursive).is_ok();
                            } else {
                                reader.content_tx.send(Ok("".to_string())).unwrap();
                            }
                        }
                    }
                }
                recv(notify_rx) -> _ => { reader.kick_tx.send(()).unwrap(); }
                recv(reader.content_rx) -> msg => {
                    let res = msg.unwrap();
                    // If we don't have a file watch yet but the file now reads OK, try enabling watch
                    if !self.watching {
                        if let (Ok(_), Some(p)) = (&res, &self.file_path) {
                            self.watching = watcher.watch(Path::new(p), RecursiveMode::NonRecursive).is_ok();
                        }
                    }
                    self.app
                        .send(AppMessage::JobOutput(res.map_err(FileWatcherError::File)))
                        .unwrap();
                }
            }
        }
    }
}

impl FileReader {
    fn new(
        content_tx: Sender<io::Result<String>>,
        kick_rx: Receiver<()>,
        file_path: PathBuf,
        interval: Duration,
    ) -> Self {
        FileReader {
            content_tx,
            kick_rx,
            file_path,
            interval,
            content: "".to_string(),
            pos: 0,
        }
    }

    fn run(&mut self) -> Result<(), ()> {
        loop {
            self.update().map_err(|_| ())?;
            select! {
                recv(self.kick_rx) -> msg => {
                    msg.map_err(|_| ())?;
                }
                // in case the file watcher doesn't work (e.g. network mounted fs)
                default(self.interval) => {}
            }
        }
    }

    fn update(&mut self) -> Result<(), SendError<io::Result<String>>> {
        let s = File::open(&self.file_path).and_then(|mut f| {
            // avoid reading the whole file every time
            self.pos = f.seek(io::SeekFrom::Start(self.pos))?;
            self.pos += f.read_to_string(&mut self.content)? as u64;
            Ok(self.content.clone())
        });
        // let s = fs::read_to_string(&self.file_path); // alternative: always read the whole file
        self.content_tx.send(s)
    }
}

impl FileWatcherHandle {
    pub fn new(app: Sender<AppMessage>, interval: Duration) -> Self {
        let (sender, receiver) = unbounded();
        let mut actor = FileWatcher::new(app, receiver, interval);
        thread::spawn(move || actor.run());

        Self {
            sender,
            file_path: None,
        }
    }

    pub fn set_file_path(&mut self, file_path: Option<PathBuf>) {
        if self.file_path != file_path {
            self.file_path = file_path.clone();
            self.sender
                .send(FileWatcherMessage::FilePath(file_path))
                .unwrap();
        }
    }
}
