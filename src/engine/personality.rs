//! Personality-value (PID) surgery, ported from the helpers at the top of
//! `src/main.ts`.
//!
//! The PID encodes nature (`pid % 25`), the ability bit (`pid & 1`) and, with
//! the trainer id, shininess. Toggling shininess therefore means searching for
//! another PID that keeps nature and ability but flips the shiny test.

/// Finds a PID with the same nature and ability bit that reads as shiny.
pub fn make_personality_shiny(current_pid: u32, ot_id: u32) -> u32 {
    let tid = ot_id & 0xFFFF;
    let sid = (ot_id >> 16) & 0xFFFF;
    let desired_xor = tid ^ sid;
    let orig_nature = current_pid % 25;
    let orig_ability_bit = current_pid & 1;

    for high in 0..=0xFFFFu32 {
        let low = (high ^ desired_xor) & 0xFFFF;
        let test_pid = (high << 16) | low;
        if test_pid % 25 == orig_nature
            && (test_pid & 1) == orig_ability_bit
            && (tid ^ sid ^ (test_pid >> 16) ^ (test_pid & 0xFFFF)) < 8
        {
            return test_pid;
        }
    }
    current_pid ^ 0x0001_0000
}

/// The inverse: same nature and ability bit, but not shiny.
pub fn make_personality_non_shiny(current_pid: u32, ot_id: u32) -> u32 {
    let tid = ot_id & 0xFFFF;
    let sid = (ot_id >> 16) & 0xFFFF;
    let desired_xor = tid ^ sid;
    let orig_nature = current_pid % 25;
    let orig_ability_bit = current_pid & 1;

    for high in 0..=0xFFFFu32 {
        let low = (high ^ desired_xor ^ 0x000F) & 0xFFFF;
        let test_pid = (high << 16) | low;
        if test_pid % 25 == orig_nature
            && (test_pid & 1) == orig_ability_bit
            && (tid ^ sid ^ (test_pid >> 16) ^ (test_pid & 0xFFFF)) >= 8
        {
            return test_pid;
        }
    }
    current_pid ^ 0x0008_0000
}

/// Rewrites just the nature, keeping the rest of the PID.
pub fn set_personality_nature(current_pid: u32, target_nature: u32) -> u32 {
    let base = current_pid - (current_pid % 25);
    base.wrapping_add(target_nature)
}
