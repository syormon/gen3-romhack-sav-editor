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

/// Writes `value` into the bits of a u16 that `mask` selects, leaving the
/// others as they were.
pub fn set_u16_bits(b: &mut [u8], o: usize, mask: u16, value: u16) {
    let kept = u16_le(b, o) & !mask;
    set_u16_le(b, o, kept | (value & mask));
}

/// The same for a u32.
pub fn set_u32_bits(b: &mut [u8], o: usize, mask: u32, value: u32) {
    let kept = u32_le(b, o) & !mask;
    set_u32_le(b, o, kept | (value & mask));
}

/// A party record's mail slot when it holds no mail (`MAIL_NONE`). Slot 0 is a
/// real mail slot, so a zeroed record reads as carrying someone's letter.
pub const NO_MAIL: u8 = 0xFF;

/// The flag byte every record format here shares: bad egg, has species, is
/// egg in the low three bits. The rest belong to the game (pokeemerald uses
/// one for its Ruby/Sapphire box lock), so they are carried over untouched.
pub fn record_flags(existing: u8, is_bad_egg: bool, is_egg: bool) -> u8 {
    (existing & !0x07) | u8::from(is_bad_egg) | 0x02 | (u8::from(is_egg) << 2)
}

/// JavaScript's `x || fallback` for numbers: 0 counts as "unset".
pub fn nz<T: PartialEq + Default>(value: T, fallback: T) -> T {
    if value == T::default() {
        fallback
    } else {
        value
    }
}
