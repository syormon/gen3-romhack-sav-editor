//! Save-file geometry for the game currently loaded.
//!
//! These were `const`s describing one hack. They are now accessors over the
//! active game pack's manifest, so a different hack only needs a different
//! `game.json` — see `src/game/manifest.rs` for the fields and their defaults.

use crate::game;

#[inline]
pub fn sector_size() -> usize {
    game::current().layout().sector_size
}

#[inline]
pub fn sector_data_size() -> usize {
    game::current().layout().sector_data_size
}

#[inline]
pub fn sector_signature() -> u32 {
    game::current().layout().sector_signature
}

#[inline]
pub fn sectors_per_slot() -> usize {
    game::current().layout().sectors_per_slot
}

#[inline]
pub fn min_save_size() -> usize {
    game::current().layout().min_save_size
}

#[inline]
pub fn sb1_size() -> usize {
    game::current().layout().sb1_size
}

#[inline]
pub fn sb2_size() -> usize {
    game::current().layout().sb2_size
}

#[inline]
pub fn storage_size() -> usize {
    game::current().layout().storage_size
}

#[inline]
pub fn player_name_offset() -> usize {
    game::current().layout().player_name
}

#[inline]
pub fn player_name_length() -> usize {
    game::current().layout().player_name_length
}

#[inline]
pub fn player_gender_offset() -> usize {
    game::current().layout().player_gender
}

#[inline]
pub fn trainer_id_offset() -> usize {
    game::current().layout().trainer_id
}

#[inline]
pub fn encryption_key_offset() -> Option<usize> {
    game::current().layout().encryption_key
}

#[inline]
pub fn party_count_offset() -> usize {
    game::current().layout().party_count
}

#[inline]
pub fn party_offset() -> usize {
    game::current().layout().party
}

#[inline]
pub fn money_offset() -> usize {
    game::current().layout().money
}

#[inline]
pub fn coins_offset() -> usize {
    game::current().layout().coins
}

#[inline]
pub fn max_money() -> u32 {
    game::current().layout().max_money
}

#[inline]
pub fn max_coins() -> u32 {
    game::current().layout().max_coins
}

#[inline]
pub fn bag_offset() -> usize {
    game::current().layout().bag
}

#[inline]
pub fn party_size() -> usize {
    game::current().layout().party_size
}

#[inline]
pub fn mon_size() -> usize {
    game::current().layout().mon_size
}

/// True when boxed records use CFRU's 58-byte compact form rather than the
/// game's party encoding.
#[inline]
pub fn boxes_are_cfru_compact() -> bool {
    game::current().layout().box_encoding == crate::game::BoxEncoding::CfruCompact
}

#[inline]
pub fn box_mon_size() -> usize {
    game::current().layout().box_mon_size
}

#[inline]
pub fn total_boxes() -> usize {
    game::current().layout().total_boxes
}

#[inline]
pub fn box_capacity() -> usize {
    game::current().layout().box_capacity
}

#[inline]
pub fn storage_boxes_offset() -> usize {
    game::current().layout().storage_boxes
}

#[inline]
pub fn storage_box_names_offset() -> usize {
    game::current().layout().storage_box_names
}

#[inline]
pub fn box_name_length() -> usize {
    game::current().layout().box_name_length
}

#[inline]
pub fn storage_sectors_start() -> usize {
    game::current().layout().storage_sectors.0
}

#[inline]
pub fn storage_sectors_end() -> usize {
    game::current().layout().storage_sectors.1
}

/// Bytes covered by a sector's checksum.
#[inline]
pub fn sector_checksum_size(sector: usize) -> usize {
    game::current().layout().sector_checksum_size(sector)
}

// ------------------------------------------------------------------- pockets
//
// Bag pockets are per-game, so they are addressed by index into
// `layout.pockets` rather than by a fixed enum.

#[inline]
pub fn pocket_count() -> usize {
    game::current().layout().pockets.len()
}

pub fn pocket_name(index: usize) -> String {
    game::current()
        .layout()
        .pocket(index)
        .map(|p| p.name.clone())
        .unwrap_or_default()
}

pub fn pocket_capacity(index: usize) -> usize {
    game::current()
        .layout()
        .pocket(index)
        .map(|p| p.count)
        .unwrap_or(0)
}

/// Which buffer a pocket's slots live in.
pub fn pocket_region(index: usize) -> crate::game::PocketRegion {
    game::current()
        .layout()
        .pocket(index)
        .map_or(crate::game::PocketRegion::SaveBlock1, |p| p.region)
}

pub fn pocket_offset(index: usize) -> usize {
    game::current()
        .layout()
        .pocket(index)
        .map(|p| p.offset)
        .unwrap_or(0)
}
