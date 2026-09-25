//! Port of `src/engine/SaveParser.ts`.

use super::bytes::*;
use super::charmap::{decode_gba_string, encode_gba_string};
use super::layout::*;
use super::lookup::get_species_name;

#[derive(Clone, Debug, PartialEq)]
pub struct Pokemon {
    pub personality: u32,
    pub ot_id: u32,
    pub nickname: String,
    pub ot_name: String,
    pub language: u8,
    pub hidden_nature_modifier: u8,
    pub is_bad_egg: bool,
    pub has_species: bool,
    pub is_egg: bool,
    pub shiny_modifier: u8,
    pub species: u32,
    pub tera_type: u32,
    pub held_item: u32,
    pub pokeball: u32,
    pub experience: u32,
    pub moves: [u32; 4],
    pub pps: [u32; 4],
    pub evs: [u32; 6],
    pub ivs: [u32; 6],
    pub pp_bonuses: [u32; 4],
    pub pokerus: u8,
    pub met_location: u8,
    pub met_level: u32,
    pub level: u8,
    pub friendship: u8,
    pub ability_num: u8,

    /// Only present for party members; box mons carry no battle stats.
    pub status: Option<u32>,
    pub hp: Option<u16>,
    pub max_hp: Option<u16>,
    pub attack: Option<u16>,
    pub defense: Option<u16>,
    pub speed: Option<u16>,
    pub sp_attack: Option<u16>,
    pub sp_defense: Option<u16>,
}

impl Pokemon {
    /// Shiny check from `main.ts`: `(tid ^ sid ^ pid_hi ^ pid_lo) < 8`, flipped
    /// by the stored shiny modifier bit.
    pub fn is_shiny(&self) -> bool {
        if crate::game::current()
            .behavior()
            .never_shiny
            .contains(&self.species)
        {
            return false;
        }
        let p1 = (self.personality >> 16) & 0xFFFF;
        let p2 = self.personality & 0xFFFF;
        let tid = self.ot_id & 0xFFFF;
        let sid = (self.ot_id >> 16) & 0xFFFF;
        let threshold = crate::game::current().behavior().shiny_threshold;
        ((tid ^ sid ^ p1 ^ p2) < threshold) != (self.shiny_modifier & 1 != 0)
    }

    pub fn nature(&self) -> u32 {
        self.personality % 25
    }

