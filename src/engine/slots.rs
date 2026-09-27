//! Party and box slots: reading one, and the edits that move Pokémon between
//! them.
//!
//! Two rules hold after every edit here, because the games rely on them:
//!
//! * The party is packed to the front, and the stored party count matches it.
//!   A game reads exactly `count` records from slot 0, so a gap hides a
//!   Pokémon and turns an empty record into a party member.
//! * A record moved between slots keeps its bytes. The editor models only
//!   part of a record — ribbons, contest stats, markings and origin data are
//!   carried, not understood — so a move copies the record rather than
//!   rebuilding it from the fields the editor happens to know.

use super::layout::*;
use super::mon_writer::{clear_pokemon_slot, pack_and_write_pokemon};
use super::save_parser::{GameSave, Pokemon, unpack_mon};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum SlotKind {
    Party,
    Box,
}

/// Identifies one storage slot: a party position, or a slot in a given box.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct SlotRef {
    pub kind: SlotKind,
    pub index: usize,
    pub box_index: usize,
}

impl SlotRef {
    pub fn party(index: usize) -> Self {
        Self {
            kind: SlotKind::Party,
            index,
            box_index: 0,
        }
    }

    pub fn boxed(box_index: usize, index: usize) -> Self {
        Self {
            kind: SlotKind::Box,
            index,
            box_index,
        }
    }

    pub fn is_party(self) -> bool {
        self.kind == SlotKind::Party
    }

    /// Whether this is one of the boxes kept outside the storage block.
    pub fn is_extra_box(self) -> bool {
        !self.is_party() && self.box_index >= total_boxes()
    }

    /// Byte offset within the buffer this slot lives in: sb1 for the party,
    /// storage for a regular box, the extra-box records for the rest.
    pub fn offset(self) -> usize {
        if self.is_party() {
            party_offset() + self.index * mon_size()
        } else if self.is_extra_box() {
            ((self.box_index - total_boxes()) * box_capacity() + self.index) * box_mon_size()
        } else {
            storage_boxes_offset() + (self.box_index * box_capacity() + self.index) * box_mon_size()
        }
    }

    /// Bytes in one record of this slot's kind.
    pub fn size(self) -> usize {
        if self.is_party() {
            mon_size()
        } else {
            box_mon_size()
        }
    }
}

/// Why an edit was refused.
pub const LAST_POKEMON: &str = "Your party cannot be left empty.";

impl GameSave {
    fn buffer_for(&self, slot: SlotRef) -> &[u8] {
        if slot.is_party() {
            &self.sb1
        } else if slot.is_extra_box() {
            &self.extra
        } else {
            &self.storage
        }
    }

    fn buffer_for_mut(&mut self, slot: SlotRef) -> &mut [u8] {
        if slot.is_party() {
            &mut self.sb1
        } else if slot.is_extra_box() {
            &mut self.extra
        } else {
            &mut self.storage
        }
    }

    /// A slot's record bytes, if the slot exists in this save.
    pub(super) fn record(&self, slot: SlotRef) -> Option<&[u8]> {
        if !self.in_range(slot) {
            return None;
        }
        let at = slot.offset();
        Some(&self.buffer_for(slot)[at..at + slot.size()])
    }

    /// Whether `slot` names a record inside the save, so indexing it is safe.
    fn in_range(&self, slot: SlotRef) -> bool {
        let limit = if slot.is_party() {
            party_size()
        } else {
            box_capacity()
        };
        let box_ok = slot.is_party() || slot.box_index < self.box_count();
        box_ok && slot.index < limit && slot.offset() + slot.size() <= self.buffer_for(slot).len()
    }

    /// The Pokémon in a slot, if there is one.
    pub fn mon_at(&self, slot: SlotRef) -> Option<Pokemon> {
        if !self.in_range(slot) {
            return None;
        }
        if slot.is_party() {
            return self.party_slot(slot.index);
        }
        self.box_pokemon(slot.box_index, slot.index)
    }

    /// The raw bytes of a slot's record.
    fn raw_record(&self, slot: SlotRef) -> Vec<u8> {
        let at = slot.offset();
        self.buffer_for(slot)[at..at + slot.size()].to_vec()
    }

    /// Writes `mon` over the record already in `slot`, keeping whatever bytes
    /// of it the editor does not model. This is what "Apply Changes" does.
    pub fn write_mon(&mut self, slot: SlotRef, mon: &Pokemon) {
        if !self.in_range(slot) {
            return;
        }
        let at = slot.offset();
        pack_and_write_pokemon(self.buffer_for_mut(slot), at, mon, slot.is_party());
    }

    /// Commits an edited Pokémon, refreshing a party member's stats when the
    /// edit touched anything they are computed from.
    ///
    /// An unchanged Pokémon is written back exactly as it was read, so opening
    /// one and pressing Apply leaves the save byte-for-byte alone.
    pub fn apply_edit(&mut self, slot: SlotRef, edited: &Pokemon) {
        let mut mon = edited.clone();
        if slot.is_party()
            && self
                .mon_at(slot)
                .is_some_and(|before| super::stats::inputs_changed(&before, &mon))
        {
            super::stats::refresh(&mut mon);
        }
        self.write_mon(slot, &mon);
    }

