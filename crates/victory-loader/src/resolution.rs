//! The title's output size and render scale (`VICTORY_OUTPUT`,
//! `VICTORY_RENDER_SCALE`, `VICTORY_UPSCALER`; each off without it).
//!
//! Each place is compared byte for byte before it is changed, and left
//! alone when the bytes differ, so a different build of the executable is
//! not patched. `VICTORY_FFX_SCALE` is not implemented: scaling FidelityFX
//! that way divided by a jitter phase count of 0.
//!
//! The places were read from `game_scarlett_master.exe` 1.0.2.1 (image
//! offsets):
//!
//! - **Output size.** Settings are applied from +0x224352b, which hands the
//!   size to +0x2478130, the writer of the screen size (+0xa9438cc /
//!   +0xa9438d0), and calls +0xd8c3a0 when the height changes. The
//!   replacement writes the asked size into the settings and then that same
//!   else-path. The back-buffer pool's leftover check (+0x247af25, an
//!   `int3` unless under 2 MB remains) is sized for 3840x2160. For another
//!   size that check is jumped over. The pool stays as large.
//! - **Render scale.** The title's own upscaler stores 2/3 at +0x28f1e78.
//!   `VICTORY_RENDER_SCALE` replaces that constant (0.5 to 1.0).
//! - **Upscaler.** `VICTORY_UPSCALER` makes the picker at +0x7e93f0 return
//!   6 (the title's own) or 7 (FidelityFX).

/// What a compared write did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Patched {
    /// The bytes were the expected ones, and the replacement was written.
    Done,
    /// The bytes there are not the ones this was taken from.
    Differs,
    /// The site does not fit, or Windows would not make it writable.
    Failed(&'static str),
}

/// Sizes the title has a display mode for.
pub const MODES: [(u32, u32); 5] = [(1280, 720), (1664, 936), (1920, 1080), (2560, 1440), (3840, 2160)];

/// `VICTORY_OUTPUT=<w>x<h>`: one of [`MODES`]. `None` if unset or not one
/// (said why).
pub fn asked_output() -> Option<(u32, u32)> {
    let spec = std::env::var("VICTORY_OUTPUT").ok()?;
    let size = parse_output(&spec);
    if size.is_none() {
        eprintln!(
            "VIctory: VICTORY_OUTPUT={spec}: expected one of 1920x1080, 2560x1440, 1280x720, 1664x936, 3840x2160"
        );
    }
    size
}

pub fn parse_output(spec: &str) -> Option<(u32, u32)> {
    let (w, h) = spec.trim().split_once(['x', 'X'])?;
    let size = (w.trim().parse().ok()?, h.trim().parse().ok()?);
    MODES.contains(&size).then_some(size)
}

/// `VICTORY_RENDER_SCALE=<s>`, 0.5 to 1.0: the title's own upscaler's scale
/// (2/3 without it). `None` if unset or out of range (said why).
pub fn asked_render_scale() -> Option<f32> {
    let spec = std::env::var("VICTORY_RENDER_SCALE").ok()?;
    let scale = parse_render_scale(&spec);
    if scale.is_none() {
        eprintln!("VIctory: VICTORY_RENDER_SCALE={spec}: expected 0.5 to 1.0");
    }
    scale
}

pub fn parse_render_scale(spec: &str) -> Option<f32> {
    spec.trim().parse::<f32>().ok().filter(|s| (0.5..=1.0).contains(s))
}

/// `VICTORY_UPSCALER=<kind>`: 6 (the title's own) or 7 (FidelityFX).
pub fn asked_upscaler() -> Option<u32> {
    let spec = std::env::var("VICTORY_UPSCALER").ok()?;
    let kind = spec.trim().parse::<u32>().ok().filter(|k| matches!(k, 6 | 7));
    if kind.is_none() {
        eprintln!("VIctory: VICTORY_UPSCALER={spec}: expected 6 (the title's own) or 7 (FidelityFX)");
    }
    kind
}

/// A place to change: its offset in the image and the bytes there
/// (compared first; the replacement is at most as long).
pub struct Site {
    pub at: usize,
    pub what: &'static str,
    pub bytes: &'static [u8],
}

