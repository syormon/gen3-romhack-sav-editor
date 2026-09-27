//! Port of `src/engine/charmap.ts`: the in-game text encoding.

const TABLE: &[(u8, char)] = &[
    (0x00, ' '),
    // Punctuation the web original left out. An unmapped byte decodes as
    // "?" and is written back that way, so "Sirfetch\u{2019}d" lost its
    // apostrophe the moment the editor touched it. These are the standard
    // English Gen 3 values, confirmed against real Unbound box data.
    (0x1B, 'é'),
    (0x2D, '&'),
    (0x2E, '+'),
    (0xB0, '…'),
    (0xB1, '“'),
    (0xB2, '”'),
    (0xB3, '‘'),
    (0xB4, '\''),
    (0xB9, '×'),
    (0xAB, '!'),
    (0xAC, '?'),
    (0xAD, '.'),
    (0xAE, '-'),
    (0xB5, '♂'),
    (0xB6, '♀'),
    (0xB8, ','),
    (0xBA, '/'),
    (0xBB, 'A'),
    (0xBC, 'B'),
    (0xBD, 'C'),
    (0xBE, 'D'),
    (0xBF, 'E'),
    (0xC0, 'F'),
    (0xC1, 'G'),
    (0xC2, 'H'),
    (0xC3, 'I'),
    (0xC4, 'J'),
    (0xC5, 'K'),
    (0xC6, 'L'),
    (0xC7, 'M'),
    (0xC8, 'N'),
    (0xC9, 'O'),
    (0xCA, 'P'),
    (0xCB, 'Q'),
    (0xCC, 'R'),
    (0xCD, 'S'),
    (0xCE, 'T'),
    (0xCF, 'U'),
    (0xD0, 'V'),
    (0xD1, 'W'),
    (0xD2, 'X'),
    (0xD3, 'Y'),
    (0xD4, 'Z'),
    (0xD5, 'a'),
    (0xD6, 'b'),
    (0xD7, 'c'),
    (0xD8, 'd'),
    (0xD9, 'e'),
    (0xDA, 'f'),
    (0xDB, 'g'),
    (0xDC, 'h'),
    (0xDD, 'i'),
    (0xDE, 'j'),
    (0xDF, 'k'),
    (0xE0, 'l'),
    (0xE1, 'm'),
    (0xE2, 'n'),
    (0xE3, 'o'),
    (0xE4, 'p'),
    (0xE5, 'q'),
    (0xE6, 'r'),
    (0xE7, 's'),
    (0xE8, 't'),
    (0xE9, 'u'),
    (0xEA, 'v'),
    (0xEB, 'w'),
    (0xEC, 'x'),
    (0xED, 'y'),
    (0xEE, 'z'),
    (0xA1, '0'),
    (0xA2, '1'),
    (0xA3, '2'),
    (0xA4, '3'),
    (0xA5, '4'),
    (0xA6, '5'),
    (0xA7, '6'),
    (0xA8, '7'),
    (0xA9, '8'),
    (0xAA, '9'),
];

fn decode_byte(b: u8) -> Option<char> {
    if crate::game::is_loaded() {
        let pack = crate::game::current();
        if !pack.charmap.is_empty() {
            return pack.charmap.get(&b).copied();
        }
    }
    TABLE.iter().find(|(code, _)| *code == b).map(|(_, c)| *c)
}

fn encode_char(c: char) -> Option<u8> {
    if crate::game::is_loaded() {
        let pack = crate::game::current();
        if !pack.charmap.is_empty() {
            return pack
                .charmap
                .iter()
                .find(|(_, ch)| **ch == c)
                .map(|(code, _)| *code);
        }
    }
    if c == ' ' {
        return Some(0x00);
    }
    TABLE.iter().find(|(_, ch)| *ch == c).map(|(code, _)| *code)
}

/// Terminator byte used for unused name slots.
pub const TERMINATOR: u8 = 0xFF;

pub fn decode_gba_string(bytes: &[u8], max_length: usize) -> String {
    let mut result = String::new();
    for (i, &b) in bytes.iter().enumerate() {
        if i >= max_length {
            break;
        }
        // Only 0xFF ends a string. 0x00 is the space character; the web
        // original also treated it as a terminator, which truncated every name
        // containing a space ("LUGIA SHADOW" read back as "LUGIA", and was then
        // written back that way). Trailing 0x00 padding is handled by the trim
        // below.
        if b == 0xFF {
            break;
        }
        result.push(decode_byte(b).unwrap_or('?'));
    }
    result.trim().to_string()
}

pub fn encode_gba_string(text: &str, max_length: usize) -> Vec<u8> {
    let mut buf = vec![TERMINATOR; max_length];
    let chars: Vec<char> = text.chars().collect();
    for i in 0..max_length {
        if i < chars.len() {
            buf[i] = encode_char(chars[i]).unwrap_or(0x00);
        }
    }
    buf
}

/// Writes `text` into a name field that already has bytes in it.
///
/// Only the name and its terminator are written; whatever follows the
/// terminator is left alone. The games do the same — they copy up to the
/// terminator and never clear the tail — so a name written back unchanged
/// leaves the record byte-for-byte identical. Filling the tail instead makes
/// every record the editor touches differ from the game's own bytes.
pub fn write_gba_string(dst: &mut [u8], text: &str) {
    let mut written = 0;
    for c in text.chars().take(dst.len()) {
        dst[written] = encode_char(c).unwrap_or(0x00);
        written += 1;
    }
    if written < dst.len() {
        dst[written] = TERMINATOR;
    }
}

/// The editable character set, used to validate name inputs.
pub fn is_encodable(c: char) -> bool {
    encode_char(c).is_some()
}
