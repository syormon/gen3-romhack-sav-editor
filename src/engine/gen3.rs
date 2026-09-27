//! The vanilla Generation III Pokémon record: four 12-byte substructures,
//! permuted by the personality value and XOR-encrypted.
//!
//! A record is 80 bytes boxed, 100 in the party:
//!
//! ```text
//!   0x00 personality (u32)        0x1C checksum (u16)
//!   0x04 OT id (u32)              0x20 48 bytes: 4 x 12-byte substructures
//!   0x08 nickname (10)            0x50 party only: status, level, battle stats
//!   0x12 language   0x13 flags
//!   0x14 OT name (7)  0x1B markings
//! ```
//!
//! The 48-byte block is XOR-encrypted with `personality ^ ot_id` (as u32
//! words), and the four substructures — Growth, Attacks, EVs, Misc — appear in
//! the order given by `personality % 24`. The checksum is the sum of the
//! decrypted block as 24 little-endian u16s, which is also how an occupied slot
//! is told from noise.
//!
//! Field positions inside the substructures are fixed by the format, so unlike
//! the `plain` encoding they are not configurable; a pack selects this with
//! `"record_encoding": "gen3_shuffled"`.
//!
//! `"record_encoding": "gen3_plain"` uses the same field positions with the
//! obfuscation switched off: substructures in G/A/E/M order, no XOR, and the
//! checksum neither verified nor written. CFRU-based hacks (Unbound) store
//! records that way — they leave the checksum field at zero, so validating it
//! would reject every Pokémon in the save.

use super::bytes::*;
use super::save_parser::Pokemon;

pub const BOX_SIZE: usize = 80;
pub const PARTY_SIZE: usize = 100;
const SUBSTRUCTURES: usize = 0x20;
const BLOCK: usize = 48;
const CHECKSUM: usize = 0x1C;

const NICKNAME: usize = 0x08;
const NICKNAME_LEN: usize = 10;
const LANGUAGE: usize = 0x12;
const FLAGS: usize = 0x13;
const OT_NAME: usize = 0x14;
const OT_NAME_LEN: usize = 7;

// Party tail.
const STATUS: usize = 0x50;
const LEVEL: usize = 0x54;
/// Which of the player's mail slots the Pokémon's held mail is in.
pub const MAIL: usize = 0x55;
const HP: usize = 0x56;
const MAX_HP: usize = 0x58;
const ATTACK: usize = 0x5A;
const DEFENSE: usize = 0x5C;
const SPEED: usize = 0x5E;
const SP_ATTACK: usize = 0x60;
const SP_DEFENSE: usize = 0x62;

/// Substructure order per `personality % 24`: G(rowth), A(ttacks), E(Vs),
/// M(isc).
const ORDER: [[u8; 4]; 24] = [
    *b"GAEM", *b"GAME", *b"GEAM", *b"GEMA", *b"GMAE", *b"GMEA", *b"AGEM", *b"AGME", *b"AEGM",
    *b"AEMG", *b"AMGE", *b"AMEG", *b"EGAM", *b"EGMA", *b"EAGM", *b"EAMG", *b"EMGA", *b"EMAG",
    *b"MGAE", *b"MGEA", *b"MAGE", *b"MAEG", *b"MEGA", *b"MEAG",
];

/// Where each substructure sits, in 12-byte slots.
/// Whether the active pack scrambles records. Passed down explicitly rather
/// than read inside the codec, so tests can exercise both forms without
/// swapping the global game pack.
fn obfuscated() -> bool {
    crate::game::current()
        .layout()
        .record_encoding
        .is_obfuscated()
}

fn slot_of(personality: u32, letter: u8, obfuscated: bool) -> usize {
    let order = if obfuscated {
        &ORDER[(personality % 24) as usize]
    } else {
        b"GAEM"
    };
    order.iter().position(|c| *c == letter).unwrap_or(0)
}