    pub fn display_name(&self) -> String {
        let nick = self.nickname.trim();
        if nick.is_empty() {
            get_species_name(self.species)
        } else {
            nick.to_string()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BagItem {
    pub id: u32,
    pub quantity: u32,
}

#[derive(Clone, Debug)]
pub struct TrainerInfo {
    pub name: String,
    pub gender_code: u8,
    pub tid: u16,
    pub sid: u16,
    pub money: u32,
    pub coins: u32,
}

impl TrainerInfo {
    pub fn gender_label(&self) -> &'static str {
        if self.gender_code == 1 { "Girl" } else { "Boy" }
    }
}

pub struct GameSave {
    pub buffer: Vec<u8>,
    pub sb1: Vec<u8>,
    pub sb2: Vec<u8>,
    pub storage: Vec<u8>,
    pub active_slot: usize,
    pub save_counter: u32,
    pub encryption_key: u32,
}

impl GameSave {
    /// The web version simply constructs over whatever `ArrayBuffer` it is
    /// handed; here a short file is rejected up front, since the exporter has to
    /// write the slot counters in sectors 28-31.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, String> {
        let pack = crate::game::current();
        if bytes.len() < min_save_size() {
            return Err(format!(
                "That file is {} bytes. A {} save is {} bytes.",
                bytes.len(),
                pack.manifest.name,
                min_save_size()
            ));
        }

        let mut save = Self {
            buffer: bytes,
            sb1: vec![0; sb1_size()],
            sb2: vec![0; sb2_size()],
            storage: vec![0; storage_size()],
            active_slot: 0,
            save_counter: 0,
            encryption_key: 0,
        };
        save.parse_active_slot();

        if !save.sb2.iter().any(|&b| b != 0) {
            return Err(format!(
                "No valid save sectors found. Is this a {} .sav file?",
                pack.manifest.title()
            ));
        }
        Ok(save)
    }

    fn parse_active_slot(&mut self) {
        let (c0, c1) = (slot_counter(&self.buffer, 0), slot_counter(&self.buffer, 1));
        self.active_slot = match (c0, c1) {
            (Some(a), Some(b)) => usize::from(b > a),
            (None, Some(_)) => 1,
            _ => 0,
        };
        self.save_counter = c0.unwrap_or(0).max(c1.unwrap_or(0));

        let base_offset = sectors_per_slot() * self.active_slot * sector_size();
        let mut sector_map: Vec<Option<usize>> = vec![None; sectors_per_slot()];

        for i in 0..sectors_per_slot() {
            let s_offset = base_offset + i * sector_size();
            let sec_id = u16_le(&self.buffer, s_offset + 0xFF4) as usize;
            let sig = u32_le(&self.buffer, s_offset + 0xFF8);
            if sig == sector_signature() && sec_id < sectors_per_slot() {
                sector_map[sec_id] = Some(s_offset);
            }
        }

        if let Some(off) = sector_map[0] {
            copy_from(&self.buffer, off, &mut self.sb2, 0, sb2_size());
            // A game with no security key leaves the XOR as a no-op.
            self.encryption_key = encryption_key_offset().map_or(0, |at| u32_le(&self.sb2, at));
        }

        let mut sb1_pos = 0;
        for sid in 1..=4 {
            if let Some(off) = sector_map[sid] {
                let chunk = sector_data_size().min(sb1_size() - sb1_pos);
                copy_from(&self.buffer, off, &mut self.sb1, sb1_pos, chunk);
                sb1_pos += chunk;
            }
        }

        let mut stor_pos = 0;
        for sid in storage_sectors_start()..=storage_sectors_end() {
            if let Some(off) = sector_map[sid] {
                let chunk = sector_data_size().min(storage_size() - stor_pos);
                copy_from(&self.buffer, off, &mut self.storage, stor_pos, chunk);
                stor_pos += chunk;
            }
        }
    }

    pub fn trainer_info(&self) -> TrainerInfo {
        let name_len = player_name_length() + 1;
        let name_bytes = &self.sb2[player_name_offset()..player_name_offset() + name_len];

        let raw_money = u32_le(&self.sb1, money_offset());
        let money = raw_money ^ self.encryption_key;

        let raw_coins = u16_le(&self.sb1, coins_offset());
        let coins = raw_coins ^ (self.encryption_key & 0xFFFF) as u16;

        TrainerInfo {
            name: decode_gba_string(name_bytes, name_len),
            gender_code: self.sb2[player_gender_offset()],
            tid: u16_le(&self.sb2, trainer_id_offset()),
            sid: u16_le(&self.sb2, trainer_id_offset() + 2),
            money: money.min(max_money()),
            coins: u32::from(coins).min(max_coins()),
        }
    }

    pub fn set_trainer_info(&mut self, data: &TrainerInfo) {
        let name_len = player_name_length() + 1;
        let name: String = data.name.chars().take(player_name_length()).collect();
        let encoded = encode_gba_string(&name, name_len);
        self.sb2[player_name_offset()..player_name_offset() + name_len].copy_from_slice(&encoded);
        self.sb2[player_gender_offset()] = data.gender_code & 0x1;

        set_u16_le(&mut self.sb2, trainer_id_offset(), data.tid);
        set_u16_le(&mut self.sb2, trainer_id_offset() + 2, data.sid);

        let clamped_money = data.money.min(max_money());
        set_u32_le(
            &mut self.sb1,
            money_offset(),
            clamped_money ^ self.encryption_key,
        );

        let clamped_coins = data.coins.min(max_coins()) as u16;
        let store_coins = clamped_coins ^ (self.encryption_key & 0xFFFF) as u16;
        set_u16_le(&mut self.sb1, coins_offset(), store_coins);
    }

    /// Where a pocket's slots start, and in which buffer.
    ///
    /// `None` when the pocket does not fit the buffer it names, so a bad
    /// manifest reads as an empty pocket rather than panicking.
    fn pocket_span(&self, pocket: usize) -> Option<(crate::game::PocketRegion, usize, usize)> {
        let (offset, count) = (pocket_offset(pocket), pocket_capacity(pocket));
        let region = pocket_region(pocket);
        let base = match region {
            crate::game::PocketRegion::SaveBlock1 => bag_offset() + offset,
            _ => offset,
        };
        let len = match region {
            crate::game::PocketRegion::SaveBlock1 => self.sb1.len(),
            crate::game::PocketRegion::Storage => self.storage.len(),
            crate::game::PocketRegion::File => self.buffer.len(),
        };
        (base + count * 4 <= len).then_some((region, base, count))
    }

    fn pocket_buffer(&self, region: crate::game::PocketRegion) -> &[u8] {
        match region {
            crate::game::PocketRegion::SaveBlock1 => &self.sb1,
            crate::game::PocketRegion::Storage => &self.storage,
            crate::game::PocketRegion::File => &self.buffer,
        }
    }

    fn pocket_buffer_mut(&mut self, region: crate::game::PocketRegion) -> &mut [u8] {
        match region {
            crate::game::PocketRegion::SaveBlock1 => &mut self.sb1,
            crate::game::PocketRegion::Storage => &mut self.storage,
            crate::game::PocketRegion::File => &mut self.buffer,
        }
    }

    pub fn pocket_items(&self, pocket: usize) -> Vec<BagItem> {
        let Some((region, base, count)) = self.pocket_span(pocket) else {
            return Vec::new();
        };
        let enc_hword = (self.encryption_key & 0xFFFF) as u16;
        let buf = self.pocket_buffer(region);

        (0..count)
            .filter_map(|i| {
                let id = u16_le(buf, base + i * 4);
                if id == 0 {
                    return None;
                }
                let raw_qty = u16_le(buf, base + i * 4 + 2);
                Some(BagItem {
                    id: u32::from(id),
                    quantity: u32::from(raw_qty ^ enc_hword),
                })
            })
            .collect()
    }

    pub fn set_pocket_items(&mut self, pocket: usize, items: &[BagItem]) {
        let Some((region, base, count)) = self.pocket_span(pocket) else {
            return;
        };
        let enc_hword = (self.encryption_key & 0xFFFF) as u16;
        let buf = self.pocket_buffer_mut(region);

        for i in 0..count {
            match items.get(i) {
                Some(item) => {
                    let enc_qty = (item.quantity as u16) ^ enc_hword;
                    set_u16_le(buf, base + i * 4, item.id as u16);
                    set_u16_le(buf, base + i * 4 + 2, enc_qty);
                }
                None => {
                    set_u16_le(buf, base + i * 4, 0);
                    set_u16_le(buf, base + i * 4 + 2, 0);
                }
            }
        }
    }

    /// Party members, packed to the front exactly like `getParty()`.
    pub fn party(&self) -> Vec<Pokemon> {
        let count = (u32_le(&self.sb1, party_count_offset()) as usize).min(party_size());
        (0..count)
            .filter_map(|i| {
                let offset = party_offset() + i * mon_size();
                unpack_mon(&self.sb1[offset..offset + mon_size()], true)
            })
            .filter(|mon| mon.species > 0)
            .collect()
    }

    pub fn box_pokemon(&self, box_index: usize, slot_index: usize) -> Option<Pokemon> {
        if box_index >= total_boxes() || slot_index >= box_capacity() {
            return None;
        }
        let linear_index = box_index * box_capacity() + slot_index;
        let offset = storage_boxes_offset() + linear_index * box_mon_size();
        unpack_mon(&self.storage[offset..offset + box_mon_size()], false)
    }

    pub fn box_names(&self) -> Vec<String> {
        (0..total_boxes())
            .map(|b| {
                // A game whose box names this editor has not located declares a
                // width of 0; the UI then falls back to "Box N".
                let width = box_name_length();
                let off = storage_box_names_offset() + b * width;
                if width == 0 || off + width > self.storage.len() {
                    return String::new();
                }
                decode_gba_string(&self.storage[off..off + width], width)
            })
            .collect()
    }

    pub fn set_box_name(&mut self, box_index: usize, name: &str) {
        let width = box_name_length();
        let off = storage_box_names_offset() + box_index * width;
        // `width - 1` would underflow for a game with no box names, and the
        // slice would be out of range.
        if box_index >= total_boxes() || width == 0 || off + width > self.storage.len() {
            return;
        }
        let trimmed: String = name.chars().take(width - 1).collect();
        let encoded = encode_gba_string(&trimmed, width);
        self.storage[off..off + width].copy_from_slice(&encoded);
    }

    pub fn box_wallpapers(&self) -> Vec<u8> {
        (0..total_boxes())
            .map(|b| {
                self.storage
                    .get(storage_box_wallpapers_offset() + b)
                    .copied()
                    .unwrap_or(0)
            })
            .collect()
    }

    pub fn set_box_wallpaper(&mut self, box_index: usize, wallpaper_idx: u8) {
        if box_index < total_boxes() {
            if let Some(slot) = self
                .storage
                .get_mut(storage_box_wallpapers_offset() + box_index)
            {
                *slot = wallpaper_idx;
            }
        }
    }

    pub fn current_box(&self) -> usize {
        self.storage
            .get(storage_current_box())
            .copied()
            .unwrap_or(0) as usize
    }

    pub fn set_current_box(&mut self, box_idx: usize) {
        if box_idx < total_boxes() {
            self.storage[storage_current_box()] = box_idx as u8;
        }
    }

    /// Rewrites the party count from the slots that actually hold data, as
    /// `updatePartyCount()` does after every move or release.
    pub fn refresh_party_count(&mut self) {
        let mut count = 0u32;
        for i in 0..party_size() {
            let off = party_offset() + i * mon_size();
            if u32_le(&self.sb1, off) != 0 || u32_le(&self.sb1, off + 4) != 0 {
                count += 1;
            }
        }
        set_u32_le(&mut self.sb1, party_count_offset(), count);
    }
}

fn copy_from(src: &[u8], src_off: usize, dst: &mut [u8], dst_off: usize, len: usize) {
    let available = src.len().saturating_sub(src_off).min(len);
    if available > 0 {
        dst[dst_off..dst_off + available].copy_from_slice(&src[src_off..src_off + available]);
    }
}

/// `unpackMon` from the TypeScript source.
/// How new a slot is, or `None` if the game has never written it.
///
/// The counter alone is not enough: an unwritten slot is erased flash, every
/// byte 0xFF, so it reads as counter 0xFFFFFFFF and beats any real save. A slot
/// only counts if it holds at least one correctly signed sector.
pub fn slot_counter(buffer: &[u8], slot: usize) -> Option<u32> {
    let base = sectors_per_slot() * slot * sector_size();
    let mut newest: Option<u32> = None;
    for i in 0..sectors_per_slot() {
        let at = base + i * sector_size();
        if at + sector_size() > buffer.len() {
            break;
        }
        if u32_le(buffer, at + 0xFF8) == sector_signature() {
            let counter = u32_le(buffer, at + 0xFFC);
            newest = Some(newest.map_or(counter, |n| n.max(counter)));
        }
    }
    newest
}

pub fn unpack_mon(b: &[u8], is_party: bool) -> Option<Pokemon> {
    if b.len() < box_mon_size() {
        return None;
    }
    let pack = crate::game::current();
    if !is_party && boxes_are_cfru_compact() {
        return super::cfru::unpack(b);
    }
    if pack.layout().record_encoding.is_gen3() {
        return super::gen3::unpack(b, is_party);
    }
    let r = pack.record();

    let personality = u32_le(b, r.personality);
    let ot_id = u32_le(b, r.ot_id);
    if personality == 0 && ot_id == 0 {
        return None;
    }

    let nick_len = r.nickname_length;
    let raw_nick = decode_gba_string(&b[r.nickname..r.nickname + nick_len], nick_len);
    let b20 = b[r.language];
    let language = b20 & 0x7;
    let hidden_nature_modifier = (b20 >> 3) & 0x1F;

    let b21 = b[r.flags];
    let is_bad_egg = b21 & 0x1 != 0;
    let has_species = b21 & 0x2 != 0;
    let is_egg = b21 & 0x4 != 0;

    let ot_len = r.ot_name_length;
    let ot_name = decode_gba_string(&b[r.ot_name..r.ot_name + ot_len], ot_len);
    let w30 = u16_le(b, r.shiny_word);
    let shiny_modifier = ((w30 >> 14) & 0x1) as u8;

    // Substructure, at the offset the manifest gives.
    let sec = &b[r.substructure..];
    let w0 = u16_le(sec, r.species);
    let species = u32::from(w0 & 0x7FF);
    let tera_type = u32::from((w0 >> 11) & 0x1F);

    let w1 = u16_le(sec, r.held_item);
    let held_item = u32::from(w1 & 0x3FF);
    let pokeball = u32::from((w1 >> 10) & 0x3F);

    let experience = u32_le(sec, r.experience) & 0xFF_FFFF;

    let pp_bonuses_byte = sec[r.pp_bonuses];
    let pp_bonuses = [
        u32::from((pp_bonuses_byte >> 0) & 0x3),
        u32::from((pp_bonuses_byte >> 2) & 0x3),
        u32::from((pp_bonuses_byte >> 4) & 0x3),
        u32::from((pp_bonuses_byte >> 6) & 0x3),
    ];

    let moves = [
        u32::from(u16_le(sec, r.moves) & 0x7FF),
        u32::from(u16_le(sec, r.moves + 2) & 0x7FF),
        u32::from(u16_le(sec, r.moves + 4) & 0x7FF),
        u32::from(u16_le(sec, r.moves + 6) & 0x7FF),
    ];

    let pps = [
        u32::from(sec[r.pps] & 0x7F),
        u32::from(sec[r.pps + 1] & 0x7F),
        u32::from(sec[r.pps + 2] & 0x7F),
        u32::from(sec[r.pps + 3] & 0x7F),
    ];

    let mut evs = [0u32; 6];
    for (i, ev) in evs.iter_mut().enumerate() {
        *ev = u32::from(sec[r.evs + i]);
    }

    let pokerus = sec[r.pokerus];
    let met_location = sec[r.met_location];
    let met_level = u32::from(u16_le(sec, r.met_level) & 0x7F);

    // The ability slot shares its word with move 4.
    let ability_num = ((u16_le(sec, r.moves + 6) >> 12) & 0x3) as u8;

    let iv_word = u32_le(sec, r.ivs);
    let ivs = [
        iv_word & 0x1F,
        (iv_word >> 5) & 0x1F,
        (iv_word >> 10) & 0x1F,
        (iv_word >> 15) & 0x1F,
        (iv_word >> 20) & 0x1F,
        (iv_word >> 25) & 0x1F,
    ];

    // A nickname that is just the species name is treated as "no nickname".
    let full_species = get_species_name(species).to_uppercase();
    let base_species = strip_form_prefixes(&full_species);
    let raw_nick_clean = raw_nick.to_uppercase();
    let raw_nick_clean = raw_nick_clean.trim();
    let is_default_nick = raw_nick_clean.is_empty()
        || raw_nick_clean == full_species
        || raw_nick_clean == base_species;

    let mut mon = Pokemon {
        personality,
        ot_id,
        nickname: if is_default_nick {
            String::new()
        } else {
            raw_nick
        },
        ot_name,
        language,
        hidden_nature_modifier,
        is_bad_egg,
        has_species,
        is_egg,
        shiny_modifier,
        species,
        tera_type: nz(tera_type, 1),
        held_item,
        pokeball: nz(pokeball, 1),
        experience,
        moves,
        pps,
        evs,
        ivs,
        pp_bonuses,
        pokerus,
        met_location,
        met_level,
        // A boxed Pokémon stores no level; the game derives it from experience
        // and the species' growth rate, so do the same. (The web original left
        // a hardcoded 5 here, which is why every boxed Pokémon read as Lv.5.)
        // For party members this is overwritten below with the stored byte.
        level: super::experience::level_from_experience(species, experience),
        friendship: sec[r.friendship], // 0 is a real value, not "unset"
        ability_num,
        status: None,
        hp: None,
        max_hp: None,
        attack: None,
        defense: None,
        speed: None,
        sp_attack: None,
        sp_defense: None,
    };

    if is_party && b.len() >= mon_size() {
        mon.status = Some(u32_le(b, r.status));
        mon.level = b[r.level];
        mon.hp = Some(u16_le(b, r.hp));
        mon.max_hp = Some(u16_le(b, r.max_hp));
        mon.attack = Some(u16_le(b, r.attack));
        mon.defense = Some(u16_le(b, r.defense));
        mon.speed = Some(u16_le(b, r.speed));
        mon.sp_attack = Some(u16_le(b, r.sp_attack));
        mon.sp_defense = Some(u16_le(b, r.sp_defense));
    }

    Some(mon)
}

/// `replace(/\b(MEGA(\s+[XY])?|PRIMAL|GMAX)\b/gi, '').trim()` on an
/// already-uppercased species name.
fn strip_form_prefixes(upper_name: &str) -> String {
    let words: Vec<&str> = upper_name.split_whitespace().collect();
    let mut out: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        match words[i] {
            "MEGA" => {
                // "MEGA X" / "MEGA Y" drop the suffix letter too.
                if matches!(words.get(i + 1), Some(&"X") | Some(&"Y")) {
                    i += 1;
                }
            }
            "PRIMAL" | "GMAX" => {}
            other => out.push(other),
        }
        i += 1;
    }
    out.join(" ").trim().to_string()
}
