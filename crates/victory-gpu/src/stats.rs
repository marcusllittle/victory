//! The run's numbers once a second in a CSV file, for measuring a build on a
//! card (`VICTORY_STATS=<file>`; off by default). It works in release builds,
//! where the panel (`VICTORY_PANEL`) and the info-level frame line are not
//! shown. The panel needs a diagnostics build, and the launchers log `warn` only.
//!
//! One row per frame-meter second: the frame rate, the mean and the longest
//! frame of that second, draws, draws dropped for want of a host pipeline
//! (objects missing while pipelines are made), indirect records run empty for
//! want of their list's budget (the suspect for outdoor flicker), the
//! submitting thread's time and its waits for the card, the process's private
//! bytes and working set, what Windows has left to commit, the card's memory
//! as the system counts it to this process, and the texture queue (slots
//! waiting now, and how often since the start it found the card full: textures
//! that come late).
//!
//! An existing file is never written over: `stats.csv` becomes `stats-1.csv`,
//! `stats-2.csv` and so on. Beside it, `<file>.switches.txt` lists the run's
//! `VICTORY_*` variables. `scripts\stats-summary.ps1` reads the file back.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// What the frame meter worked out for the last second, per frame.
pub(crate) struct Second {
    pub(crate) fps: f64,
    pub(crate) ms: f64,
    pub(crate) worst_ms: f64,
    pub(crate) draws: f64,
    pub(crate) dropped: f64,
    pub(crate) indirect_empty: f64,
    pub(crate) preparing_ms: f64,
    pub(crate) gpu_wait_ms: f64,
}

const HEADER: &str = "seconds,frame,fps,frame_ms,worst_ms,draws,dropped_no_pipeline,indirect_empty,\
preparing_ms,gpu_wait_ms,private_mb,working_mb,commit_left_mb,vram_used_mb,vram_budget_mb,shared_used_mb,\
textures_waiting,texture_queue_full_total";

struct Out {
    file: std::io::BufWriter<std::fs::File>,
    began: Instant,
    failed: bool,
}

fn out() -> Option<&'static Mutex<Out>> {
    static OUT: OnceLock<Option<Mutex<Out>>> = OnceLock::new();
    OUT.get_or_init(open).as_ref()
}

/// The file named by `VICTORY_STATS` (unset, empty or `0`: none), made new
/// with its header.
fn open() -> Option<Mutex<Out>> {
    let asked = std::env::var_os("VICTORY_STATS").filter(|v| !v.is_empty() && v != "0")?;
    let path = free_path(Path::new(&asked));
    let file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "VICTORY_STATS: the file could not be made; no numbers are written");
            return None;
        }
    };
    let mut file = std::io::BufWriter::new(file);
    if let Err(e) = writeln!(file, "{HEADER}").and_then(|()| file.flush()) {
        tracing::warn!(path = %path.display(), error = %e, "VICTORY_STATS: the file could not be written; no numbers are written");
        return None;
    }
    write_switches(&path);
    // Warn level, so that the launchers' `VICTORY_LOG=warn` shows where it went.
    tracing::warn!(path = %path.display(), "VICTORY_STATS: the run's numbers go to this file once a second");
    Some(Mutex::new(Out {
        file,
        began: Instant::now(),
        failed: false,
    }))
}

/// `asked`, or the first `<stem>-<n><.ext>` beside it that does not exist.
fn free_path(asked: &Path) -> PathBuf {
    if !asked.exists() {
        return asked.to_owned();
    }
    let stem = asked
        .file_stem()
        .map_or_else(|| "stats".to_owned(), |s| s.to_string_lossy().into_owned());
    let ext = asked
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (1..1000)
        .map(|n| asked.with_file_name(format!("{stem}-{n}{ext}")))
        .find(|p| !p.exists())
        .unwrap_or_else(|| asked.to_owned())
}

/// The run's `VICTORY_*` variables, sorted, in `<csv>.switches.txt`.
fn write_switches(csv: &Path) {
    let mut vars: Vec<(String, String)> = std::env::vars_os()
        .filter_map(|(k, v)| {
            let k = k.into_string().ok()?;
            k.starts_with("VICTORY_")
                .then(|| (k, v.to_string_lossy().into_owned()))
        })
        .collect();
    vars.sort();
    let text: String = vars.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
    let mut name = csv.as_os_str().to_owned();
    name.push(".switches.txt");
    if let Err(e) = std::fs::write(PathBuf::from(name), text) {
        tracing::warn!(error = %e, "VICTORY_STATS: the list of switches could not be written");
    }
}