/// The screen size setter, its screen height, and what +0x22434a0 calls when
/// the height changed.
pub const SET_SCREEN_SIZE: usize = 0x2478130;
pub const SCREEN_HEIGHT: usize = 0xa9438d0;
pub const HEIGHT_CHANGED: usize = 0xd8c3a0;

/// +0x22434a0 from +0x224352b to +0x2243568: the settings' size handed to
/// the screen size.
pub const OUTPUT: Site = Site {
    at: 0x224352b,
    what: "settings' output size applied",
    bytes: &[
        0x8b, 0x0d, 0x9b, 0x03, 0x70, 0x08, // mov ecx, [+0xa9438cc]
        0x8b, 0x86, 0x04, 0x02, 0x00, 0x00, // mov eax, [rsi+204h]
        0x39, 0xc1, 0x75, 0x0d, // cmp ecx, eax ; jne
        0x8b, 0x96, 0x08, 0x02, 0x00, 0x00, // mov edx, [rsi+208h]
        0xe8, 0xea, 0x4b, 0x23, 0x00, 0xeb, 0x20, // call +0x2478130 ; jmp +0x2243568
        0x8b, 0xbe, 0x08, 0x02, 0x00, 0x00, // mov edi, [rsi+208h]
        0x8b, 0x2d, 0x7c, 0x03, 0x70, 0x08, // mov ebp, [+0xa9438d0]
        0x89, 0xc1, 0x89, 0xfa, // mov ecx, eax ; mov edx, edi
        0xe8, 0xd3, 0x4b, 0x23, 0x00, // call +0x2478130
        0x39, 0xfd, 0x74, 0x07, 0xb1, 0x01, // cmp ebp, edi ; je ; mov cl, 1
        0xe8, 0x38, 0x8e, 0xb4, 0xfe, // call +0xd8c3a0
    ],
};

/// +0x247ad10, after the back buffers are made in their pool: `int3` unless
/// less than 2 MB of the pool is left over. Sized for 3840x2160. With
/// another output size the check becomes a jump over the `int3`.
pub const POOL_CHECK: Site = Site {
    at: 0x247af25,
    what: "back buffer pool's left-over check",
    bytes: &[0x48, 0x3d, 0xff, 0xff, 0x1f, 0x00, 0x76, 0x01, 0xcc], // cmp rax, 1FFFFFh ; jbe +1 ; int3
};

/// What goes at [`POOL_CHECK`]: `jmp +1` in place of `jbe +1`.
pub fn pool_check_code() -> Vec<u8> {
    let mut c = POOL_CHECK.bytes[..8].to_vec();
    c[6] = 0xeb;
    c
}

/// The title's own upscaler: `mov dword [rcx+90h], 2/3`.
pub const OWN_SCALE: Site = Site {
    at: 0x28f1e78,
    what: "the title's own upscaler's render scale",
    bytes: &[0xc7, 0x81, 0x90, 0x00, 0x00, 0x00, 0xab, 0xaa, 0x2a, 0x3f],
};

/// The upscaler picker, +0x7e93f0, to its first `ret`.
pub const PICKER: Site = Site {
    at: 0x7e93f0,
    what: "upscaler picker",
    bytes: &[
        0x48, 0x83, 0xec, 0x28, 0x80, 0x3d, 0x6d, 0xf0, 0x3a, 0x07, 0x00, 0x75, 0x09, 0x83, 0x3d, 0x68, 0xf0, 0x3a, 0x07, 0x00,
        0x74, 0x28, 0xe8, 0x35, 0xbd, 0x88, 0xff, 0x8a, 0x0d, 0x5b, 0xf0, 0x3a, 0x07, 0x80, 0xe1, 0x40, 0x80, 0xf9, 0x01, 0xb9,
        0x07, 0x00, 0x00, 0x00, 0x83, 0xd9, 0x00, 0x84, 0xc0, 0xb8, 0x06, 0x00, 0x00, 0x00, 0x0f, 0x45, 0xc1, 0x48, 0x83, 0xc4,
        0x28, 0xc3,
    ],
};

