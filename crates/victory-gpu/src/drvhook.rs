//! Diagnostics (`VICTORY_DRIVER_HOOK=1`): which call stacks make the host
//! graphics driver keep memory. The driver's own heap and page allocations
//! are taken through its import table; a sample of them is recorded with
//! its call stack, and what is freed again leaves the record. Every 20
//! seconds the stacks holding the most are logged, frames as module+offset
//! (the private memory of the process grew by gigabytes a minute in the
//! driver, with nothing drawn, and no object of ours accounted for it).

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};

use windows_sys::Win32::System::Diagnostics::Debug::RtlCaptureStackBackTrace;
use windows_sys::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, GetModuleFileNameW,
    GetModuleHandleExW, GetModuleHandleW,
};
use windows_sys::Win32::System::Memory::{PAGE_READWRITE, VirtualProtect};

type HeapAllocFn = unsafe extern "system" fn(*mut c_void, u32, usize) -> *mut c_void;
type HeapFreeFn = unsafe extern "system" fn(*mut c_void, u32, *mut c_void) -> i32;
type HeapReAllocFn = unsafe extern "system" fn(*mut c_void, u32, *mut c_void, usize) -> *mut c_void;
type VirtualAllocFn = unsafe extern "system" fn(*mut c_void, usize, u32, u32) -> *mut c_void;
type VirtualFreeFn = unsafe extern "system" fn(*mut c_void, usize, u32) -> i32;

