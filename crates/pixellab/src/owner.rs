//! Owning a document (docs/intents/2026-10-autosave/spec.md, D9): while Pixel Lab has a document
//! open, its server (`serve --stdio`) is the one process that writes it. It holds
//! `owners/<key>.lock` in the state dir with an OS file lock, which the system lets go of when
//! the process ends however it ends (R25), and listens on a Unix socket beside it. The CLI and MCP
//! servers find it through the lock (client.rs) and send it their commands.
//!
//! ```text
//! owners/<key>.lock   { pid, appPid, document, socket, protocol, version, startedAt }, held by the owner
//! owners/<key>.sock   the owner's socket (0600; owners/ is 0700), or $TMPDIR/pixellab-<key>.sock
//! ```
//!
//! A lock that isn't held says nothing, whatever it holds: a crash leaves it and its socket behind,
//! and the next owner takes it over. Only the user's own processes may connect (R27).

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use engine::format::stringify;
use serde_json::{json, Value};

use crate::history::key_of;
use crate::state::{iso_time, now_ms, real_path};

/// The version of the conversation on the socket: a client of another one can't edit through it (R28).
pub const PROTOCOL: i64 = 1;

/// The longest socket path macOS takes (`sun_path` is 104 bytes, the last a NUL).
const SOCKET_PATH_MAX: usize = 100;

/// The owners folder of a state dir, made private to the user.
pub fn owners_dir(state_dir: &Path) -> PathBuf {
    state_dir.join("owners")
}

/// The lock file of a document.
pub fn lock_path(state_dir: &Path, root: &Path) -> PathBuf {
    owners_dir(state_dir).join(format!("{}.lock", key_of(root)))
}

/// Where the owner of a document listens: beside its lock, or in `$TMPDIR` when that path is
/// longer than a socket path may be.
pub fn socket_path(state_dir: &Path, root: &Path) -> PathBuf {
    let key = key_of(root);
    let beside = owners_dir(state_dir).join(format!("{key}.sock"));
    if beside.as_os_str().len() <= SOCKET_PATH_MAX {
        return beside;
    }
    let tmp = std::env::var_os("TMPDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
    tmp.join(format!("pixellab-{key}.sock"))
}

fn open_lock(path: &Path) -> std::io::Result<File> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    }
    OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path)
}

fn read_all(file: &mut File) -> String {
    let mut text = String::new();
    let _ = file.seek(SeekFrom::Start(0));
    let _ = file.read_to_string(&mut text);
    text
}

/// Who owns a document now: the lock's contents while someone holds it, None when no one does.
pub fn holder(state_dir: &Path, root: &Path) -> Option<Value> {
    let path = lock_path(state_dir, root);
    if !path.exists() {
        return None;
    }
    let mut file = open_lock(&path).ok()?;
    match file.try_lock() {
        Ok(()) => {
            let _ = file.unlock();
            None
        }
        Err(_) => {
            // The owner writes its contents just after it takes the lock: wait a moment for them.
            for _ in 0..50 {
                if let Ok(v) = serde_json::from_str::<Value>(&read_all(&mut file)) {
                    return Some(v);
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Some(json!({}))
        }
    }
}

/// A document this process owns: the held lock and the socket it listens on.
pub struct Claim {
    _lock: File,
    pub socket: PathBuf,
    pub listener: UnixListener,
    pub contents: Value,
}

impl Drop for Claim {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.socket);
    }
}

/// What claiming a document came to.
pub enum Claimed {
    /// This process owns it now.
    Owned(Claim),
    /// Another process does: its lock's contents.
    Held(Value),
}

/// Takes ownership of a document, or says who has it (D9). `app_pid` is the Mac app's pid, which
/// a second copy of the app brings forward (R26).
pub fn claim(state_dir: &Path, root: &Path, app_pid: Option<i64>, version: &str) -> std::io::Result<Claimed> {
    let path = lock_path(state_dir, root);
    let mut lock = open_lock(&path)?;
    if lock.try_lock().is_err() {
        return Ok(Claimed::Held(holder(state_dir, root).unwrap_or_else(|| json!({}))));
    }
    let socket = socket_path(state_dir, root);
    if let Some(dir) = socket.parent() {
        fs::create_dir_all(dir)?;
    }
    // We hold the lock, so a socket file left here is a dead owner's.
    let _ = fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket)?;
    let _ = fs::set_permissions(&socket, fs::Permissions::from_mode(0o600));
    let contents = json!({
        "pid": std::process::id(),
        "appPid": app_pid,
        "document": real_path(root).to_string_lossy(),
        "socket": socket.to_string_lossy(),
        "protocol": PROTOCOL,
        "version": version,
        "startedAt": iso_time(now_ms()),
    });
    lock.set_len(0)?;
    lock.seek(SeekFrom::Start(0))?;
    lock.write_all(stringify(&contents).as_bytes())?;
    lock.flush()?;
    Ok(Claimed::Owned(Claim { _lock: lock, socket, listener, contents }))
}