fn xor_block(block: &mut [u8], key: u32) {
    for i in (0..BLOCK).step_by(4) {
        let value = u32_le(block, i) ^ key;
        set_u32_le(block, i, value);
    }
}

fn block_checksum(block: &[u8]) -> u16 {
    let mut sum: u16 = 0;
    for i in (0..BLOCK).step_by(2) {
        sum = sum.wrapping_add(u16_le(block, i));
    }
    sum
}

/// The four substructures, decrypted and put back in G/A/E/M order.
struct Subs {
    growth: [u8; 12],
    attacks: [u8; 12],
    evs: [u8; 12],
    misc: [u8; 12],
}

impl Subs {
    fn read(record: &[u8], personality: u32, ot_id: u32, obfuscated: bool) -> (Self, u16) {
        let mut block = [0u8; BLOCK];
        block.copy_from_slice(&record[SUBSTRUCTURES..SUBSTRUCTURES + BLOCK]);
        if obfuscated {
            xor_block(&mut block, personality ^ ot_id);
        }

        let take = |letter: u8| {
            let at = slot_of(personality, letter, obfuscated) * 12;
            let mut out = [0u8; 12];
            out.copy_from_slice(&block[at..at + 12]);
            out
        };
        let subs = Self {
            growth: take(b'G'),
            attacks: take(b'A'),
            evs: take(b'E'),
            misc: take(b'M'),
        };
        (subs, block_checksum(&block))
    }

    fn write(&self, record: &mut [u8], personality: u32, ot_id: u32, obfuscated: bool) {
        let mut block = [0u8; BLOCK];
        for (letter, data) in [
            (b'G', &self.growth),
            (b'A', &self.attacks),
            (b'E', &self.evs),
            (b'M', &self.misc),
        ] {
            let at = slot_of(personality, letter, obfuscated) * 12;
            block[at..at + 12].copy_from_slice(data);
        }
        if obfuscated {
            // The plain form leaves this field alone: CFRU keeps it at zero and
            // never reads it, so writing a value would be inventing data.
            set_u16_le(record, CHECKSUM, block_checksum(&block));
            xor_block(&mut block, personality ^ ot_id);
        }
        record[SUBSTRUCTURES..SUBSTRUCTURES + BLOCK].copy_from_slice(&block);
    }
}

/// Reads a record. Returns `None` for an empty slot or one whose checksum does
/// not match, which is how the games themselves tell a used slot from noise.
pub fn unpack(record: &[u8], is_party: bool) -> Option<Pokemon> {
    unpack_as(record, is_party, obfuscated())
}