/// The real functions: HeapAlloc, HeapFree, HeapReAlloc, VirtualAlloc, VirtualFree.
static REAL: [AtomicUsize; 5] = [const { AtomicUsize::new(0) }; 5];
/// One small allocation in `SAMPLE` is recorded (and counted `SAMPLE`
/// times); every one of `BIG` bytes or more is.
const SAMPLE: u64 = 16;
const BIG: usize = 256 << 10;
static CALLS: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
struct State {
    /// Per stack: its frames, bytes held, allocations held.
    stacks: HashMap<u64, (Vec<usize>, i64, i64)>,
    /// The thread that first made each stack's allocation.
    names: HashMap<u64, String>,
    /// Recorded allocations alive: address -> (stack, bytes, count).
    live: HashMap<usize, (u64, i64, i64)>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn thread_name() -> String {
    use windows_sys::Win32::System::Threading::{GetCurrentThread, GetCurrentThreadId, GetThreadDescription};
    // SAFETY: plain queries; the description is freed below.
    let id = unsafe { GetCurrentThreadId() };
    let mut p: windows_sys::core::PWSTR = std::ptr::null_mut();
    let ok = unsafe { GetThreadDescription(GetCurrentThread(), &mut p) } >= 0;
    let mut name = String::new();
    if ok && !p.is_null() {
        // SAFETY: a NUL-terminated wide string from the system, freed after.
        unsafe {
            let mut n = 0;
            while *p.add(n) != 0 {
                n += 1;
            }
            name = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
            windows_sys::Win32::Foundation::LocalFree(p.cast());
        }
    }
    format!("thread {id} '{name}'")
}

fn record(at: *mut c_void, size: usize) {
    if at.is_null() {
        return;
    }
    let n = CALLS.fetch_add(1, Relaxed);
    let scale = if size >= BIG {
        1
    } else if n.is_multiple_of(SAMPLE) {
        SAMPLE as i64
    } else {
        return;
    };
    let mut frames = [std::ptr::null_mut::<c_void>(); 62];
    // SAFETY: a buffer of 62 frames.
    let got = unsafe { RtlCaptureStackBackTrace(2, 62, frames.as_mut_ptr(), std::ptr::null_mut()) } as usize;
    let frames: Vec<usize> = frames[..got].iter().map(|&f| f as usize).collect();
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for &f in &frames {
        hash = (hash ^ f as u64).wrapping_mul(0x100_0000_01b3);
    }
    let bytes = size as i64 * scale;
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let state = state.get_or_insert_with(State::default);
    if !state.stacks.contains_key(&hash) {
        state.names.insert(hash, thread_name());
    }
    let entry = state.stacks.entry(hash).or_insert_with(|| (frames, 0, 0));
    entry.1 += bytes;
    entry.2 += scale;
    state.live.insert(at as usize, (hash, bytes, scale));
}

fn forget(at: *mut c_void) {
    if at.is_null() {
        return;
    }
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(state) = state.as_mut() else {
        return;
    };
    if let Some((hash, bytes, count)) = state.live.remove(&(at as usize))
        && let Some(entry) = state.stacks.get_mut(&hash)
    {
        entry.1 -= bytes;
        entry.2 -= count;
    }
}

unsafe extern "system" fn heap_alloc(heap: *mut c_void, flags: u32, size: usize) -> *mut c_void {
    // SAFETY: the real HeapAlloc, saved before the patch.
    let real: HeapAllocFn = unsafe { std::mem::transmute(REAL[0].load(Relaxed)) };
    let at = unsafe { real(heap, flags, size) };
    record(at, size);
    at
}

unsafe extern "system" fn heap_free(heap: *mut c_void, flags: u32, at: *mut c_void) -> i32 {
    forget(at);
    // SAFETY: the real HeapFree.
    let real: HeapFreeFn = unsafe { std::mem::transmute(REAL[1].load(Relaxed)) };
    unsafe { real(heap, flags, at) }
}

unsafe extern "system" fn heap_realloc(heap: *mut c_void, flags: u32, at: *mut c_void, size: usize) -> *mut c_void {
    // SAFETY: the real HeapReAlloc.
    let real: HeapReAllocFn = unsafe { std::mem::transmute(REAL[2].load(Relaxed)) };
    let new = unsafe { real(heap, flags, at, size) };
    if !new.is_null() {
        forget(at);
        record(new, size);
    }
    new
}

unsafe extern "system" fn virtual_alloc(at: *mut c_void, size: usize, kind: u32, protect: u32) -> *mut c_void {
    // SAFETY: the real VirtualAlloc.
    let real: VirtualAllocFn = unsafe { std::mem::transmute(REAL[3].load(Relaxed)) };
    let got = unsafe { real(at, size, kind, protect) };
    const MEM_COMMIT: u32 = 0x1000;
    if kind & MEM_COMMIT != 0 && at.is_null() {
        record(got, size.max(BIG));
    }
    got
}

unsafe extern "system" fn virtual_free(at: *mut c_void, size: usize, kind: u32) -> i32 {
    const MEM_RELEASE: u32 = 0x8000;
    if kind & MEM_RELEASE != 0 {
        forget(at);
    }
    // SAFETY: the real VirtualFree.
    let real: VirtualFreeFn = unsafe { std::mem::transmute(REAL[4].load(Relaxed)) };
    unsafe { real(at, size, kind) }
}

/// Patches `module`'s imports from KERNEL32 named in `names` to `hooks`.
///
/// # Safety
/// `module` is a loaded image.
unsafe fn patch(module: usize, names: &[&str], hooks: &[usize]) -> usize {
    // SAFETY (both): reads inside the loaded image's headers and tables.
    let rd32 = |at: usize| unsafe { (at as *const u32).read_unaligned() } as usize;
    let rd64 = |at: usize| unsafe { (at as *const u64).read_unaligned() };
    let pe = module + rd32(module + 0x3c);
    let imports = rd32(pe + 24 + 120);
    let mut desc = module + imports;
    let mut patched = 0;
    loop {
        let (ilt, name, iat) = (rd32(desc), rd32(desc + 12), rd32(desc + 16));
        if name == 0 {
            break;
        }
        // SAFETY: a NUL-terminated import name in the image.
        let dll = unsafe { std::ffi::CStr::from_ptr((module + name) as *const std::ffi::c_char) }
            .to_string_lossy()
            .to_ascii_lowercase();
        if dll == "kernel32.dll" {
            let mut i = 0;
            loop {
                let e = rd64(module + if ilt != 0 { ilt } else { iat } + i * 8);
                if e == 0 {
                    break;
                }
                if e >> 63 == 0 {
                    // SAFETY: the hint/name entry's name.
                    let f = unsafe {
                        std::ffi::CStr::from_ptr((module + (e as usize & 0x7fff_ffff) + 2) as *const std::ffi::c_char)
                    };
                    if let Some(k) = names.iter().position(|n| f.to_bytes() == n.as_bytes()) {
                        let slot = module + iat + i * 8;
                        let mut old = 0;
                        // SAFETY: the import slot, made writable for the store.
                        unsafe {
                            VirtualProtect(slot as *const c_void, 8, PAGE_READWRITE, &mut old);
                            REAL[k].store((slot as *const usize).read(), Relaxed);
                            (slot as *mut usize).write(hooks[k]);
                            VirtualProtect(slot as *const c_void, 8, old, &mut old);
                        }
                        patched += 1;
                    }
                }
                i += 1;
            }
        }
        desc += 20;
    }
    patched
}

fn place(frame: usize) -> String {
    let mut module = std::ptr::null_mut();
    // SAFETY: a code address; the handle is not kept.
    let ok = unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            frame as *const u16,
            &mut module,
        )
    };
    if ok == 0 {
        return format!("{frame:#x}");
    }
    let mut name = [0u16; 260];
    // SAFETY: a buffer of 260.
    let n = unsafe { GetModuleFileNameW(module, name.as_mut_ptr(), 260) } as usize;
    let path = String::from_utf16_lossy(&name[..n]);
    let file = path.rsplit('\\').next().unwrap_or(&path).to_owned();
    format!("{file}+{:#x}", frame - module as usize)
}

