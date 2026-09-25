//! Port of `src/engine/SaveWriter.ts`: rebuilds both save slots, recomputes
//! sector checksums and bumps the slot counters.

use super::bytes::*;
use super::layout::*;

fn calc_sector_checksum(data: &[u8], size: usize) -> u16 {
    let mut chk: u32 = 0;
    for i in 0..(size / 4) {
        chk = chk.wrapping_add(u32_le(data, i * 4));
    }
    (((chk >> 16) + chk) & 0xFFFF) as u16
}

pub fn export_updated_save(
    original: &[u8],
    active_slot: usize,
    sb1: &[u8],
    sb2: &[u8],
    storage: &[u8],
) -> Vec<u8> {
    let mut raw = original.to_vec();

    // Only slots the game has actually written count; an erased one reads as
    // 0xFFFFFFFF and would overflow the increments below.
    let c0 = super::save_parser::slot_counter(&raw, 0).unwrap_or(0);
    let c1 = super::save_parser::slot_counter(&raw, 1).unwrap_or(0);
    let base_counter = c0.max(c1).min(u32::MAX - 2);

    // The active slot always ends up with the highest counter so the game loads
    // the edited data; the backup slot trails it by one.
    let (slot0_counter, slot1_counter) = if active_slot == 0 {
        if base_counter % 2 == 1 {
            (base_counter + 1, base_counter)
        } else {
            (base_counter + 2, base_counter + 1)
        }
    } else if base_counter % 2 == 0 {
        (base_counter, base_counter + 1)
    } else {
        (base_counter + 1, base_counter + 2)
    };

    let active_counter = if active_slot == 0 {
        slot0_counter
    } else {
        slot1_counter
    };
    let backup_slot = 1 - active_slot;
    let backup_counter = if backup_slot == 0 {
        slot0_counter
    } else {
        slot1_counter
    };

    // Where each logical sector currently lives, so its bytes can be carried
    // over rather than invented.
    let mut source: Vec<Option<usize>> = vec![None; sectors_per_slot()];
    for slot in 0..2 {
        for i in 0..sectors_per_slot() {
            let at = (sectors_per_slot() * slot + i) * sector_size();
            if at + sector_size() > raw.len() {
                continue;
            }
            if u32_le(&raw, at + 0xFF8) != sector_signature() {
                continue;
            }
            let sid = u16_le(&raw, at + 0xFF4) as usize;
            let newer = source[sid]
                .is_none_or(|prev| u32_le(&raw, at + 0xFFC) >= u32_le(&raw, prev + 0xFFC));
            if sid < sectors_per_slot() && newer {
                source[sid] = Some(at);
            }
        }
    }

    // Build the 14 logical sectors once, then stamp them into both slots.
    //
    // Each starts as a copy of what the game last wrote there, so anything the
    // editor does not model survives. That is not hypothetical padding: a
    // sector's checksum covers only the save block it carries, and CFRU hacks
    // keep live data in the bytes past that point — zero-filling them would
    // quietly discard part of the save.
    let mut sector_data: Vec<Vec<u8>> = Vec::with_capacity(sectors_per_slot());
    for sid in 0..sectors_per_slot() {
        let mut sec = match source[sid] {
            Some(at) => raw[at..at + sector_size()].to_vec(),
            None => vec![0u8; sector_size()],
        };
        if sid == 0 {
            sec[..sb2_size()].copy_from_slice(&sb2[..sb2_size()]);
        } else if (1..=4).contains(&sid) {
            let start = (sid - 1) * sector_data_size();
            let end = (start + sector_data_size()).min(sb1_size());
            sec[..end - start].copy_from_slice(&sb1[start..end]);
        } else if (storage_sectors_start()..=storage_sectors_end()).contains(&sid) {
            let start = (sid - storage_sectors_start()) * sector_data_size();
            let end = (start + sector_data_size()).min(storage_size());
            sec[..end - start].copy_from_slice(&storage[start..end]);
        }
        sector_data.push(sec);
    }

    for (slot, counter) in [(active_slot, active_counter), (backup_slot, backup_counter)] {
        for sid in 0..sectors_per_slot() {
            let mut sec = sector_data[sid].clone();
            let exp_size = sector_checksum_size(sid);
            let chk = calc_sector_checksum(&sec[..exp_size], exp_size);

            set_u16_le(&mut sec, 0xFF4, sid as u16);
            set_u16_le(&mut sec, 0xFF6, chk);
            set_u32_le(&mut sec, 0xFF8, sector_signature());
            set_u32_le(&mut sec, 0xFFC, counter);

            let phys_idx = sectors_per_slot() * slot + sid;
            let dst = phys_idx * sector_size();
            if dst + sector_size() <= raw.len() {
                raw[dst..dst + sector_size()].copy_from_slice(&sec);
            }
        }
    }

    // Hall of fame / mystery gift / trainer hill sector counters — but only
    // where the game keeps a real save sector. An erased one should stay
    // erased, and a hack may have repurposed the space for something whose
    // footer means nothing: Unbound keeps most of its bag in sector 30.
    for (sector, counter) in [
        (28, slot0_counter),
        (30, slot0_counter),
        (29, slot1_counter),
        (31, slot1_counter),
    ] {
        let at = sector * sector_size();
        if at + sector_size() <= raw.len() && u32_le(&raw, at + 0xFF8) == sector_signature() {
            set_u32_le(&mut raw, at + 0xFFC, counter);
        }
    }

    raw
}
