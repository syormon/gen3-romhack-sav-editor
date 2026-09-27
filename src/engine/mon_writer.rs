//! Port of `src/engine/MonWriter.ts`.

use super::bytes::*;
use super::charmap::write_gba_string;
use super::layout::*;
use super::lookup::{
    get_move_base_pp, get_primary_tera_type_id, get_species_name, get_suggested_encounter_level,
    get_suggested_moves_for_species,
};
use super::save_parser::{Pokemon, TrainerInfo};

/// Small xorshift PRNG, standing in for `Math.random()`.
///
/// Personality values and IVs only need to be unpredictable enough that two
/// created Pokémon differ; this is seeded from the clock.
pub struct Rng(u32);

impl Rng {
    pub fn from_clock() -> Self {
        let nanos = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() ^ d.as_secs() as u32)
            .unwrap_or(0x1234_5678);
        Self(nz(nanos, 0x1234_5678))
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    pub fn below(&mut self, bound: u32) -> u32 {
        self.next_u32() % bound
    }
}

pub fn create_default_pokemon(species_id: u32, trainer: &TrainerInfo, rng: &mut Rng) -> Pokemon {
    let pack = crate::game::current();
    let behavior = pack.behavior();
    let tid = u32::from(trainer.tid);
    let sid = u32::from(trainer.sid);
    let personality = rng.next_u32();
    let level = get_suggested_encounter_level(species_id);
    let moves = get_suggested_moves_for_species(species_id);
    let pps = [
        get_move_base_pp(moves[0]),
        get_move_base_pp(moves[1]),
        get_move_base_pp(moves[2]),
        get_move_base_pp(moves[3]),
    ];
    let ot_name = trainer.name.clone();
    // Experience must sit on the species' own growth curve, or the level read
    // back from it will not be the level we asked for.
    let exp = super::experience::experience_for_level(species_id, level);

    let mut mon = Pokemon {
        personality,
        ot_id: (sid << 16) | tid,
        nickname: String::new(),
        ot_name,
        language: 2,
        hidden_nature_modifier: 0,
        is_bad_egg: false,
        has_species: true,
        is_egg: false,
        shiny_modifier: 0,
        species: species_id,
        tera_type: get_primary_tera_type_id(species_id),
        held_item: 0,
        pokeball: 1,
        experience: exp,
        moves,
        pps,
        ivs: [
            rng.below(32),
            rng.below(32),
            rng.below(32),
            rng.below(32),
            rng.below(32),
            rng.below(32),
        ],
        evs: [0; 6],
        pp_bonuses: [0; 4],
        pokerus: 0,
        met_location: behavior.met_location,
        met_level: u32::from(level),
        level,
        friendship: 70,
        ability_num: 0,
        status: Some(0),
        hp: Some(40),
        max_hp: Some(40),
        attack: Some(25),
        defense: Some(25),
        speed: Some(25),
        sp_attack: Some(25),
        sp_defense: Some(25),
    };
    // The placeholders above stand only for a pack without base stats.
    super::stats::refresh(&mut mon);
    mon
}

pub fn clear_pokemon_slot(target: &mut [u8], offset: usize, is_party: bool) {
    let size = if is_party { mon_size() } else { box_mon_size() };
    let end = (offset + size).min(target.len());
    target[offset..end].fill(0);
}

