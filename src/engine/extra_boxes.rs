//! Boxes a game keeps outside the storage block (`layout.extra_boxes`).
//!
//! Their records can be split across several places, so the editor does not
//! edit them in place. Loading joins each box's pieces into one run of
//! records, after the regular boxes' numbering; the slot code then treats that
//! run like any other box. Export splits it back out and rewrites every
//! checksum the pieces are covered by.

use crate::game::{Area, ExtraBoxes};

use super::bytes::*;
use super::charmap::decode_gba_string;
use super::layout::*;

/// The buffers an extra-box position can point into, for one save slot.
struct Areas<'a> {
    file: &'a [u8],
    sb1: &'a [u8],
    storage: &'a [u8],
    /// File offset of each extra sector, for this slot.
    sectors: Vec<usize>,
}

impl Areas<'_> {
    fn get(&self, area: Area, sector: usize, offset: usize, length: usize) -> Option<&[u8]> {
        let (buf, start) = match area {
            Area::Storage => (self.storage, offset),
            Area::Sb1 => (self.sb1, offset),
            Area::Extra => (self.file, self.sectors.get(sector)? + offset),
        };
        buf.get(start..start + length)
    }
}

fn config() -> Option<ExtraBoxes> {
    crate::game::current().layout().extra_boxes.clone()
}

fn sector_offsets(cfg: &ExtraBoxes, slot: usize) -> Vec<usize> {
    cfg.sectors
        .iter()
        .map(|pair| pair[slot.min(1)] * sector_size())
        .collect()
}

/// What the extra boxes hold in a save, read from the slot the game would load.
pub struct Loaded {
    /// Every extra box's records, one box after another.
    pub records: Vec<u8>,
    pub names: Vec<String>,
}

/// Reads the extra boxes, or `None` if this game has none or this save does
/// not carry them — an unwritten sector, or a missing marker, means an older
/// save format that the layout does not describe.
pub fn load(file: &[u8], sb1: &[u8], storage: &[u8], slot: usize) -> Option<Loaded> {
    let cfg = config()?;
    let areas = Areas {
        file,
        sb1,
        storage,
        sectors: sector_offsets(&cfg, slot),
    };
    let signed = areas.sectors.iter().all(|at| {
        at + sector_size() <= file.len() && u32_le(file, at + 0xFF8) == sector_signature()
    });
    let marked = cfg
        .markers
        .iter()
        .all(|m| areas.get(m.area, m.sector, m.offset, m.text.len()) == Some(m.text.as_bytes()));
    if !signed || !marked {
        return None;
    }

    let mut records = Vec::with_capacity(cfg.boxes.len() * box_capacity() * box_mon_size());
    let mut names = Vec::with_capacity(cfg.boxes.len());
    for b in &cfg.boxes {
        for span in &b.records {
            records.extend_from_slice(areas.get(
                span.area,
                span.sector,
                span.offset,
                span.length,
            )?);
        }
        let width = box_name_length();
        let name = areas.get(b.name.area, b.name.sector, b.name.offset, width)?;
        names.push(decode_gba_string(name, width));
    }
    Some(Loaded { records, names })
}

/// Copies the extra boxes' records back into SaveBlock1 and storage, before
/// those are cut into sectors — so the ordinary sector checksums cover them.
pub fn store_in_blocks(records: &[u8], sb1: &mut [u8], storage: &mut [u8]) {
    let Some(cfg) = config() else { return };
    for (span, piece) in pieces(&cfg, records) {
        let buf: &mut [u8] = match span.area {
            Area::Sb1 => sb1,
            Area::Storage => storage,
            Area::Extra => continue,
        };
        if let Some(dst) = buf.get_mut(span.offset..span.offset + piece.len()) {
            dst.copy_from_slice(piece);
        }
    }
}

/// Writes the extra sectors of an exported file, for both save slots.
///
/// The export has already made both slots hold the same data, so each slot's
/// copy of the extra sectors is rebuilt from the loaded slot's: its bytes,
/// then the records, then the hack's block checksums, then each sector's own
/// footer checksum. Sector ids and counters are left as the export set them.
pub fn store_in_sectors(
    file: &mut [u8],
    loaded_slot: usize,
    records: &[u8],
    sb1: &[u8],
    storage: &[u8],
) {
    let Some(cfg) = config() else { return };
    let source = sector_offsets(&cfg, loaded_slot);
    let originals: Vec<Vec<u8>> = source
        .iter()
        .map(|&at| file.get(at..at + 0xFF4).map(<[u8]>::to_vec))
        .collect::<Option<_>>()
        .unwrap_or_default();
    if originals.len() != cfg.sectors.len() {
        return;
    }

    for slot in 0..2 {
        let mut sectors = originals.clone();
        for (span, piece) in pieces(&cfg, records) {
            if span.area == Area::Extra
                && let Some(dst) =
                    sectors[span.sector].get_mut(span.offset..span.offset + piece.len())
            {
                dst.copy_from_slice(piece);
            }
        }

        for checksum in &cfg.checksums {
            let mut covered = Vec::new();
            for span in &checksum.over {
                let bytes = match span.area {
                    Area::Sb1 => sb1.get(span.offset..span.offset + span.length),
                    Area::Storage => storage.get(span.offset..span.offset + span.length),
                    Area::Extra => sectors[span.sector].get(span.offset..span.offset + span.length),
                };
                covered.extend_from_slice(bytes.unwrap_or_default());
            }
            let low = fold_checksum(&covered);
            let value = (u32::from(!low) << 16) | u32::from(low);
            let at = checksum.at;
            if at.area == Area::Extra {
                set_u32_le(&mut sectors[at.sector], at.offset, value);
            }
        }

        for ((data, at), size) in sectors
            .iter()
            .zip(sector_offsets(&cfg, slot))
            .zip(&cfg.sector_checksum_sizes)
        {
            file[at..at + data.len()].copy_from_slice(data);
            let checksum = fold_checksum(&data[..(*size).min(data.len())]);
            set_u16_le(file, at + 0xFF6, checksum);
        }
    }
}

/// Each record span paired with the part of `records` that belongs in it.
fn pieces<'a>(
    cfg: &'a ExtraBoxes,
    records: &'a [u8],
) -> impl Iterator<Item = (&'a crate::game::Span, &'a [u8])> {
    let mut at = 0;
    cfg.boxes
        .iter()
        .flat_map(|b| b.records.iter())
        .filter_map(move |span| {
            let piece = records.get(at..at + span.length)?;
            at += span.length;
            Some((span, piece))
        })
}
