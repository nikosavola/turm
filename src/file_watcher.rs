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
    content_sender: Sender<io::Result<String>>,
    receiver: Receiver<()>,
    file_path: PathBuf,
    interval: Duration,
    content: String,
    pos: u64,
    /// Bytes of an incomplete multi-byte UTF-8 sequence carried over from the previous read.
    /// At most 3 bytes (a valid UTF-8 sequence is at most 4 bytes long).
    pending: Vec<u8>,
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
        let (watch_sender, watch_receiver) = unbounded::<()>();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let event = res.unwrap();
            if let notify::EventKind::Modify(ModifyKind::Data(_)) = event.kind {
                watch_sender.send(()).unwrap();
            }
        })
        .unwrap();

        let (mut _content_sender, mut _content_receiver) = unbounded::<io::Result<String>>();
        let (mut _watch_sender, mut _watch_receiver) = unbounded::<()>();
        loop {
            select! {
                recv(self.receiver) -> msg => {
                    match msg? {
                        FileWatcherMessage::FilePath(file_path) => {
                            (_content_sender, _content_receiver) = unbounded();
                            (_watch_sender, _watch_receiver) = unbounded::<()>();

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
                                    let p = p.clone();
                                    move || FileReader::new(_content_sender, _watch_receiver, p, interval).run()
                                });

                                self.watching = watcher.watch(Path::new(&p), RecursiveMode::NonRecursive).is_ok();
                            } else {
                                _content_sender.send(Ok("".to_string())).unwrap();
                            }
                        }
                    }
                }
                recv(watch_receiver) -> _ => { _watch_sender.send(()).unwrap(); }
                recv(_content_receiver) -> msg => {
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
        content_sender: Sender<io::Result<String>>,
        receiver: Receiver<()>,
        file_path: PathBuf,
        interval: Duration,
    ) -> Self {
        FileReader {
            content_sender,
            receiver,
            file_path,
            interval,
            content: "".to_string(),
            pos: 0,
            pending: Vec::new(),
        }
    }

    fn run(&mut self) -> Result<(), ()> {
        loop {
            self.update().map_err(|_| ())?;
            select! {
                recv(self.receiver) -> msg => {
                    msg.map_err(|_| ())?;
                }
                // in case the file watcher doesn't work (e.g. network mounted fs)
                default(self.interval) => {}
            }
        }
    }

    fn update(&mut self) -> Result<(), SendError<io::Result<String>>> {
        let s = File::open(&self.file_path).and_then(|mut f| {
            // Seek to the current position; avoid re-reading the whole file every tick.
            f.seek(io::SeekFrom::Start(self.pos))?;

            // Read raw bytes so invalid UTF-8 never causes an error here.
            let mut raw = Vec::new();
            let bytes_read = f.read_to_end(&mut raw)? as u64;
            self.pos += bytes_read;

            if bytes_read == 0 {
                return Ok(self.content.clone());
            }

            // Prepend any bytes left over from the previous read (an incomplete
            // multi-byte sequence that was split across a read boundary).
            if !self.pending.is_empty() {
                raw.splice(0..0, self.pending.drain(..));
            }

            let decoded = decode_utf8_incremental(&raw, &mut self.pending);
            self.content.push_str(&decoded);
            Ok(self.content.clone())
        });
        // let s = fs::read_to_string(&self.file_path); // alternative: always read the whole file
        self.content_sender.send(s)
    }
}

/// Decode `raw` bytes as UTF-8, carrying any trailing incomplete multi-byte
/// sequence into `pending` for the next call.  Bytes that are genuinely
/// invalid UTF-8 (not merely truncated) are replaced with U+FFFD so the
/// caller never sees an error.
///
/// On entry `pending` must be empty (the caller has already prepended its
/// contents to `raw`).  On return `pending` contains at most 3 bytes.
fn decode_utf8_incremental(raw: &[u8], pending: &mut Vec<u8>) -> String {
    // Find the largest valid UTF-8 prefix.
    let valid_up_to = match std::str::from_utf8(raw) {
        Ok(_) => raw.len(),
        Err(e) => e.valid_up_to(),
    };

    // Everything after `valid_up_to` is either the start of a multi-byte
    // sequence that was truncated at the read boundary, or genuinely invalid
    // bytes.  A valid UTF-8 sequence is at most 4 bytes long, so fewer than 4
    // leftover bytes can still be a legitimate incomplete sequence.  4 or more
    // leftover bytes can never form a single sequence → replace them lossily.
    let remainder = &raw[valid_up_to..];
    if remainder.len() < 4 {
        // Might be an incomplete sequence; stash for the next read.
        pending.extend_from_slice(remainder);
        // SAFETY: we verified `raw[..valid_up_to]` is valid UTF-8 above.
        unsafe { std::str::from_utf8_unchecked(&raw[..valid_up_to]) }.to_owned()
    } else {
        // Genuinely undecodable bytes: emit replacement characters and move on.
        String::from_utf8_lossy(raw).into_owned()
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Split a multibyte character (U+00E9 LATIN SMALL LETTER E WITH ACUTE,
    /// encoded as 0xC3 0xA9) across two reads and verify that both chunks
    /// together produce the correct string with no replacement characters.
    #[test]
    fn test_incremental_utf8_split_multibyte() {
        // U+00E9 = 0xC3 0xA9 (two-byte sequence).
        // Simulate first read: valid ASCII + first byte of the sequence.
        let first_chunk = b"hello \xC3";
        let mut pending = Vec::new();

        let decoded1 = decode_utf8_incremental(first_chunk, &mut pending);
        assert_eq!(decoded1, "hello ");
        // The incomplete byte must be stashed.
        assert_eq!(pending, vec![0xC3]);

        // Simulate second read: prepend pending, then append the rest.
        let second_raw_from_file = b"\xA9 world";
        let mut second_chunk = pending.clone();
        second_chunk.extend_from_slice(second_raw_from_file);
        pending.clear();

        let decoded2 = decode_utf8_incremental(&second_chunk, &mut pending);
        assert_eq!(decoded2, "\u{00E9} world");
        assert!(pending.is_empty());

        // Together the two decoded pieces equal the original string.
        assert_eq!(decoded1 + &decoded2, "hello \u{00E9} world");
    }

    /// Genuinely invalid UTF-8 bytes (not a truncated sequence) are replaced
    /// with U+FFFD replacement characters rather than returning an error.
    #[test]
    fn test_incremental_utf8_invalid_bytes() {
        // 0xFF is never valid in UTF-8.
        let raw = b"abc\xFF\xFEdef";
        let mut pending = Vec::new();
        let decoded = decode_utf8_incremental(raw, &mut pending);
        // The output must contain no panic and must contain the replacement char.
        assert!(decoded.contains('\u{FFFD}'));
        assert!(decoded.contains("abc"));
        assert!(decoded.contains("def"));
    }
}