/// A row: the seconds since the file was made, the frame, `s`, then bytes
/// (private, working set, commit left, card used, card budget, shared used)
/// shown as MiB, then the texture queue (waiting now, full since the start).
fn row(seconds: f64, frame: u64, s: &Second, bytes: [u64; 6], textures: [u64; 2]) -> String {
    let mb = bytes.map(|b| b >> 20);
    format!(
        "{seconds:.1},{frame},{:.2},{:.2},{:.2},{:.0},{:.1},{:.2},{:.2},{:.2},{},{},{},{},{},{},{},{}",
        s.fps,
        s.ms,
        s.worst_ms,
        s.draws,
        s.dropped,
        s.indirect_empty,
        s.preparing_ms,
        s.gpu_wait_ms,
        mb[0],
        mb[1],
        mb[2],
        mb[3],
        mb[4],
        mb[5],
        textures[0],
        textures[1]
    )
}

/// Writes the row of a second, if `VICTORY_STATS` names a file.
pub(crate) fn second(s: &Second) {
    let Some(out) = out() else {
        return;
    };
    let (working, private) = process_memory();
    let commit_left = commit_left();
    let [budget, used, _, shared] = crate::host::host()
        .and_then(|h| h.video_memory())
        .unwrap_or_default();
    let textures = [
        crate::draw::HEAP_TEXTURES_WAITING.load(Relaxed),
        crate::draw::HEAP_TEXTURES_FULL.load(Relaxed),
    ];
    let mut out = out.lock().unwrap_or_else(|e| e.into_inner());
    if out.failed {
        return;
    }
    let line = row(
        out.began.elapsed().as_secs_f64(),
        crate::frame::current(),
        s,
        [private, working, commit_left, used, budget, shared],
        textures,
    );
    if let Err(e) = writeln!(out.file, "{line}").and_then(|()| out.file.flush()) {
        out.failed = true;
        tracing::warn!(error = %e, "VICTORY_STATS: writing failed; no more numbers are written");
    }
}

/// The process's working set and private bytes.
fn process_memory() -> (u64, u64) {
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut c = PROCESS_MEMORY_COUNTERS {
        cb: size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..Default::default()
    };
    // SAFETY: our own process and a sized structure.
    unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    (c.WorkingSetSize as u64, c.PagefileUsage as u64)
}

/// What Windows has left to commit, in bytes.
fn commit_left() -> u64 {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    // SAFETY: plain data the call fills in.
    let mut m: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    m.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
    // SAFETY: a sized structure.
    unsafe { GlobalMemoryStatusEx(&mut m) };
    m.ullAvailPageFile
}

#[cfg(test)]
mod tests {
    use super::*;

    fn second() -> Second {
        Second {
            fps: 20.0,
            ms: 50.0,
            worst_ms: 120.5,
            draws: 3000.0,
            dropped: 1.5,
            indirect_empty: 0.25,
            preparing_ms: 40.0,
            gpu_wait_ms: 5.0,
        }
    }

    #[test]
    fn a_row_has_a_value_for_every_column() {
        let line = row(
            12.34,
            900,
            &second(),
            [5 << 30, 4 << 30, 20 << 30, 9 << 30, 10 << 30, 1 << 29],
            [17, 3],
        );
        assert_eq!(line.split(',').count(), HEADER.split(',').count(), "{line}");
        assert!(line.starts_with("12.3,900,20.00,50.00,120.50,3000,1.5,0.25,40.00,5.00,5120,4096,20480,9216,10240,512,17,3"), "{line}");
    }

    #[test]
    fn the_header_has_no_spaces_or_empty_names() {
        assert!(
            HEADER
                .split(',')
                .all(|name| !name.is_empty() && !name.contains(' ')),
            "{HEADER}"
        );
    }

    #[test]
    fn an_existing_file_is_not_written_over() {
        let dir = tempfile::tempdir().unwrap();
        let asked = dir.path().join("stats.csv");
        assert_eq!(free_path(&asked), asked);
        std::fs::write(&asked, "").unwrap();
        assert_eq!(free_path(&asked), dir.path().join("stats-1.csv"));
        std::fs::write(dir.path().join("stats-1.csv"), "").unwrap();
        assert_eq!(free_path(&asked), dir.path().join("stats-2.csv"));
        let bare = dir.path().join("run");
        std::fs::write(&bare, "").unwrap();
        assert_eq!(free_path(&bare), dir.path().join("run-1"));
    }
}