/// As `unpack`, with the obfuscation stated rather than taken from the pack.
pub fn unpack_as(record: &[u8], is_party: bool, obfuscated: bool) -> Option<Pokemon> {
    if record.len() < BOX_SIZE {
        return None;
    }
    let personality = u32_le(record, 0);
    let ot_id = u32_le(record, 4);
    if personality == 0 && ot_id == 0 {
        return None;
    }

    let (subs, checksum) = Subs::read(record, personality, ot_id, obfuscated);
    // Only the obfuscated form maintains this; CFRU leaves it zero, and an
    // occupied slot is told from noise by the species id below instead.
    if obfuscated && checksum != u16_le(record, CHECKSUM) {
        return None;
    }

    let g = &subs.growth;
    let a = &subs.attacks;
    let e = &subs.evs;
    let m = &subs.misc;

    let species = u32::from(u16_le(g, 0));
    if species == 0 {
        return None;
    }

    let origins = u16_le(m, 2);
    let iv_word = u32_le(m, 4);
    let pp_bonuses_byte = g[8];

    let mut evs = [0u32; 6];
    for (i, ev) in evs.iter_mut().enumerate() {
        *ev = u32::from(e[i]);
    }

    let flags = record[FLAGS];
    let nickname =
        super::charmap::decode_gba_string(&record[NICKNAME..NICKNAME + NICKNAME_LEN], NICKNAME_LEN);
    let species_name = super::lookup::get_species_name(species);
    let is_default_nick =
        nickname.trim().is_empty() || nickname.trim().eq_ignore_ascii_case(species_name.trim());

    let mut mon = Pokemon {
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
        is_bad_egg: flags & 0x1 != 0,
        has_species: flags & 0x2 != 0,
        // Gen 3 keeps the egg flag in the IV word, not the flag byte.
        is_egg: (iv_word >> 30) & 1 != 0,
        // Shininess is purely a function of the personality value here.
        shiny_modifier: 0,
        species,
        tera_type: 0,
        held_item: u32::from(u16_le(g, 2)),
        pokeball: match crate::game::current().record().gen3_ball {
            crate::game::Gen3Ball::GrowthU16 => u32::from(u16_le(g, 10)),
            crate::game::Gen3Ball::OriginsBits => u32::from((origins >> 11) & 0xF),
        },
        experience: u32_le(g, 4),
        moves: [
            u32::from(u16_le(a, 0)),
            u32::from(u16_le(a, 2)),
            u32::from(u16_le(a, 4)),
            u32::from(u16_le(a, 6)),
        ],
        pps: [
            u32::from(a[8]),
            u32::from(a[9]),
            u32::from(a[10]),
            u32::from(a[11]),
        ],
        evs,
        ivs: [
            iv_word & 0x1F,
            (iv_word >> 5) & 0x1F,
            (iv_word >> 10) & 0x1F,
            (iv_word >> 15) & 0x1F,
            (iv_word >> 20) & 0x1F,
            (iv_word >> 25) & 0x1F,
        ],
        pp_bonuses: [
            u32::from(pp_bonuses_byte & 0x3),
            u32::from((pp_bonuses_byte >> 2) & 0x3),
            u32::from((pp_bonuses_byte >> 4) & 0x3),
            u32::from((pp_bonuses_byte >> 6) & 0x3),
        ],
        pokerus: m[0],
        met_location: m[1],
        met_level: u32::from(origins & 0x7F),
        // Boxed Pokémon carry no level; it comes from experience.
        level: super::experience::level_from_experience(species, u32_le(g, 4)),
        friendship: g[9],
        // One bit here, not two: slot 0 or slot 1.
        ability_num: ((iv_word >> 31) & 1) as u8,
        status: None,
        hp: None,
        max_hp: None,
        attack: None,
        defense: None,
        speed: None,
        sp_attack: None,
        sp_defense: None,
    };

    if is_party && record.len() >= PARTY_SIZE {
        mon.status = Some(u32_le(record, STATUS));
        mon.level = record[LEVEL];
        mon.hp = Some(u16_le(record, HP));
        mon.max_hp = Some(u16_le(record, MAX_HP));
        mon.attack = Some(u16_le(record, ATTACK));
        mon.defense = Some(u16_le(record, DEFENSE));
        mon.speed = Some(u16_le(record, SPEED));
        mon.sp_attack = Some(u16_le(record, SP_ATTACK));
        mon.sp_defense = Some(u16_le(record, SP_DEFENSE));
    }

    Some(mon)
}

/// Writes a record in place, preserving the bytes this editor does not model
/// (contest stats, ribbons, markings, the origin game).
pub fn pack(record: &mut [u8], mon: &Pokemon, is_party: bool) {
    pack_as(record, mon, is_party, obfuscated());
}