fn report() {
    let (total, top): (i64, Vec<(i64, i64, Vec<usize>, String)>) = {
        let state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(state) = state.as_ref() else {
            return;
        };
        let total = state.stacks.values().map(|v| v.1).sum();
        let mut all: Vec<_> = state
            .stacks
            .iter()
            .map(|(h, (f, b, c))| (*b, *c, f.clone(), state.names.get(h).cloned().unwrap_or_default()))
            .collect();
        all.sort_by_key(|a| std::cmp::Reverse(a.0));
        all.truncate(3);
        (total, all)
    };
    let mut s = format!("driver memory held by stack (sampled): {} MB in all;", total >> 20);
    for (bytes, count, frames, who) in top {
        s += &format!("\n  {} MB in {} allocations, first by {who}:", bytes >> 20, count);
        for f in &frames {
            s += " ";
            s += &place(*f);
        }
    }
    tracing::info!("{s}");
}

/// Installs the hooks in the driver module once (after the device exists).
pub(crate) fn install() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    if std::env::var_os("VICTORY_DRIVER_HOOK").is_none_or(|v| v != "1") {
        return;
    }
    ONCE.call_once(|| {
        let name: Vec<u16> = "amdxc64.dll\0".encode_utf16().collect();
        // SAFETY: a NUL-terminated name.
        let module = unsafe { GetModuleHandleW(name.as_ptr()) } as usize;
        if module == 0 {
            tracing::warn!("driver hook: amdxc64.dll not loaded");
            return;
        }
        // A function item cast straight to `usize` is not the code address
        // (the item is zero-sized). The pointer cast is the address written
        // into the driver's import slot.
        let hooks = [
            heap_alloc as *const () as usize,
            heap_free as *const () as usize,
            heap_realloc as *const () as usize,
            virtual_alloc as *const () as usize,
            virtual_free as *const () as usize,
        ];
        // SAFETY: a loaded image.
        let n = unsafe {
            patch(module, &["HeapAlloc", "HeapFree", "HeapReAlloc", "VirtualAlloc", "VirtualFree"], &hooks)
        };
        tracing::info!(patched = n, "driver hook installed in amdxc64.dll");
        std::thread::spawn(|| {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(20));
                report();
            }
        });
    });
}
