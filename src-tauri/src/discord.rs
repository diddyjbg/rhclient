// Discord Rich Presence + "Join" invites, talking to the Discord app directly over its local pipe (\\.\pipe\discord-ipc-N).
// One background thread: it polls the pipe (so it never blocks), sends status updates and passes Discord events to the game.
use serde_json::{json, Value};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub enum Msg {
    Activity(Value),
    Reply(String, bool),
}

pub struct Discord {
    tx: Sender<Msg>,
    /// a "Join" that arrived before the game page was listening (for example when Discord started the app for it)
    pub pending: Arc<Mutex<Option<String>>>,
}

impl Discord {
    pub fn start<F: Fn(Value) + Send + 'static>(client_id: &'static str, on_event: F) -> Self {
        let (tx, rx) = mpsc::channel::<Msg>();
        let pending = Arc::new(Mutex::new(None));
        if !client_id.is_empty() {
            let pend = pending.clone();
            thread::spawn(move || {
                run(client_id, rx, move |ev: Value| {
                    if ev["type"] == "join" {
                        if let Some(sec) = ev["secret"].as_str() {
                            *pend.lock().unwrap() = Some(sec.to_string());
                        }
                    }
                    on_event(ev)
                })
            });
        }
        Discord { tx, pending }
    }
    /// the game asks for this once it has loaded
    pub fn take_join(&self) -> Option<String> {
        self.pending.lock().unwrap().take()
    }
    pub fn set_activity(&self, a: Value) {
        let _ = self.tx.send(Msg::Activity(a));
    }
    pub fn reply(&self, user: String, accept: bool) {
        let _ = self.tx.send(Msg::Reply(user, accept));
    }
}

#[cfg(windows)]
mod pipe {
    use std::fs::{File, OpenOptions};
    use std::io::{Read, Write};
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Pipes::PeekNamedPipe;

    pub struct Pipe(File);

    impl Pipe {
        pub fn connect() -> Option<Pipe> {
            (0..10).find_map(|i| OpenOptions::new().read(true).write(true).open(format!(r"\\?\pipe\discord-ipc-{}", i)).ok().map(Pipe))
        }
        pub fn send(&mut self, op: u32, payload: &serde_json::Value) -> std::io::Result<()> {
            let body = payload.to_string().into_bytes();
            let mut buf = Vec::with_capacity(8 + body.len());
            buf.extend_from_slice(&op.to_le_bytes());
            buf.extend_from_slice(&(body.len() as u32).to_le_bytes());
            buf.extend_from_slice(&body);
            self.0.write_all(&buf)?;
            self.0.flush()
        }
        /// bytes waiting in the pipe (None = the pipe broke)
        pub fn available(&self) -> Option<u32> {
            let mut avail: u32 = 0;
            let ok = unsafe { PeekNamedPipe(self.0.as_raw_handle() as _, std::ptr::null_mut(), 0, std::ptr::null_mut(), &mut avail, std::ptr::null_mut()) };
            if ok == 0 { None } else { Some(avail) }
        }
        pub fn recv(&mut self) -> std::io::Result<(u32, serde_json::Value)> {
            let mut head = [0u8; 8];
            self.0.read_exact(&mut head)?;
            let op = u32::from_le_bytes([head[0], head[1], head[2], head[3]]);
            let len = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize;
            let mut body = vec![0u8; len.min(1 << 20)];
            self.0.read_exact(&mut body)?;
            Ok((op, serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null)))
        }
    }
}

