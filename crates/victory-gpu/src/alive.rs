//! Host GPU objects alive by kind, counted down when Direct3D destroys
//! them: each tracked object holds a private-data interface that Direct3D
//! releases with the object. The fork's memory line counts what was made;
//! this says what is still held (a measurement for the growth of private
//! memory in 3D scenes that no other line accounts for).

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use windows::Win32::Graphics::Direct3D12::ID3D12Object;
use windows::core::{GUID, IUnknown, Interface, implement};

pub(crate) const UPLOAD: usize = 0;
pub(crate) const SHARED: usize = 1;
pub(crate) const READ_BACK: usize = 2;
pub(crate) const CARD: usize = 3;
pub(crate) const TEXTURE: usize = 4;
const NAMES: [&str; 5] = ["upload buffers", "shared buffers", "read-back buffers", "card buffers", "host textures"];

/// Count and bytes alive, by kind.
static ALIVE: [[AtomicU64; 2]; 5] = [const { [const { AtomicU64::new(0) }; 2] }; 5];

const KEY: GUID = GUID::from_u128(0x5f0c_2b1e_9a4d_4c7e_8b31_76d2_e4a9_c013);

#[implement()]
struct Gone {
    kind: usize,
    bytes: u64,
}

impl Drop for Gone {
    fn drop(&mut self) {
        ALIVE[self.kind][0].fetch_sub(1, Relaxed);
        ALIVE[self.kind][1].fetch_sub(self.bytes, Relaxed);
    }
}

/// Counts `object` as alive until Direct3D destroys it.
pub(crate) fn track<T: Interface>(object: &T, kind: usize, bytes: u64) {
    let Ok(o) = object.cast::<ID3D12Object>() else {
        return;
    };
    ALIVE[kind][0].fetch_add(1, Relaxed);
    ALIVE[kind][1].fetch_add(bytes, Relaxed);
    let gone: IUnknown = Gone { kind, bytes }.into();
    // SAFETY: a live object; Direct3D holds the interface and releases it
    // when the object is destroyed (which runs `Gone::drop`).
    if unsafe { o.SetPrivateDataInterface(&KEY, &gone) }.is_err() {
        // Not held: undo the count when `gone` drops here.
    }
}

/// "host objects alive: ..." for the memory line.
pub(crate) fn line() -> String {
    let mut s = String::from("host objects alive (counted down at destruction):");
    for (kind, name) in NAMES.iter().enumerate() {
        s += &format!(
            " {} {} {:.0} MB;",
            ALIVE[kind][0].load(Relaxed),
            name,
            ALIVE[kind][1].load(Relaxed) as f64 / 1048576.0
        );
    }
    s
}
