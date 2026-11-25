//! Discord Rich Presence (local addition, 2026-11-24): what the player is
//! doing, shown on their Discord profile. Talks to the Discord app on this
//! PC only, through its local pipe (`\\.\pipe\discord-ipc-N`); nothing goes
//! anywhere else. Off unless `VICTORY_DISCORD_APP_ID` holds the id of an
//! application the user made at discord.com/developers (its name is what
//! Discord shows as "Playing ..."). An art asset uploaded to it under the
//! key `VICTORY_DISCORD_IMAGE` (default `gta6`) is shown as the picture.

use std::io::{Read, Write};
use std::sync::Mutex;

/// The newest status line ("En jeu · 2 fps", ...), sent by the thread.
static STATUS: Mutex<Option<String>> = Mutex::new(None);

/// Sets the status line shown under the game's name.
pub(crate) fn update(status: String) {
    let Ok(app) = std::env::var("VICTORY_DISCORD_APP_ID") else { return };
    *STATUS.lock().unwrap_or_else(|e| e.into_inner()) = Some(status);
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        let _ = std::thread::Builder::new().name("victory-discord".into()).spawn(move || run(&app));
    });
}

fn frame(op: u32, json: &str) -> Vec<u8> {
    let mut b = Vec::with_capacity(8 + json.len());
    b.extend_from_slice(&op.to_le_bytes());
    b.extend_from_slice(&(json.len() as u32).to_le_bytes());
    b.extend_from_slice(json.as_bytes());
    b
}

/// A JSON string literal of `s`.
fn quoted(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn connect(app: &str) -> Option<std::fs::File> {
    for n in 0..10 {
        let Ok(mut pipe) = std::fs::OpenOptions::new().read(true).write(true).open(format!(r"\\.\pipe\discord-ipc-{n}"))
        else {
            continue;
        };
        let hello = format!("{{\"v\":1,\"client_id\":{}}}", quoted(app));
        if pipe.write_all(&frame(0, &hello)).is_err() {
            continue;
        }
        // The READY answer: header then body.
        let mut head = [0u8; 8];
        if pipe.read_exact(&mut head).is_err() {
            continue;
        }
        let len = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize;
        let mut body = vec![0u8; len.min(1 << 16)];
        if pipe.read_exact(&mut body).is_err() {
            continue;
        }
        tracing::info!(pipe = n, "discord rich presence connected");
        return Some(pipe);
    }
    None
}

fn run(app: &str) {
    let start = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let image = std::env::var("VICTORY_DISCORD_IMAGE").unwrap_or_else(|_| "gta6".into());
    let mut pipe = None;
    let mut sent = String::new();
    let mut nonce = 0u64;
    loop {
        if pipe.is_none() {
            pipe = connect(app);
        }
        let status = STATUS.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_default();
        if let Some(p) = pipe.as_mut()
            && status != sent
        {
            nonce += 1;
            let activity = format!(
                "{{\"cmd\":\"SET_ACTIVITY\",\"nonce\":\"{nonce}\",\"args\":{{\"pid\":{pid},\"activity\":{{\
                 \"details\":{details},\"state\":{state},\"timestamps\":{{\"start\":{start}}},\
                 \"assets\":{{\"large_image\":{image},\"large_text\":{details}}}}}}}}}",
                pid = std::process::id(),
                details = quoted("GTA VI \u{2014} Native by Roro"),
                state = quoted(&status),
                image = quoted(&image),
            );
            let mut answer = [0u8; 8];
            let ok = p.write_all(&frame(1, &activity)).is_ok() && p.read_exact(&mut answer).is_ok() && {
                let len = u32::from_le_bytes([answer[4], answer[5], answer[6], answer[7]]) as usize;
                let mut body = vec![0u8; len.min(1 << 16)];
                p.read_exact(&mut body).is_ok()
            };
            if ok {
                sent = status;
            } else {
                pipe = None; // Discord closed: try again later
            }
        }
        // Discord accepts an update every 15 s at most.
        std::thread::sleep(std::time::Duration::from_secs(15));
    }
}