/// `rel32` from the end of an instruction at image offset `next` to `to`.
fn rel(to: usize, next: usize) -> [u8; 4] {
    ((to as i64 - next as i64) as i32).to_le_bytes()
}

/// What goes at [`OUTPUT`] for `width` x `height` (the same length).
pub fn output_code(width: u32, height: u32) -> Vec<u8> {
    let at = OUTPUT.at;
    let mut c = Vec::with_capacity(OUTPUT.bytes.len());
    c.push(0xb8); // mov eax, width
    c.extend(width.to_le_bytes());
    c.extend([0x89, 0x86, 0x04, 0x02, 0x00, 0x00]); // mov [rsi+204h], eax
    c.push(0xbf); // mov edi, height
    c.extend(height.to_le_bytes());
    c.extend([0x89, 0xbe, 0x08, 0x02, 0x00, 0x00]); // mov [rsi+208h], edi
    c.extend([0x8b, 0x2d]); // mov ebp, [screen height]
    c.extend(rel(SCREEN_HEIGHT, at + c.len() + 4));
    c.extend([0x89, 0xc1, 0x89, 0xfa]); // mov ecx, eax ; mov edx, edi
    c.push(0xe8); // call the setter
    c.extend(rel(SET_SCREEN_SIZE, at + c.len() + 4));
    let end = OUTPUT.bytes.len();
    c.extend([0x39, 0xfd, 0x74, (end - (c.len() + 4)) as u8]); // cmp ebp, edi ; je end
    c.extend([0xb1, 0x01, 0xe8]); // mov cl, 1 ; call
    c.extend(rel(HEIGHT_CHANGED, at + c.len() + 4));
    c.extend([0xeb, (end - (c.len() + 2)) as u8]); // jmp end
    c.resize(end, 0xcc);
    c
}

/// What goes at [`OWN_SCALE`] for `scale`.
pub fn own_scale_code(scale: f32) -> Vec<u8> {
    let mut c = OWN_SCALE.bytes[..6].to_vec();
    c.extend(scale.to_le_bytes());
    c
}

/// `mov eax, kind ; ret`.
pub fn picker_code(kind: u32) -> Vec<u8> {
    let mut c = vec![0xb8];
    c.extend(kind.to_le_bytes());
    c.push(0xc3);
    c
}

/// Compares `site`'s bytes in the image of `size` bytes at `base`.
///
/// # Safety
/// The image is mapped for `size` bytes at `base`.
pub unsafe fn matches(base: usize, size: usize, site: &Site) -> Patched {
    if site.at.checked_add(site.bytes.len()).is_none_or(|end| end > size) {
        return Patched::Failed("outside the image");
    }
    // SAFETY: inside the mapped image (checked above).
    let there = unsafe { std::slice::from_raw_parts((base + site.at) as *const u8, site.bytes.len()) };
    if there == site.bytes { Patched::Done } else { Patched::Differs }
}

/// Writes `code` over `site` if its bytes are the expected ones.
///
/// # Safety
/// The image stays mapped and no thread runs its code yet.
#[cfg(windows)]
pub unsafe fn write(base: usize, size: usize, site: &Site, code: &[u8]) -> Patched {
    use windows_sys::Win32::System::Diagnostics::Debug::FlushInstructionCache;
    use windows_sys::Win32::System::Memory::{PAGE_EXECUTE_READWRITE, VirtualProtect};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    if code.len() > site.bytes.len() {
        return Patched::Failed("replacement longer than the bytes compared");
    }
    // SAFETY: the caller's promise.
    let found = unsafe { matches(base, size, site) };
    if found != Patched::Done {
        return found;
    }
    let at = base + site.at;
    let mut old = 0;
    // SAFETY: inside the image (checked); made writable for this and put
    // back; nothing runs it yet.
    unsafe {
        if VirtualProtect(at as *const _, code.len(), PAGE_EXECUTE_READWRITE, &mut old) == 0 {
            return Patched::Failed("Windows would not make it writable");
        }
        std::ptr::copy_nonoverlapping(code.as_ptr(), at as *mut u8, code.len());
        VirtualProtect(at as *const _, code.len(), old, &mut old);
        FlushInstructionCache(GetCurrentProcess(), at as *const _, code.len());
    }
    Patched::Done
}

