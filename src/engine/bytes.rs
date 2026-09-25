//! Little-endian byte accessors, replacing the browser's `DataView`.
//!
//! Reads are bounds-checked and return 0 past the end of the slice, which keeps
//! a truncated save file from panicking mid-parse. Writes past the end are
//! silently dropped for the same reason.

pub fn u16_le(b: &[u8], o: usize) -> u16 {
    match (b.get(o), b.get(o + 1)) {
        (Some(&lo), Some(&hi)) => u16::from_le_bytes([lo, hi]),
        _ => 0,
    }
}

pub fn u32_le(b: &[u8], o: usize) -> u32 {
    if o + 4 <= b.len() {
        u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
    } else {
        0
    }
}

pub fn set_u16_le(b: &mut [u8], o: usize, v: u16) {
    if o + 2 <= b.len() {
        b[o..o + 2].copy_from_slice(&v.to_le_bytes());
    }
}

pub fn set_u32_le(b: &mut [u8], o: usize, v: u32) {
    if o + 4 <= b.len() {
        b[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
}

/// JavaScript's `x || fallback` for numbers: 0 counts as "unset".
pub fn nz<T: PartialEq + Default>(value: T, fallback: T) -> T {
    if value == T::default() {
        fallback
    } else {
        value
    }
}
