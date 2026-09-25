//! The 58-byte boxed Pokémon record used by Complete FireRed Upgrade hacks.
//!
//! CFRU shrinks the boxed record from Gen 3's 80 bytes so more Pokémon fit in
//! the same storage. The header is unchanged, but everything after the species
//! is packed by hand rather than held in four 12-byte substructures — and the
//! four move ids are squeezed into five bytes as 10-bit fields:
//!
//! ```text
//!   0x00 personality (u32)     0x24 PP bonuses      0x2C EVs (6)
//!   0x04 OT id (u32)           0x25 friendship      0x32 unmodelled (4)
//!   0x08 nickname (10)         0x26 ball            0x36 IV word (u32)
//!   0x12 language  0x13 flags  0x27 moves: 4 x 10 bits in 5 bytes
//!   0x14 OT name (7)           0x2C
//!   0x1B markings              = 0x3A (58) total
//!   0x1C species (u16)
//!   0x1E held item (u16)
//!   0x20 experience (u32)
//! ```
//!
//! Worked out from a real Unbound save rather than from documentation: the
//! 58-byte stride is the spacing between occurrences of the trainer's OT id,
//! and the move offset is the only one of the 23 candidates whose four decoded
//! ids are moves the species can actually learn (91.8% against learnsets, where
//! every other offset scores under 17%).
//!
//! There is no room for current PP, so the games recompute it; this module
//! reports full PP for each move. Bytes 0x32..0x35 carry met data whose
//! meanings are not yet pinned down, so `pack` leaves them exactly as it found
//! them.

use super::bytes::*;
use super::save_parser::Pokemon;

pub const BOX_SIZE: usize = 58;

const NICKNAME: usize = 0x08;
const NICKNAME_LEN: usize = 10;
const LANGUAGE: usize = 0x12;
const FLAGS: usize = 0x13;
const OT_NAME: usize = 0x14;
const OT_NAME_LEN: usize = 7;

const SPECIES: usize = 0x1C;
const HELD_ITEM: usize = 0x1E;
const EXPERIENCE: usize = 0x20;
const PP_BONUSES: usize = 0x24;
const FRIENDSHIP: usize = 0x25;
const BALL: usize = 0x26;
const MOVES: usize = 0x27;
const MOVES_LEN: usize = 5;
const EVS: usize = 0x2C;
const IVS: usize = 0x36;

/// Four 10-bit move ids, little-endian, low field first.
fn read_moves(record: &[u8]) -> [u32; 4] {
    let mut packed: u64 = 0;
    for i in 0..MOVES_LEN {
        packed |= u64::from(record[MOVES + i]) << (8 * i);
    }
    let mut moves = [0u32; 4];
    for (i, m) in moves.iter_mut().enumerate() {
        *m = ((packed >> (10 * i)) & 0x3FF) as u32;
    }
    moves
}

fn write_moves(record: &mut [u8], moves: &[u32; 4]) {
    let mut packed: u64 = 0;
    for (i, m) in moves.iter().enumerate() {
        packed |= u64::from(u64::from(*m) & 0x3FF) << (10 * i);
    }
    for i in 0..MOVES_LEN {
        record[MOVES + i] = ((packed >> (8 * i)) & 0xFF) as u8;
    }
}