/// As `pack`, with the obfuscation stated rather than taken from the pack.
pub fn pack_as(record: &mut [u8], mon: &Pokemon, is_party: bool, obfuscated: bool) {
    let size = if is_party { PARTY_SIZE } else { BOX_SIZE };
    if record.len() < size {
        return;
    }

    // Read the existing substructures under the *old* personality so untouched
    // fields survive, then re-key them to the new one.
    let old_personality = u32_le(record, 0);
    let old_ot_id = u32_le(record, 4);
    let (mut subs, _) = Subs::read(record, old_personality, old_ot_id, obfuscated);

    let personality = nz(mon.personality, 1);
    set_u32_le(record, 0, personality);
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
    record[FLAGS] = record_flags(record[FLAGS], mon.is_bad_egg, mon.is_egg);

    let ot_name = if mon.ot_name.trim().is_empty() {
        crate::game::current().behavior().default_ot_name.clone()
    } else {
        mon.ot_name.clone()
    };
    super::charmap::write_gba_string(&mut record[OT_NAME..OT_NAME + OT_NAME_LEN], &ot_name);

    // Growth
    set_u16_le(&mut subs.growth, 0, mon.species as u16);
    set_u16_le(&mut subs.growth, 2, mon.held_item as u16);
    set_u32_le(&mut subs.growth, 4, mon.experience);
    let mut pp_bonuses = 0u8;
    for i in 0..4 {
        pp_bonuses |= ((mon.pp_bonuses[i] & 0x3) as u8) << (2 * i);
    }
    subs.growth[8] = pp_bonuses;
    // Verbatim: plenty of Pokémon really do have friendship 0 (freshly caught
    // legendaries, for one), and substituting a default here would quietly
    // rewrite them every time the record is saved.
    subs.growth[9] = mon.friendship;

    // Attacks
    for i in 0..4 {
        set_u16_le(&mut subs.attacks, i * 2, mon.moves[i] as u16);
        subs.attacks[8 + i] = (mon.pps[i] & 0xFF) as u8;
    }

    // EVs (the six contest stats that follow are left as they were)
    for i in 0..6 {
        subs.evs[i] = mon.evs[i].min(252) as u8;
    }

    // Misc
    subs.misc[0] = mon.pokerus;
    subs.misc[1] = mon.met_location;
    let origins = u16_le(&subs.misc, 2);
    let origins = (origins & !0x7F) | ((mon.met_level as u16) & 0x7F);
    let origins = match crate::game::current().record().gen3_ball {
        crate::game::Gen3Ball::GrowthU16 => {
            set_u16_le(&mut subs.growth, 10, mon.pokeball as u16);
            origins
        }
        crate::game::Gen3Ball::OriginsBits => {
            (origins & !(0xF << 11)) | (((mon.pokeball as u16) & 0xF) << 11)
        }
    };
    set_u16_le(&mut subs.misc, 2, origins);

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
    set_u32_le(&mut subs.misc, 4, iv_word);

    subs.write(record, personality, mon.ot_id, obfuscated);

    if is_party && record.len() >= PARTY_SIZE {
        set_u32_le(record, STATUS, mon.status.unwrap_or(0));
        record[LEVEL] = nz(mon.level, 5).clamp(1, 100);
        let max_hp = nz(mon.max_hp.unwrap_or(0), 1);
        set_u16_le(record, HP, mon.hp.unwrap_or(max_hp));
        set_u16_le(record, MAX_HP, max_hp);
        set_u16_le(record, ATTACK, nz(mon.attack.unwrap_or(0), 1));
        set_u16_le(record, DEFENSE, nz(mon.defense.unwrap_or(0), 1));
        set_u16_le(record, SPEED, nz(mon.speed.unwrap_or(0), 1));
        set_u16_le(record, SP_ATTACK, nz(mon.sp_attack.unwrap_or(0), 1));
        set_u16_le(record, SP_DEFENSE, nz(mon.sp_defense.unwrap_or(0), 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::tests::{ensure_pack, sample_mon};

    /// Verified against Ehsan516/seaglass-save-editor: a record written here
    /// decodes field-for-field there, and one written there decodes here.
    #[test]
    fn round_trips_through_every_substructure_order() {
        ensure_pack();
        for order in 0..24u32 {
            // A personality with this remainder picks that substructure order.
            let personality = 0x3A17_0000 + order;
            assert_eq!(personality % 24, order % 24);

            let mut mon = sample_mon(25, 50, "Sparky");
            mon.personality = personality;
            mon.tera_type = 0;
            mon.shiny_modifier = 0;
            mon.pokeball = 4;
            mon.ability_num = 1;
            mon.hidden_nature_modifier = 0;

            let mut record = vec![0u8; PARTY_SIZE];
            pack_as(&mut record, &mon, true, true);
            let read = unpack_as(&record, true, true).expect("record decodes");

            assert_eq!(read.species, mon.species, "order {order}");
            assert_eq!(read.experience, mon.experience, "order {order}");
            assert_eq!(read.moves, mon.moves, "order {order}");
            assert_eq!(read.pps, mon.pps, "order {order}");
            assert_eq!(read.evs, mon.evs, "order {order}");
            assert_eq!(read.ivs, mon.ivs, "order {order}");
            assert_eq!(read.pp_bonuses, mon.pp_bonuses, "order {order}");
            assert_eq!(read.held_item, mon.held_item, "order {order}");
            assert_eq!(read.pokeball, mon.pokeball, "order {order}");
            assert_eq!(read.ability_num, mon.ability_num, "order {order}");
            assert_eq!(read.met_level, mon.met_level, "order {order}");
            assert_eq!(read.met_location, mon.met_location, "order {order}");
            assert_eq!(read.pokerus, mon.pokerus, "order {order}");
            assert_eq!(read.friendship, mon.friendship, "order {order}");
            assert_eq!(read.level, mon.level, "order {order}");
            assert_eq!(read.max_hp, mon.max_hp, "order {order}");
            assert_eq!(read.ot_id, mon.ot_id, "order {order}");
        }
    }

    /// The checksum is how the games tell a used slot from noise, so a record
    /// whose block has been disturbed must read as empty rather than as junk.
    #[test]
    fn a_damaged_record_reads_as_empty() {
        ensure_pack();
        let mut mon = sample_mon(25, 50, "Sparky");
        mon.tera_type = 0;

        let mut record = vec![0u8; PARTY_SIZE];
        pack_as(&mut record, &mon, true, true);
        assert!(unpack_as(&record, true, true).is_some());

        record[SUBSTRUCTURES + 5] ^= 0xFF;
        assert!(
            unpack_as(&record, true, true).is_none(),
            "bad checksum must not decode"
        );

        assert!(
            unpack_as(&[0u8; PARTY_SIZE], true, true).is_none(),
            "empty slot"
        );
    }

    /// A boxed record is 80 bytes and carries no battle stats; its level comes
    /// from experience, as elsewhere.
    #[test]
    fn boxed_records_carry_no_battle_stats() {
        ensure_pack();
        let mut mon = sample_mon(25, 50, "Sparky");
        mon.tera_type = 0;
        mon.experience = crate::engine::experience::experience_for_level(25, 37);

        let mut record = vec![0u8; BOX_SIZE];
        pack_as(&mut record, &mon, false, true);
        let read = unpack_as(&record, false, true).expect("record decodes");

        assert_eq!(read.hp, None);
        assert_eq!(read.status, None);
        assert_eq!(read.level, 37, "derived from experience");
    }

    /// Fields this editor does not model must survive a write.
    #[test]
    fn untouched_fields_are_preserved() {
        ensure_pack();
        let mut mon = sample_mon(25, 50, "Sparky");
        mon.tera_type = 0;

        let mut record = vec![0u8; PARTY_SIZE];
        pack_as(&mut record, &mon, true, true);

        // Contest stats live in the EV substructure, past the six EVs.
        let (mut subs, _) = Subs::read(&record, mon.personality, mon.ot_id, true);
        subs.evs[6..12].copy_from_slice(&[10, 20, 30, 40, 50, 60]);
        subs.write(&mut record, mon.personality, mon.ot_id, true);

        let read = unpack_as(&record, true, true).expect("decodes");
        pack_as(&mut record, &read, true, true);

        let (after, _) = Subs::read(&record, mon.personality, mon.ot_id, true);
        assert_eq!(
            &after.evs[6..12],
            &[10, 20, 30, 40, 50, 60],
            "contest stats kept"
        );
    }
}