#[cfg(windows)]
fn run<F: Fn(Value)>(client_id: &str, rx: Receiver<Msg>, on_event: F) {
    let mut last: Option<Value> = None; // latest status, re-sent after reconnecting
    let mut nonce: u64 = 0;
    let mut next = || { nonce += 1; nonce.to_string() };
    loop {
        // keep only the newest status while Discord is not running
        let Some(mut p) = pipe::Pipe::connect() else {
            while let Ok(m) = rx.try_recv() { if let Msg::Activity(a) = m { last = Some(a); } }
            thread::sleep(Duration::from_secs(10));
            continue;
        };
        if p.send(0, &json!({ "v": 1, "client_id": client_id })).is_err() { continue; }
        // Discord answers the handshake with READY; only then does it accept commands
        let mut ready = false;
        for _ in 0..100 {
            match p.available() {
                None => break,
                Some(n) if n >= 8 => match p.recv() {
                    Ok((1, v)) if v["evt"] == "READY" => { on_event(json!({ "type": "ready", "user": v["data"]["user"] })); ready = true; break; }
                    Ok((2, _)) | Err(_) => break,
                    _ => {}
                },
                _ => thread::sleep(Duration::from_millis(50)),
            }
        }
        if !ready { thread::sleep(Duration::from_secs(5)); continue; }
        let mut ok = true;
        for evt in ["ACTIVITY_JOIN", "ACTIVITY_JOIN_REQUEST"] {
            ok &= p.send(1, &json!({ "cmd": "SUBSCRIBE", "evt": evt, "args": {}, "nonce": next() })).is_ok();
        }
        if let Some(a) = &last {
            ok &= p.send(1, &json!({ "cmd": "SET_ACTIVITY", "args": { "pid": std::process::id(), "activity": a }, "nonce": next() })).is_ok();
        }
        while ok {
            // things to send
            while let Ok(m) = rx.try_recv() {
                let r = match m {
                    Msg::Activity(a) => {
                        last = Some(a.clone());
                        let act = if a.is_null() { Value::Null } else { a };
                        p.send(1, &json!({ "cmd": "SET_ACTIVITY", "args": { "pid": std::process::id(), "activity": act }, "nonce": next() }))
                    }
                    Msg::Reply(user, accept) => {
                        let cmd = if accept { "SEND_ACTIVITY_JOIN_INVITE" } else { "CLOSE_ACTIVITY_REQUEST" };
                        p.send(1, &json!({ "cmd": cmd, "args": { "user_id": user }, "nonce": next() }))
                    }
                };
                if r.is_err() { ok = false; break; }
            }
            // things Discord sent
            match p.available() {
                None => ok = false,
                Some(n) if n >= 8 => match p.recv() {
                    Ok((2, _)) | Err(_) => ok = false, // op 2 = Discord closed the connection
                    Ok((_, v)) => {
                        let d = &v["data"];
                        match (v["cmd"].as_str(), v["evt"].as_str()) {
                            (Some("DISPATCH"), Some("READY")) => on_event(json!({ "type": "ready", "user": d["user"] })),
                            (Some("DISPATCH"), Some("ACTIVITY_JOIN")) => on_event(json!({ "type": "join", "secret": d["secret"] })),
                            (Some("DISPATCH"), Some("ACTIVITY_JOIN_REQUEST")) => on_event(json!({ "type": "request", "user": d["user"] })),
                            _ => {}
                        }
                    }
                },
                _ => thread::sleep(Duration::from_millis(150)),
            }
        }
        thread::sleep(Duration::from_secs(3)); // Discord closed or restarted: try again
    }
}

#[cfg(not(windows))]
fn run<F: Fn(Value)>(_client_id: &str, rx: Receiver<Msg>, _on_event: F) {
    while rx.recv().is_ok() {}
}

/// Lets Discord start the game when someone presses "Join" while Rhythmhel is closed
/// (the same registry entry Discord's own SDK writes: HKCU\Software\Classes\discord-<app id>).
pub fn register_launch(client_id: &str) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if client_id.is_empty() { return; }
        let Ok(exe) = std::env::current_exe() else { return };
        let exe = exe.to_string_lossy().to_string();
        let key = format!(r"HKCU\Software\Classes\discord-{}", client_id);
        let cmd = format!("\"{}\" \"%1\"", exe);
        let entries: [(String, &str, String); 4] = [
            (key.clone(), "/ve", format!("URL:Run game {} protocol", client_id)),
            (key.clone(), "URL Protocol", String::new()),
            (format!(r"{}\DefaultIcon", key), "/ve", exe.clone()),
            (format!(r"{}\shell\open\command", key), "/ve", cmd),
        ];
        for (k, name, val) in entries {
            let mut c = std::process::Command::new("reg");
            c.arg("add").arg(&k);
            if name == "/ve" { c.arg("/ve"); } else { c.arg("/v").arg(name); }
            c.arg("/d").arg(&val).arg("/f").creation_flags(0x0800_0000); // no console window
            let _ = c.status();
        }
    }
    #[cfg(not(windows))]
    let _ = client_id;
}