pub fn unpack(record: &[u8]) -> Option<Pokemon> {
    if record.len() < BOX_SIZE {
        return None;
    }
    let personality = u32_le(record, 0);
    let ot_id = u32_le(record, 4);
    if personality == 0 && ot_id == 0 {
        return None;
    }

    // There is no checksum here to tell a used slot from noise, so the games'
    // own occupancy flag has to do it. Without this, leftover bytes past the
    // last real box decode into plausible-looking Pokémon.
    if record[FLAGS] & 0x02 == 0 {
        return None;
    }

    let species = u32::from(u16_le(record, SPECIES));
    if species == 0 {
        return None;
    }

    let iv_word = u32_le(record, IVS);
    let mut ivs = [0u32; 6];
    for (i, iv) in ivs.iter_mut().enumerate() {
        *iv = (iv_word >> (5 * i)) & 0x1F;
    }
    let mut evs = [0u32; 6];
    for (i, ev) in evs.iter_mut().enumerate() {
        *ev = u32::from(record[EVS + i]);
    }

    let moves = read_moves(record);
    let pp_bonuses_byte = record[PP_BONUSES];
    let mut pp_bonuses = [0u32; 4];
    for (i, b) in pp_bonuses.iter_mut().enumerate() {
        *b = u32::from((pp_bonuses_byte >> (2 * i)) & 0x3);
    }
    // Current PP is not stored, so the games recompute it; report it full.
    let mut pps = [0u32; 4];
    for (i, pp) in pps.iter_mut().enumerate() {
        let base = super::lookup::get_move_base_pp(moves[i]);
        *pp = base + base / 5 * pp_bonuses[i];
    }

    let experience = u32_le(record, EXPERIENCE);
    let nickname =
        super::charmap::decode_gba_string(&record[NICKNAME..NICKNAME + NICKNAME_LEN], NICKNAME_LEN);
    let species_name = super::lookup::get_species_name(species);
    let is_default_nick =
        nickname.trim().is_empty() || nickname.trim().eq_ignore_ascii_case(species_name.trim());

    Some(Pokemon {
        personality,
        ot_id,
        nickname: if is_default_nick {
            String::new()
        } else {
            nickname
        },
        ot_name: super::charmap::decode_gba_string(
            &record[OT_NAME..OT_NAME + OT_NAME_LEN],
            OT_NAME_LEN,
        ),
        language: record[LANGUAGE] & 0x7,
        hidden_nature_modifier: 0,
        is_bad_egg: record[FLAGS] & 0x1 != 0,
        has_species: record[FLAGS] & 0x2 != 0,
        is_egg: (iv_word >> 30) & 1 != 0,
        shiny_modifier: 0,
        species,
        tera_type: 0,
        held_item: u32::from(u16_le(record, HELD_ITEM)),
        pokeball: u32::from(record[BALL]),
        experience,
        moves,
        pps,
        evs,
        ivs,
        pp_bonuses,
        pokerus: 0,
        met_location: 0,
        met_level: 0,
        level: super::experience::level_from_experience(species, experience),
        friendship: record[FRIENDSHIP],
        ability_num: ((iv_word >> 31) & 1) as u8,
        status: None,
        hp: None,
        max_hp: None,
        attack: None,
        defense: None,
        speed: None,
        sp_attack: None,
        sp_defense: None,
    })
}