// ------------------------------------------------------------------ peers

#[cfg(target_os = "macos")]
fn peer_uid(stream: &UnixStream) -> Option<u32> {
    extern "C" {
        fn getpeereid(socket: i32, uid: *mut u32, gid: *mut u32) -> i32;
    }
    let (mut uid, mut gid) = (0u32, 0u32);
    // SAFETY: the fd is the stream's, open for the call; uid and gid are valid out pointers.
    let ok = unsafe { getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } == 0;
    ok.then_some(uid)
}

#[cfg(target_os = "linux")]
fn peer_uid(stream: &UnixStream) -> Option<u32> {
    #[repr(C)]
    struct Ucred {
        pid: i32,
        uid: u32,
        gid: u32,
    }
    extern "C" {
        fn getsockopt(socket: i32, level: i32, name: i32, value: *mut std::ffi::c_void, len: *mut u32) -> i32;
    }
    const SOL_SOCKET: i32 = 1;
    const SO_PEERCRED: i32 = 17;
    let mut cred = Ucred { pid: 0, uid: 0, gid: 0 };
    let mut len = std::mem::size_of::<Ucred>() as u32;
    // SAFETY: the fd is the stream's; cred and len are valid for getsockopt to fill.
    let ok = unsafe { getsockopt(stream.as_raw_fd(), SOL_SOCKET, SO_PEERCRED, &mut cred as *mut Ucred as *mut _, &mut len) } == 0;
    ok.then_some(cred.uid)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn peer_uid(_: &UnixStream) -> Option<u32> {
    Some(my_uid())
}

fn my_uid() -> u32 {
    extern "C" {
        fn getuid() -> u32;
    }
    // SAFETY: getuid takes nothing and can't fail.
    unsafe { getuid() }
}

/// Whether a peer runs as this user (R27).
pub fn same_user(stream: &UnixStream) -> bool {
    peer_uid(stream) == Some(my_uid())
}

// --------------------------------------------------------------- listener

/// What a client connection sends the owner's event loop.
pub enum Incoming {
    /// A line from connection `conn`.
    Line { conn: u64, line: String },
    /// Connection `conn` closed.
    Gone { conn: u64 },
}

/// The client connections' write halves, by connection id, for the replies.
pub type Clients = Arc<Mutex<std::collections::HashMap<u64, UnixStream>>>;

/// Accepts connections on the claim's socket, each read on a thread of its own, every line sent to
/// `to` as `wrap(Incoming)`. Connections from other users are dropped. Returns the write halves.
pub fn listen<E: Send + 'static>(claim: &Claim, to: Sender<E>, wrap: fn(Incoming) -> E) -> std::io::Result<Clients> {
    let listener = claim.listener.try_clone()?;
    let clients: Clients = Arc::default();
    let writers = clients.clone();
    std::thread::spawn(move || {
        let mut next = 0u64;
        for stream in listener.incoming() {
            let Ok(stream) = stream else { break };
            if !same_user(&stream) {
                continue;
            }
            next += 1;
            let conn = next;
            let Ok(write_half) = stream.try_clone() else { continue };
            writers.lock().expect("clients").insert(conn, write_half);
            let to = to.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stream).lines() {
                    let Ok(line) = line else { break };
                    if to.send(wrap(Incoming::Line { conn, line })).is_err() {
                        return;
                    }
                }
                let _ = to.send(wrap(Incoming::Gone { conn }));
            });
        }
    });
    Ok(clients)
}

/// Writes one reply line to a connection; a client that went away is dropped.
pub fn reply(clients: &Clients, conn: u64, message: &Value) {
    let mut list = clients.lock().expect("clients");
    let failed = match list.get_mut(&conn) {
        Some(stream) => writeln!(stream, "{}", stringify(message)).and_then(|_| stream.flush()).is_err(),
        None => false,
    };
    if failed {
        list.remove(&conn);
    }
}

/// Closes every client connection (the owner is stopping).
pub fn close_all(clients: &Clients) {
    for (_, stream) in clients.lock().expect("clients").drain() {
        let _ = stream.shutdown(std::net::Shutdown::Both);
    }
}
