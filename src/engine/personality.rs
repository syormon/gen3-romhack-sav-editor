//! Personality-value (PID) surgery, ported from the helpers at the top of
//! `src/main.ts`.
//!
//! The PID encodes nature (`pid % 25`), the ability bit (`pid & 1`) and, with
//! the trainer id, shininess. Toggling shininess therefore means searching for
//! another PID that keeps nature and ability but flips the shiny test.
//!
//! The shiny test is `tid ^ sid ^ pid_hi ^ pid_lo < threshold`, and the
//! threshold is per game — 8 in vanilla, 16 in Unbound — so it is passed in.

/// The value the shiny test compares against the threshold.
fn shiny_xor(pid: u32, ot_id: u32) -> u32 {
    (ot_id & 0xFFFF) ^ (ot_id >> 16) ^ (pid >> 16) ^ (pid & 0xFFFF)
}

/// Searches for a PID with the same nature and ability bit whose shiny xor is
/// `target_xor`. Each high half pins the low half, so this is at most 65,536
/// tries, and in practice a few dozen.
fn search(current_pid: u32, ot_id: u32, target_xor: u32) -> Option<u32> {
    let trainer = (ot_id & 0xFFFF) ^ (ot_id >> 16);
    (0..=0xFFFFu32)
        .map(|high| (high << 16) | ((high ^ trainer ^ target_xor) & 0xFFFF))
        .find(|pid| pid % 25 == current_pid % 25 && pid & 1 == current_pid & 1)
}

/// Finds a PID with the same nature and ability bit that reads as shiny.
pub fn make_personality_shiny(current_pid: u32, ot_id: u32, threshold: u32) -> u32 {
    if shiny_xor(current_pid, ot_id) < threshold {
        return current_pid;
    }
    search(current_pid, ot_id, 0).unwrap_or(current_pid)
}

/// The inverse: same nature and ability bit, but not shiny under `threshold`.
pub fn make_personality_non_shiny(current_pid: u32, ot_id: u32, threshold: u32) -> u32 {
    if shiny_xor(current_pid, ot_id) >= threshold {
        return current_pid;
    }
    // 0x8000 is far past any game's threshold.
    search(current_pid, ot_id, 0x8000).unwrap_or(current_pid)
}

/// Rewrites just the nature, keeping the ability bit and otherwise staying as
/// close to the old PID as it can.
pub fn set_personality_nature(current_pid: u32, target_nature: u32) -> u32 {
    let target = target_nature % 25;
    let base = current_pid - current_pid % 25;
    // 25 is odd, so a neighbouring block has the other parity. The last block
    // below u32::MAX is incomplete, so stepping down has to stay an option
    // rather than wrapping round to a PID with some other nature.
    [
        Some(base),
        base.checked_add(25),
        base.checked_sub(25),
        base.checked_sub(50),
    ]
    .into_iter()
    .flatten()
    .filter_map(|block| block.checked_add(target))
    .find(|pid| pid & 1 == current_pid & 1)
    .unwrap_or(current_pid)
}

/// Rewrites just the ability bit, keeping the nature, whether it reads as shiny
/// under `threshold`, and the rest of the low byte, which the gender test
/// compares.
pub fn set_personality_ability_bit(current_pid: u32, ot_id: u32, threshold: u32, bit: u32) -> u32 {
    if current_pid & 1 == bit & 1 {
        return current_pid;
    }
    let shiny = shiny_xor(current_pid, ot_id) < threshold;
    (1..=0xFF_FFFFu32)
        .map(|step| current_pid.wrapping_add(step << 8) ^ 1)
        .find(|pid| pid % 25 == current_pid % 25 && (shiny_xor(*pid, ot_id) < threshold) == shiny)
        .unwrap_or(current_pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggling_respects_a_wider_threshold() {
        let ot = 0x1234_ABCD;
        let pid = 0x1357_9BDF;
        for threshold in [8, 16] {
            let shiny = make_personality_shiny(pid, ot, threshold);
            assert!(shiny_xor(shiny, ot) < threshold);
            let plain = make_personality_non_shiny(shiny, ot, threshold);
            assert!(
                shiny_xor(plain, ot) >= threshold,
                "threshold {threshold}: xor {}",
                shiny_xor(plain, ot)
            );
            for p in [shiny, plain] {
                assert_eq!(p % 25, pid % 25, "nature kept");
                assert_eq!(p & 1, pid & 1, "ability bit kept");
            }
        }
    }

    #[test]
    fn setting_the_nature_near_the_top_of_the_range_does_not_wrap() {
        for pid in [u32::MAX, u32::MAX - 3, 0, 24, 25] {
            for nature in 0..25 {
                let changed = set_personality_nature(pid, nature);
                assert_eq!(changed % 25, nature, "{pid:#x}");
                assert_eq!(changed & 1, pid & 1, "{pid:#x}: ability bit kept");
            }
        }
    }

    #[test]
    fn changing_the_ability_bit_keeps_the_rest_of_the_pokemon() {
        let ot = 0x1234_ABCD;
        let threshold = 16;
        let plain = 0x1357_9BDF;
        let shiny = make_personality_shiny(plain, ot, threshold);
        for pid in [plain, shiny, plain ^ 1, shiny ^ 1, u32::MAX, 0] {
            let was_shiny = shiny_xor(pid, ot) < threshold;
            for bit in [0, 1] {
                let changed = set_personality_ability_bit(pid, ot, threshold, bit);
                assert_eq!(changed & 1, bit, "{pid:#x}");
                assert_eq!(changed % 25, pid % 25, "{pid:#x}: nature kept");
                assert_eq!(changed & 0xFE, pid & 0xFE, "{pid:#x}: gender byte kept");
                assert_eq!(
                    shiny_xor(changed, ot) < threshold,
                    was_shiny,
                    "{pid:#x}: shininess kept"
                );
            }
        }
    }
}