/// Writes a Pokémon over an existing record, leaving the bytes this module does
/// not model exactly as they were.
pub fn pack(record: &mut [u8], mon: &Pokemon) {
    if record.len() < BOX_SIZE {
        return;
    }
    set_u32_le(record, 0, nz(mon.personality, 1));
    set_u32_le(record, 4, mon.ot_id);

    let game_nick = {
        let nick = mon.nickname.trim();
        if nick.is_empty() {
            super::lookup::get_species_name(mon.species)
        } else {
            nick.to_string()
        }
    };
    super::charmap::write_gba_string(&mut record[NICKNAME..NICKNAME + NICKNAME_LEN], &game_nick);

    record[LANGUAGE] = mon.language & 0x7;
    record[FLAGS] = 0x02; // has species

    let ot_name = if mon.ot_name.trim().is_empty() {
        crate::game::current().behavior().default_ot_name.clone()
    } else {
        mon.ot_name.clone()
    };
    super::charmap::write_gba_string(&mut record[OT_NAME..OT_NAME + OT_NAME_LEN], &ot_name);

    set_u16_le(record, SPECIES, mon.species as u16);
    set_u16_le(record, HELD_ITEM, mon.held_item as u16);
    set_u32_le(record, EXPERIENCE, mon.experience);

    let mut pp_bonuses = 0u8;
    for i in 0..4 {
        pp_bonuses |= ((mon.pp_bonuses[i] & 0x3) as u8) << (2 * i);
    }
    record[PP_BONUSES] = pp_bonuses;
    record[FRIENDSHIP] = mon.friendship;
    record[BALL] = mon.pokeball as u8;
    write_moves(record, &mon.moves);

    for i in 0..6 {
        record[EVS + i] = mon.evs[i].min(252) as u8;
    }

    let mut iv_word = 0u32;
    for i in 0..6 {
        iv_word |= (mon.ivs[i] & 0x1F) << (5 * i);
    }
    if mon.is_egg {
        iv_word |= 1 << 30;
    }
    if mon.ability_num & 1 != 0 {
        iv_word |= 1 << 31;
    }
    set_u32_le(record, IVS, iv_word);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_survive_the_ten_bit_packing() {
        let mut record = vec![0u8; BOX_SIZE];
        for moves in [
            [0, 0, 0, 0],
            [1, 2, 3, 4],
            [922, 921, 920, 919],
            [1023, 0, 1023, 0],
        ] {
            write_moves(&mut record, &moves);
            assert_eq!(read_moves(&record), moves, "packing {moves:?}");
        }
    }

    /// The five move bytes must not spill into the fields on either side.
    #[test]
    fn packing_moves_stays_inside_its_five_bytes() {
        let mut record = vec![0xAAu8; BOX_SIZE];
        write_moves(&mut record, &[1023, 1023, 1023, 1023]);
        assert_eq!(record[MOVES - 1], 0xAA, "byte before the move block");
        assert_eq!(record[MOVES + MOVES_LEN], 0xAA, "byte after the move block");
    }

    #[test]
    fn a_record_survives_a_round_trip() {
        crate::engine::tests::ensure_pack();
        let mut mon = crate::engine::tests::sample_mon(25, 50, "Sparky");
        mon.moves = [33, 45, 85, 148];
        mon.pp_bonuses = [3, 0, 1, 2];
        mon.evs = [252, 128, 0, 4, 60, 66];
        mon.ivs = [31, 0, 17, 5, 31, 28];
        mon.pokeball = 11;
        mon.friendship = 30;
        mon.held_item = 55;
        mon.ability_num = 1;

        let mut record = vec![0u8; BOX_SIZE];
        pack(&mut record, &mon);
        let read = unpack(&record).expect("decodes");

        assert_eq!(read.species, mon.species);
        assert_eq!(read.experience, mon.experience);
        assert_eq!(read.moves, mon.moves);
        assert_eq!(read.pp_bonuses, mon.pp_bonuses);
        assert_eq!(read.evs, mon.evs);
        assert_eq!(read.ivs, mon.ivs);
        assert_eq!(read.pokeball, mon.pokeball);
        assert_eq!(read.friendship, mon.friendship);
        assert_eq!(read.held_item, mon.held_item);
        assert_eq!(read.ability_num, mon.ability_num);
        assert_eq!(read.ot_name, mon.ot_name);
        assert_eq!(read.personality, mon.personality);
        assert_eq!(read.ot_id, mon.ot_id);
    }

    /// There is no checksum to tell a used slot from noise, so the occupancy
    /// flag has to. Without it, the bytes past the last real box decode into
    /// convincing Pokémon — 28 of them, in the save this was written against.
    #[test]
    fn a_slot_without_the_species_flag_reads_as_empty() {
        crate::engine::tests::ensure_pack();
        let mon = crate::engine::tests::sample_mon(25, 50, "Sparky");
        let mut record = vec![0u8; BOX_SIZE];
        pack(&mut record, &mon);
        assert!(unpack(&record).is_some());

        record[FLAGS] &= !0x02;
        assert!(unpack(&record).is_none(), "flag clear means empty slot");
    }

    /// Writing must not disturb the met bytes, whose meanings are not known.
    #[test]
    fn unmodelled_bytes_are_left_alone() {
        crate::engine::tests::ensure_pack();
        let mon = crate::engine::tests::sample_mon(25, 50, "Sparky");
        let mut record = vec![0u8; BOX_SIZE];
        pack(&mut record, &mon);
        record[0x32..0x36].copy_from_slice(&[0x11, 0x22, 0x33, 0x44]);

        let read = unpack(&record).expect("decodes");
        pack(&mut record, &read);
        assert_eq!(&record[0x32..0x36], &[0x11, 0x22, 0x33, 0x44]);
    }
}