pub fn pack_and_write_pokemon(target: &mut [u8], offset: usize, mon: &Pokemon, is_party: bool) {
    let size = if is_party { mon_size() } else { box_mon_size() };
    if offset + size > target.len() {
        return;
    }
    let b = &mut target[offset..offset + size];
    let pack = crate::game::current();
    // A party record whose battle half is still zero has just been made, by
    // creation or by joining the party from a box.
    let new_party_record = is_party && b[box_mon_size().min(size)..].iter().all(|x| *x == 0);
    if !is_party && boxes_are_cfru_compact() {
        super::cfru::pack(b, mon);
        return;
    }
    if pack.layout().record_encoding.is_gen3() {
        super::gen3::pack(b, mon, is_party);
        if new_party_record {
            b[super::gen3::MAIL] = NO_MAIL;
        }
        return;
    }
    let r = pack.record();
    let new_record = b[..8].iter().all(|x| *x == 0);

    // Every field below is written into just the bits the editor models. The
    // web original rebuilt whole words, which reset whatever shared them: the
    // origin game beside the met level, a flag beside the shiny bit, a
    // Pokémon's mail. A record written back unchanged must be byte-identical.

    set_u32_le(b, r.personality, nz(mon.personality, 1));
    set_u32_le(b, r.ot_id, mon.ot_id);

    // A Pokémon with no nickname still stores a name: the species name, spelled
    // exactly as the game spells it. The game decides whether a Pokémon is
    // nicknamed by comparing that stored name against the species name, and
    // renames it on evolution only when they match. So the bytes have to match
    // the game's own string — the web original wrote it UPPERCASED and clipped
    // to 10 characters, which never matched ("Skarmory" vs "SKARMORY"), leaving
    // every Pokémon the editor touched looking permanently nicknamed.
    let game_nick = {
        let nick = mon.nickname.trim();
        if nick.is_empty() {
            get_species_name(mon.species)
        } else {
            nick.to_string()
        }
    };
    let nick_len = r.nickname_length;
    write_gba_string(&mut b[r.nickname..r.nickname + nick_len], &game_nick);

    b[r.language] = (mon.language & 0x7) | ((mon.hidden_nature_modifier & 0x1F) << 3);
    b[r.flags] = record_flags(b[r.flags], mon.is_bad_egg, mon.is_egg);

    let ot_name = if mon.ot_name.trim().is_empty() {
        pack.behavior().default_ot_name.clone()
    } else {
        mon.ot_name.clone()
    };
    let ot_len = r.ot_name_length;
    write_gba_string(&mut b[r.ot_name..r.ot_name + ot_len], &ot_name);
    set_u16_bits(
        b,
        r.shiny_word,
        1 << 14,
        u16::from(mon.shiny_modifier) << 14,
    );

    let sec = &mut b[r.substructure..];
    let tera = nz(mon.tera_type, 1);
    set_u16_le(
        sec,
        r.species,
        ((mon.species & 0x7FF) as u16) | (((tera & 0x1F) as u16) << 11),
    );

    let ball_id = nz(mon.pokeball, 1).clamp(1, 63);
    set_u16_le(
        sec,
        r.held_item,
        ((mon.held_item & 0x3FF) as u16) | (((ball_id & 0x3F) as u16) << 10),
    );
    set_u32_bits(sec, r.experience, 0xFF_FFFF, mon.experience);

    let mut pp_bonuses = 0u8;
    for i in 0..4 {
        pp_bonuses |= ((mon.pp_bonuses[i] & 0x3) as u8) << (2 * i);
    }
    sec[r.pp_bonuses] = pp_bonuses;
    sec[r.friendship] = mon.friendship; // see the note in gen3::pack

    for i in 0..4 {
        set_u16_bits(sec, r.moves + i * 2, 0x7FF, mon.moves[i] as u16);
    }
    // The ability slot lives in the top bits of move 4's word.
    let ability_num = u16::from(mon.ability_num.min(2));
    set_u16_bits(sec, r.moves + 6, 0x3000, ability_num << 12);

    for i in 0..4 {
        sec[r.pps + i] = (sec[r.pps + i] & 0x80) | (mon.pps[i] & 0x7F) as u8;
    }

    for i in 0..6 {
        sec[r.evs + i] = mon.evs[i].min(252) as u8;
    }

    sec[r.pokerus] = mon.pokerus;
    sec[r.met_location] = mon.met_location;
    // The rest of the met word is the origin game and the OT's gender. A new
    // record says Emerald (3), which SoulGold's own Pokémon carry; the web
    // original stamped FireRed (4) over every record it wrote.
    if new_record {
        set_u16_le(sec, r.met_level, 3 << 7);
    }
    set_u16_bits(sec, r.met_level, 0x7F, mon.met_level as u16);

    let iv_word = (mon.ivs[0] & 0x1F)
        | ((mon.ivs[1] & 0x1F) << 5)
        | ((mon.ivs[2] & 0x1F) << 10)
        | ((mon.ivs[3] & 0x1F) << 15)
        | ((mon.ivs[4] & 0x1F) << 20)
        | ((mon.ivs[5] & 0x1F) << 25);
    // The top two bits are not IVs. Keep them as they were: the web original
    // cleared bit 30, which hatches any egg the editor writes back.
    let kept = u32_le(sec, r.ivs) & 0xC000_0000;
    set_u32_le(sec, r.ivs, iv_word | kept);

    if is_party && b.len() >= mon_size() {
        set_u32_le(b, r.status, mon.status.unwrap_or(0));
        b[r.level] = nz(mon.level, 5).clamp(1, 100);
        if new_party_record {
            b[r.level + 1] = NO_MAIL;
        }

        let max_hp = nz(mon.max_hp.unwrap_or(0), 40);
        set_u16_le(b, r.hp, mon.hp.unwrap_or(max_hp));
        set_u16_le(b, r.max_hp, max_hp);
        set_u16_le(b, r.attack, nz(mon.attack.unwrap_or(0), 25));
        set_u16_le(b, r.defense, nz(mon.defense.unwrap_or(0), 25));
        set_u16_le(b, r.speed, nz(mon.speed.unwrap_or(0), 25));
        set_u16_le(b, r.sp_attack, nz(mon.sp_attack.unwrap_or(0), 25));
        set_u16_le(b, r.sp_defense, nz(mon.sp_defense.unwrap_or(0), 25));
    }
}