    /// Puts a brand-new Pokémon in an empty slot. Into the party, it goes
    /// after the last member rather than leaving a gap.
    pub fn create_mon(&mut self, slot: SlotRef, mon: &Pokemon) {
        let slot = if slot.is_party() {
            SlotRef::party(slot.index.min(self.party().len()))
        } else {
            slot
        };
        if !self.in_range(slot) || self.mon_at(slot).is_some() {
            return;
        }
        // Start from a clean record: an "empty" slot can still hold the bytes
        // of whatever was there before, and they would otherwise leak into the
        // new Pokémon's unmodelled fields.
        self.clear_slot(slot);
        self.write_mon(slot, mon);
        self.compact_party();
    }

    /// Moves a Pokémon to another slot, swapping with whatever is there.
    pub fn move_mon(&mut self, src: SlotRef, dst: SlotRef) -> Result<(), &'static str> {
        if src == dst || !self.in_range(src) || !self.in_range(dst) {
            return Ok(());
        }
        let Some(src_mon) = self.mon_at(src) else {
            return Ok(());
        };
        let dst_mon = self.mon_at(dst);
        if src.is_party() && !dst.is_party() && dst_mon.is_none() && self.party().len() <= 1 {
            return Err(LAST_POKEMON);
        }

        let src_raw = self.raw_record(src);
        let dst_raw = self.raw_record(dst);
        self.place(dst, &src_raw, src, &src_mon);
        match dst_mon {
            Some(dst_mon) => self.place(src, &dst_raw, dst, &dst_mon),
            None => self.clear_slot(src),
        }
        self.compact_party();
        Ok(())
    }

    /// Empties a slot, closing the gap if it was in the party.
    pub fn release_mon(&mut self, slot: SlotRef) -> Result<(), &'static str> {
        if !self.in_range(slot) || self.mon_at(slot).is_none() {
            return Ok(());
        }
        if slot.is_party() && self.party().len() <= 1 {
            return Err(LAST_POKEMON);
        }
        self.clear_slot(slot);
        self.compact_party();
        Ok(())
    }

    fn clear_slot(&mut self, slot: SlotRef) {
        let at = slot.offset();
        clear_pokemon_slot(self.buffer_for_mut(slot), at, slot.is_party());
    }

    /// Writes a record that came from `from` into `slot`.
    ///
    /// Between slots of the same kind that is a straight copy. Between the
    /// party and a box the record changes shape, so what the formats share is
    /// copied and the rest is rebuilt: a party member gets battle stats worked
    /// out from its level, and one going into a box loses them, as in game.
    fn place(&mut self, slot: SlotRef, raw: &[u8], from: SlotRef, mon: &Pokemon) {
        self.clear_slot(slot);
        let at = slot.offset();
        let size = slot.size();

        if from.kind == slot.kind {
            self.buffer_for_mut(slot)[at..at + size].copy_from_slice(&raw[..size]);
            return;
        }

        // A boxed record is the front of a party record, except where a game
        // compacts its boxes into a layout of their own.
        if !boxes_are_cfru_compact() {
            let shared = box_mon_size().min(raw.len()).min(size);
            self.buffer_for_mut(slot)[at..at + shared].copy_from_slice(&raw[..shared]);
        }
        let mut mon = mon.clone();
        if slot.is_party() {
            super::stats::refresh(&mut mon);
            mon.status = Some(0);
        }
        pack_and_write_pokemon(self.buffer_for_mut(slot), at, &mon, slot.is_party());
    }

    /// Packs the party to the front and restates its count.
    ///
    /// Records are moved as raw bytes, so this never alters a Pokémon — only
    /// where it sits.
    pub fn compact_party(&mut self) {
        let occupied: Vec<Vec<u8>> = (0..party_size())
            .map(SlotRef::party)
            .filter(|slot| self.in_range(*slot) && party_slot_is_occupied(&self.sb1, *slot))
            .map(|slot| self.raw_record(slot))
            .collect();

        for i in 0..party_size() {
            let slot = SlotRef::party(i);
            if !self.in_range(slot) {
                break;
            }
            let at = slot.offset();
            match occupied.get(i) {
                Some(record) => self.sb1[at..at + record.len()].copy_from_slice(record),
                None => clear_pokemon_slot(&mut self.sb1, at, true),
            }
        }
        self.refresh_party_count();
    }
}

/// Whether a party record holds anything, the same test the party count uses.
pub(super) fn party_slot_is_occupied(sb1: &[u8], slot: SlotRef) -> bool {
    let at = slot.offset();
    sb1.get(at..at + 8)
        .is_some_and(|header| header.iter().any(|b| *b != 0))
}

/// Decodes the party record at `index`, ignoring the stored count.
pub(super) fn unpack_party_record(sb1: &[u8], index: usize) -> Option<Pokemon> {
    let at = party_offset() + index * mon_size();
    unpack_mon(sb1.get(at..at + mon_size())?, true).filter(|mon| mon.species > 0)
}