/// Applies what the switches ask for. A site whose bytes are not the ones
/// expected is left unchanged and said so.
///
/// # Safety
/// The image of `size` bytes at `base` stays mapped and no thread runs its
/// code yet.
#[cfg(windows)]
pub unsafe fn apply_asked(image: &str, base: usize, size: usize) {
    let mut asked: Vec<(&Site, Vec<u8>, String)> = Vec::new();
    if let Some((w, h)) = asked_output() {
        asked.push((&OUTPUT, output_code(w, h), format!("{w}x{h}")));
        if (w, h) != (3840, 2160) {
            asked.push((&POOL_CHECK, pool_check_code(), format!("{w}x{h}")));
        }
    }
    if let Some(s) = asked_render_scale() {
        asked.push((&OWN_SCALE, own_scale_code(s), format!("{s}")));
    }
    if let Some(kind) = asked_upscaler() {
        asked.push((&PICKER, picker_code(kind), format!("returns {kind}")));
    }
    for (site, code, value) in asked {
        // SAFETY: the caller's promise.
        let done = unsafe { write(base, size, site, &code) };
        let offset = format!("{:#x}", site.at);
        match done {
            Patched::Done => {
                tracing::info!(image, site = %offset, what = site.what, value, "resolution: patched")
            }
            Patched::Differs => tracing::warn!(
                image,
                site = %offset,
                what = site.what,
                "resolution: not patched, the bytes there are not the ones expected (another build?)"
            ),
            Patched::Failed(why) => {
                tracing::warn!(image, site = %offset, what = site.what, why, "resolution: not patched")
            }
        }
        println!("resolution: {} ({value}) at title+{offset}: {done:?}", site.what);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switches_parse() {
        assert_eq!(parse_output("1920x1080"), Some((1920, 1080)));
        assert_eq!(parse_output(" 2560X1440 "), Some((2560, 1440)));
        assert_eq!(parse_output("1600x900"), None);
        assert_eq!(parse_output("1080p"), None);
        assert_eq!(parse_render_scale("0.67"), Some(0.67));
        assert_eq!(parse_render_scale("0.5"), Some(0.5));
        assert_eq!(parse_render_scale("1"), Some(1.0));
        assert_eq!(parse_render_scale("0.4"), None);
        assert_eq!(parse_render_scale("1.5"), None);
    }

    /// The calls and loads in the bytes compared are the ones the
    /// replacement keeps.
    #[test]
    fn the_output_block_is_the_titles() {
        let b = OUTPUT.bytes;
        assert_eq!(b.len(), 61);
        let target = |op_end: usize| {
            (OUTPUT.at + op_end) as i64 + i64::from(i32::from_le_bytes(b[op_end - 4..op_end].try_into().unwrap()))
        };
        assert_eq!(target(27), SET_SCREEN_SIZE as i64);
        assert_eq!(target(50), SET_SCREEN_SIZE as i64);
        assert_eq!(target(61), HEIGHT_CHANGED as i64);
        assert_eq!(target(41), SCREEN_HEIGHT as i64);
        assert_eq!(target(6), (SCREEN_HEIGHT - 4) as i64, "the width before it");
        assert_eq!((b[14], 16 + b[15] as usize), (0x75, 29));
        assert_eq!((b[27], 29 + b[28] as usize), (0xeb, 61));
        assert_eq!((b[52], 54 + b[53] as usize), (0x74, 61));
    }

    #[test]
    fn the_output_code_keeps_the_titles_else_path() {
        let c = output_code(1920, 1080);
        assert_eq!(c.len(), OUTPUT.bytes.len());
        assert_eq!(c[0], 0xb8);
        assert_eq!(c[1..5], 1920u32.to_le_bytes());
        assert_eq!(c[5..11], [0x89, 0x86, 0x04, 0x02, 0x00, 0x00]);
        assert_eq!(c[12..16], 1080u32.to_le_bytes());
        assert_eq!(c[16..22], [0x89, 0xbe, 0x08, 0x02, 0x00, 0x00]);
        let target = |op_end: usize| {
            (OUTPUT.at + op_end) as i64 + i64::from(i32::from_le_bytes(c[op_end - 4..op_end].try_into().unwrap()))
        };
        assert_eq!(c[22..24], [0x8b, 0x2d]);
        assert_eq!(target(28), SCREEN_HEIGHT as i64);
        assert_eq!(c[32], 0xe8);
        assert_eq!(target(37), SET_SCREEN_SIZE as i64);
        assert_eq!(c[37..40], [0x39, 0xfd, 0x74]);
        assert_eq!(41 + c[40] as usize, 61, "je lands at the end");
        assert_eq!(c[43], 0xe8);
        assert_eq!(target(48), HEIGHT_CHANGED as i64);
        assert_eq!(c[48], 0xeb);
        assert_eq!(50 + c[49] as usize, 61, "jmp lands at the end");
        assert!(c[50..].iter().all(|&x| x == 0xcc));
    }

    #[test]
    fn the_scale_codes() {
        let own = own_scale_code(0.5);
        assert_eq!(own[..6], OWN_SCALE.bytes[..6]);
        assert_eq!(own[6..], 0.5f32.to_le_bytes());
        assert_eq!(OWN_SCALE.bytes[6..], (2.0f32 / 3.0).to_le_bytes());
        assert_eq!(picker_code(7), [0xb8, 7, 0, 0, 0, 0xc3]);
        assert_eq!(pool_check_code(), [0x48, 0x3d, 0xff, 0xff, 0x1f, 0x00, 0xeb, 0x01]);
    }

    /// A different image, or one too small to hold the site, is not a match.
    /// Nothing is written unless the bytes are the expected ones.
    #[test]
    fn a_different_image_is_not_a_match() {
        let mut image = vec![0u8; OWN_SCALE.at + OWN_SCALE.bytes.len()];
        let base = image.as_ptr() as usize;
        assert_eq!(unsafe { matches(base, image.len(), &OWN_SCALE) }, Patched::Differs);
        assert!(matches!(
            unsafe { matches(base, OWN_SCALE.at, &OWN_SCALE) },
            Patched::Failed(_)
        ));
        image[OWN_SCALE.at..].copy_from_slice(OWN_SCALE.bytes);
        assert_eq!(unsafe { matches(base, image.len(), &OWN_SCALE) }, Patched::Done);
        image[OWN_SCALE.at] ^= 0xff;
        assert_eq!(unsafe { matches(base, image.len(), &OWN_SCALE) }, Patched::Differs);
    }

    #[cfg(windows)]
    #[test]
    fn a_site_is_written_only_over_its_bytes() {
        use windows_sys::Win32::System::Memory::{
            MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READ, VirtualAlloc, VirtualFree,
        };
        // SAFETY: fresh memory of our own, read-only to code as the image's is.
        unsafe {
            let place = VirtualAlloc(std::ptr::null(), 0x1000, MEM_COMMIT | MEM_RESERVE, 0x40) as *mut u8;
            assert!(!place.is_null());
            let site = Site { at: 0x100, what: "test", bytes: OWN_SCALE.bytes };
            std::ptr::copy_nonoverlapping(site.bytes.as_ptr(), place.add(0x100), site.bytes.len());
            let mut old = 0;
            windows_sys::Win32::System::Memory::VirtualProtect(place.cast(), 0x1000, PAGE_EXECUTE_READ, &mut old);
            assert_eq!(write(place as usize, 0x1000, &site, &own_scale_code(0.5)), Patched::Done);
            assert_eq!(std::slice::from_raw_parts(place.add(0x106), 4), 0.5f32.to_le_bytes());
            // Already changed: its bytes differ now, nothing is written again.
            assert_eq!(write(place as usize, 0x1000, &site, &own_scale_code(0.75)), Patched::Differs);
            assert_eq!(std::slice::from_raw_parts(place.add(0x106), 4), 0.5f32.to_le_bytes());
            let far = Site { at: 0xffa, what: "test", bytes: OWN_SCALE.bytes };
            assert!(matches!(write(place as usize, 0x1000, &far, &own_scale_code(0.5)), Patched::Failed(_)));
            VirtualFree(place.cast(), 0, MEM_RELEASE);
        }
    }
}
