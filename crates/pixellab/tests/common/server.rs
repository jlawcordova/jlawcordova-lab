//! A `pixellab serve` child process for tests (the harness of servers.rs): requests and raw
//! lines in, responses and notifications out, every wait with a deadline.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

pub const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Default)]
pub struct Shared {
    lines: Vec<Value>,
    not_json: Vec<String>,
    stderr: String,
}

/// A running server: requests and raw lines in, responses and notifications out.
pub struct Server {
    pub child: Child,
    pub stdin: Option<ChildStdin>,
    shared: Arc<(Mutex<Shared>, Condvar)>,
    next_id: u64,
}

pub fn start_server(args: &[&str], env: &HashMap<String, String>, state: &Path) -> Server {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_pixellab"));
    cmd.arg("serve")
        .args(args)
        .env_remove("PIXELLAB_SITE")
        .env_remove("PIXELLAB_DOC")
        .env_remove("PIXELLAB_CHAT")
        .env_remove("PIXELLAB_CHAT_TITLE");
    cmd.envs(env).env("PIXELLAB_STATE", state);
    cmd.current_dir(state).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn the server");
    let stdin = child.stdin.take();
    let stdout = child.stdout.take().expect("stdout");
    let stderr = child.stderr.take().expect("stderr");
    let shared: Arc<(Mutex<Shared>, Condvar)> = Arc::default();
    let out = shared.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            let (lock, cv) = &*out;
            let mut s = lock.lock().unwrap();
            match serde_json::from_str::<Value>(&line) {
                Ok(v) => s.lines.push(v),
                Err(_) => s.not_json.push(line),
            }
            cv.notify_all();
        }
    });
    let err = shared.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            let Ok(line) = line else { break };
            let (lock, _) = &*err;
            let mut s = lock.lock().unwrap();
            s.stderr.push_str(&line);
            s.stderr.push('\n');
        }
    });
    Server { child, stdin, shared, next_id: 1 }
}

impl Server {
    /// Every message the server has written so far.
    pub fn lines(&self) -> Vec<Value> {
        let s = self.shared.0.lock().unwrap();
        assert!(s.not_json.is_empty(), "stdout had a line that is not JSON: {:?}", s.not_json);
        s.lines.clone()
    }

    /// The notifications (messages with a method) so far.
    pub fn notifications(&self) -> Vec<Value> {
        self.lines().into_iter().filter(|m| m.get("method").is_some()).collect()
    }

    pub fn stderr(&self) -> String {
        self.shared.0.lock().unwrap().stderr.clone()
    }

    pub fn send(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().expect("stdin is open");
        writeln!(stdin, "{line}").expect("write to the server");
        stdin.flush().expect("flush");
    }

    /// Sends a request and returns the whole response message.
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string());
        let deadline = Instant::now() + TIMEOUT;
        let (lock, cv) = &*self.shared;
        let mut s = lock.lock().unwrap();
        loop {
            assert!(s.not_json.is_empty(), "stdout had a line that is not JSON: {:?}", s.not_json);
            if let Some(m) = s.lines.iter().find(|m| m.get("id") == Some(&json!(id)) && m.get("method").is_none()) {
                return m.clone();
            }
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "timed out waiting for the response to {method}; got {:?}", s.lines);
            s = cv.wait_timeout(s, left).unwrap().0;
        }
    }

    pub fn notify(&mut self, method: &str) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": {} }).to_string());
    }

    /// Waits for a notification passing `test`; those before index `since` (of `notifications()`) don't count.
    pub fn wait_for(&self, since: usize, what: &str, test: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + TIMEOUT;
        let (lock, cv) = &*self.shared;
        let mut s = lock.lock().unwrap();
        loop {
            assert!(s.not_json.is_empty(), "stdout had a line that is not JSON: {:?}", s.not_json);
            let seen: Vec<&Value> = s.lines.iter().filter(|m| m.get("method").is_some()).skip(since).collect();
            if let Some(n) = seen.iter().find(|n| test(n)) {
                return (*n).clone();
            }
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(
                !left.is_zero(),
                "timed out waiting for {what}; got {:?}",
                seen.iter().map(|n| n["method"].clone()).collect::<Vec<_>>()
            );
            s = cv.wait_timeout(s, left).unwrap().0;
        }
    }

    /// The exit code, once the process ends (a deadline fails the test).
    pub fn exited(&mut self) -> i32 {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            if let Some(status) = self.child.try_wait().expect("wait") {
                return status.code().unwrap_or(-1);
            }
            assert!(Instant::now() < deadline, "the server did not exit");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Ends the server's input and waits for it to exit.
    pub fn close(&mut self) -> i32 {
        self.stdin = None;
        self.exited()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
