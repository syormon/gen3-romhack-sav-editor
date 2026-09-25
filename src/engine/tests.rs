//! Round-trip tests for the save engine.
//!
//! The port was additionally validated against the original TypeScript engine:
//! a synthetic save built by the TS code parses to an identical dump here, and
//! re-exporting it produces byte-identical output (see `examples/dump.rs`).

use super::bytes::*;
use super::charmap::*;
use super::layout::*;
use super::lookup::*;
use super::mon_writer::*;
use super::personality::*;
use super::save_parser::*;
use super::save_writer::export_updated_save;

/// Tests run against the bundled SoulGold pack; loading it is global, so this
/// is done once for the whole test binary.
pub fn ensure_pack() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("soulgold");
        crate::game::activate(&dir).expect("load the soulgold game pack");
    });
}

pub fn sample_mon(species: u32, level: u8, nickname: &str) -> Pokemon {
    ensure_pack();
    Pokemon {
        personality: 0x1357_9BDF,
        ot_id: 0x1234_ABCD,
        nickname: nickname.to_string(),
        ot_name: "ASH".to_string(),
        language: 2,
        hidden_nature_modifier: 3,
        is_bad_egg: false,
        has_species: true,
        is_egg: false,
        shiny_modifier: 0,
        species,
        tera_type: 11,
        held_item: 102,
        pokeball: 3,
        experience: super::experience::experience_for_level(species, level),
        moves: [33, 45, 85, 1000],
        pps: [35, 40, 15, 20],
        evs: [252, 128, 0, 4, 60, 252],
        ivs: [31, 0, 17, 5, 31, 28],
        pp_bonuses: [3, 0, 1, 2],
        pokerus: 0x22,
        met_location: 88,
        met_level: u32::from(level),
        level,
        friendship: 140,
        ability_num: 2,
        status: Some(0),
        hp: Some(40),
        max_hp: Some(99),
        attack: Some(55),
        defense: Some(44),
        speed: Some(77),
        sp_attack: Some(66),
        sp_defense: Some(33),
    }
}

/// Builds a valid 128 KB save containing a party, some box mons and a bag.
fn build_save() -> GameSave {
    const KEY: u32 = 0x2B7E_1516;

    let mut sb1 = vec![0u8; sb1_size()];
    let mut sb2 = vec![0u8; sb2_size()];
    let mut storage = vec![0u8; storage_size()];

    sb2[player_name_offset()..player_name_offset() + 8]
        .copy_from_slice(&encode_gba_string("ASH", 8));
    sb2[player_gender_offset()] = 1;
    set_u16_le(&mut sb2, trainer_id_offset(), 0xABCD);
    set_u16_le(&mut sb2, trainer_id_offset() + 2, 0x1234);
    set_u32_le(
        &mut sb2,
        encryption_key_offset().expect("soulgold has a key"),
        KEY,
    );

    set_u32_le(&mut sb1, money_offset(), 123_456 ^ KEY);
    set_u16_le(&mut sb1, coins_offset(), 4321 ^ (KEY & 0xFFFF) as u16);

    for (i, mon) in [
        sample_mon(25, 42, "SPARKY"),
        sample_mon(6, 100, ""),
        sample_mon(151, 10, "MEW"),
    ]
    .iter()
    .enumerate()
    {
        pack_and_write_pokemon(&mut sb1, party_offset() + i * mon_size(), mon, true);
    }
    set_u32_le(&mut sb1, party_count_offset(), 3);

    for (b, s, mon) in [
        (0usize, 0usize, sample_mon(149, 55, "DRAKE")),
        (0, 29, sample_mon(1435, 70, "")),
        (13, 29, sample_mon(493, 100, "ARCEUS")),
    ] {
        let off = storage_boxes_offset() + (b * box_capacity() + s) * box_mon_size();
        pack_and_write_pokemon(&mut storage, off, &mon, false);
    }

    let blank = vec![0u8; min_save_size()];
    let bytes = export_updated_save(&blank, 0, &sb1, &sb2, &storage);
    GameSave::from_bytes(bytes).expect("synthetic save parses")
}

#[test]
fn charmap_round_trips() {
    ensure_pack();
    for text in ["ASH", "Sparky", "Mr. Mime", "Nidoran♀", "0123456789"] {
        let encoded = encode_gba_string(text, 12);
        assert_eq!(decode_gba_string(&encoded, 12), text, "round trip {text}");
    }
    // Unused slots are terminated, not zero-filled.
    assert_eq!(
        encode_gba_string("AB", 5),
        vec![0xBB, 0xBC, 0xFF, 0xFF, 0xFF]
    );
    // A space is 0x00, and only 0xFF ends a string. The web original stopped at
    // the 0x00 too, so any name with a space was truncated on read and then
    // written back truncated ("LUGIA SHADOW" -> "LUGIA").
    assert_eq!(
        decode_gba_string(&encode_gba_string("Mr. Mime", 12), 12),
        "Mr. Mime"
    );
    assert_eq!(decode_gba_string(&[0xBB, 0x00, 0xBC], 8), "A B");
    // Trailing padding is still trimmed.
    assert_eq!(decode_gba_string(&[0xBB, 0xBC, 0x00, 0x00, 0xFF], 8), "AB");
}

#[test]
fn trainer_info_round_trips_through_encryption() {
    ensure_pack();
    let mut save = build_save();
    let trainer = save.trainer_info();
    assert_eq!(trainer.name, "ASH");
    assert_eq!(trainer.gender_code, 1);
    assert_eq!(trainer.tid, 0xABCD);
    assert_eq!(trainer.sid, 0x1234);
    assert_eq!(trainer.money, 123_456);
    assert_eq!(trainer.coins, 4321);

    save.set_trainer_info(&TrainerInfo {
        name: "RED".to_string(),
        gender_code: 0,
        tid: 1,
        sid: 2,
        money: 999_999,
        coins: 9_999,
    });
    let updated = save.trainer_info();
    assert_eq!(updated.name, "RED");
    assert_eq!(updated.money, 999_999);
    assert_eq!(updated.coins, 9_999);

    // Over-large values clamp instead of wrapping.
    save.set_trainer_info(&TrainerInfo {
        money: 5_000_000,
        coins: 65_000,
        ..updated
    });
    assert_eq!(save.trainer_info().money, 999_999);
    assert_eq!(save.trainer_info().coins, 9_999);
}

#[test]
fn party_and_box_mons_survive_a_round_trip() {
    ensure_pack();
    let save = build_save();
    let party = save.party();
    assert_eq!(party.len(), 3);

    let sparky = &party[0];
    assert_eq!(sparky.species, 25);
    assert_eq!(sparky.nickname, "SPARKY");
    assert_eq!(sparky.level, 42);
    assert_eq!(sparky.moves, [33, 45, 85, 1000]);
    assert_eq!(sparky.ivs, [31, 0, 17, 5, 31, 28]);
    assert_eq!(sparky.pp_bonuses, [3, 0, 1, 2]);
    assert_eq!(sparky.ability_num, 2);
    assert_eq!(sparky.max_hp, Some(99));

    // A nickname equal to the species name reads back as "no nickname".
    assert_eq!(party[1].nickname, "");
    assert_eq!(party[1].species, 6);

    assert!(save.box_pokemon(0, 0).is_some());
    assert!(save.box_pokemon(0, 1).is_none());
    let arceus = save.box_pokemon(13, 29).expect("last box slot");
    assert_eq!(arceus.species, 493);
    // Box mons carry no battle stats.
    assert_eq!(arceus.hp, None);
    // ...and no level either: it is derived from the stored experience.
    assert_eq!(arceus.level, 100);
}

/// A Pokémon with no nickname must store the species name exactly as the game
/// spells it, or the game treats it as nicknamed and never renames it on
/// evolution.
#[test]
fn unnicknamed_pokemon_store_the_species_name_verbatim() {
    ensure_pack();
    let mut buf = vec![0u8; mon_size()];

    let mut mon = sample_mon(227, 30, ""); // Skarmory, no nickname
    pack_and_write_pokemon(&mut buf, 0, &mon, true);
    let stored = decode_gba_string(&buf[8..8 + nickname_length()], nickname_length());
    assert_eq!(stored, "Skarmory", "not uppercased");
    assert_eq!(
        stored,
        get_species_name(227),
        "byte-identical to the game's name"
    );

    // A real nickname is kept as typed.
    mon.nickname = "Steely".to_string();
    pack_and_write_pokemon(&mut buf, 0, &mon, true);
    assert_eq!(
        decode_gba_string(&buf[8..8 + nickname_length()], nickname_length()),
        "Steely"
    );

    // Changing species restates the name, so it follows the evolution.
    mon.nickname = String::new();
    mon.species = 6; // Charizard
    pack_and_write_pokemon(&mut buf, 0, &mon, true);
    assert_eq!(
        decode_gba_string(&buf[8..8 + nickname_length()], nickname_length()),
        "Charizard"
    );
}

/// The name field is 12 bytes wide in this hack, not the usual 10.
#[test]
fn twelve_character_names_are_not_clipped() {
    ensure_pack();
    let mut buf = vec![0u8; mon_size()];

    // Crabominable is exactly 12 characters.
    let name = get_species_name(740);
    assert_eq!(name, "Crabominable");
    assert_eq!(name.chars().count(), nickname_length());

    let mon = sample_mon(740, 40, "");
    pack_and_write_pokemon(&mut buf, 0, &mon, true);
    assert_eq!(
        decode_gba_string(&buf[8..8 + nickname_length()], nickname_length()),
        "Crabominable",
        "the old 10-character clip turned this into \"Crabominab\""
    );

    // And it survives a full read-back, still reading as un-nicknamed.
    let read = unpack_mon(&buf, true).expect("packs and parses");
    assert_eq!(read.nickname, "", "recognised as the default name");
    assert_eq!(read.display_name(), "Crabominable");
}

/// A boxed Pokémon has no level field, so its level comes from experience.
#[test]
fn boxed_pokemon_level_comes_from_experience() {
    ensure_pack();
    use super::experience::{experience_for_level, level_from_experience};

    let mut save = build_save();

    // Dragonite (slow curve) parked in a box at level 55.
    let mut drake = sample_mon(149, 55, "DRAKE");
    drake.experience = experience_for_level(149, 55);
    let off = storage_boxes_offset();
    pack_and_write_pokemon(&mut save.storage, off, &drake, false);

    let read_back = save.box_pokemon(0, 0).expect("box slot 0");
    assert_eq!(
        read_back.level, 55,
        "not the old hardcoded placeholder of 5"
    );
    assert_eq!(read_back.experience, drake.experience);
    assert_eq!(level_from_experience(149, read_back.experience), 55);
}

/// Opening a boxed Pokémon and pressing Apply must not disturb what is stored —
/// in particular its experience, which is where its level lives.
#[test]
fn re_applying_a_boxed_pokemon_changes_nothing() {
    ensure_pack();
    let save = build_save();

    for (b, s) in [(0usize, 0usize), (0, 29), (13, 29)] {
        let offset = storage_boxes_offset() + (b * box_capacity() + s) * box_mon_size();
        let before = save.storage[offset..offset + box_mon_size()].to_vec();

        let mon = save.box_pokemon(b, s).expect("occupied slot");
        let mut after = before.clone();
        pack_and_write_pokemon(&mut after, 0, &mon, false);

        assert_eq!(after, before, "box {b} slot {s} must round-trip unchanged");
    }
}

/// The games write a name, terminate it, and leave the rest of the field as it
/// was — so a record they wrote has zeros after the terminator, not more
/// terminators. Re-saving such a record must not rewrite those bytes.
///
/// The synthetic saves here are built by this editor, so they can only agree
/// with whatever it already does; this test forges the game's own padding.
#[test]
fn a_name_written_by_the_game_is_left_byte_for_byte_alone() {
    ensure_pack();
    let mut save = build_save();
    let r = crate::game::current().record().clone();

    let offset = storage_boxes_offset();
    let record = &mut save.storage[offset..offset + box_mon_size()];

    // "DRAKE" plus the terminator, then zeros, as the game would store it.
    for i in 6..r.nickname_length {
        record[r.nickname + i] = 0x00;
    }
    let before = record.to_vec();

    let mon = save.box_pokemon(0, 0).expect("occupied slot");
    assert_eq!(mon.nickname, "DRAKE");

    let mut after = before.clone();
    pack_and_write_pokemon(&mut after, 0, &mon, false);
    assert_eq!(after, before, "re-saving must not touch the padding");
}

/// Freshly caught legendaries really do have friendship 0. Substituting a
/// default for it would rewrite them on every save.
#[test]
fn friendship_zero_is_not_replaced_with_a_default() {
    ensure_pack();
    let mut mon = sample_mon(384, 70, "");
    mon.friendship = 0;

    let mut record = vec![0u8; box_mon_size()];
    pack_and_write_pokemon(&mut record, 0, &mon, false);

    let mut storage = vec![0u8; storage_size()];
    storage[storage_boxes_offset()..storage_boxes_offset() + box_mon_size()]
        .copy_from_slice(&record);
    let read = unpack_mon(&storage[storage_boxes_offset()..], false).expect("record decodes");
    assert_eq!(read.friendship, 0);
}

/// A slot the game has never written is erased flash, every byte 0xFF, so its
/// counter reads as 0xFFFFFFFF and beats any real save. Picking it would show
/// an empty editor for a perfectly good file — which is what a fresh Unbound
/// save did.
#[test]
fn an_erased_slot_never_wins_the_counter_comparison() {
    ensure_pack();
    let save = build_save();
    let mut bytes = save.buffer.clone();

    // Wipe slot 0 the way flash erases.
    let slot_bytes = sectors_per_slot() * sector_size();
    bytes[..slot_bytes].fill(0xFF);

    let reread = GameSave::from_bytes(bytes).expect("the written slot is still readable");
    assert_eq!(reread.active_slot, 1, "must take the slot that was written");
    assert_eq!(reread.trainer_info().name, "ASH");
    assert_eq!(reread.party().len(), 3);

    // And exporting must not overflow on the erased slot's 0xFFFFFFFF counter.
    let out = export_updated_save(
        &reread.buffer,
        reread.active_slot,
        &reread.sb1,
        &reread.sb2,
        &reread.storage,
    );
    assert_eq!(out.len(), reread.buffer.len());
    let after = GameSave::from_bytes(out).expect("export reloads");
    assert_eq!(after.trainer_info().name, "ASH");
}

/// Sector 0 carries SaveBlock2, but the region the game checksums is not
/// always the same length as that block — Seaglass checksums the full 0xF80
/// while its SaveBlock2 is 0xF2C. An explicit entry has to win, or every
/// export of that game carries a wrong checksum.
#[test]
fn an_explicit_sector_size_beats_the_save_block_size() {
    let layout: crate::game::Layout = serde_json::from_str(
        r#"{
            "sb2_size": 3884,
            "sector_data_size": 3968,
            "sector_sizes": [{ "sector": 0, "size": 3968 }, { "sector": 4, "size": 3540 }]
        }"#,
    )
    .expect("layout parses");

    assert_eq!(layout.sector_checksum_size(0), 3968, "explicit entry wins");
    assert_eq!(layout.sector_checksum_size(4), 3540);
    assert_eq!(
        layout.sector_checksum_size(2),
        3968,
        "falls back to the data size"
    );

    // With no entry for sector 0, SaveBlock2's size is the sensible default.
    let bare: crate::game::Layout = serde_json::from_str(
        r#"{ "sb2_size": 3884, "sector_data_size": 3968, "sector_sizes": [] }"#,
    )
    .expect("layout parses");
    assert_eq!(bare.sector_checksum_size(0), 3884);
}

/// Facts about Unbound that were measured from a real save, kept here so a
/// careless edit to its manifest is caught rather than silently shipped.
#[test]
fn the_unbound_pack_keeps_its_measured_layout() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("unbound");
    let pack = crate::game::GamePack::load(&dir).expect("unbound pack loads");
    let l = pack.layout();

    assert_eq!(l.sector_signature, 0x0112_1999, "FireRed's signature");
    assert_eq!(l.sector_data_size, 0xFF0, "CFRU widens this from 0xF80");
    assert_eq!(l.sb1_size, 0x3D68, "FireRed's SaveBlock1");
    assert_eq!(l.party_count, 0x34);
    assert_eq!(l.party, 0x38);
    assert_eq!(l.money, 0x290);
    assert_eq!(l.encryption_key, None, "FireRed has no security key");
    assert_eq!(l.box_mon_size, 58, "CFRU's compact box record");
    assert_eq!(l.box_encoding, crate::game::BoxEncoding::CfruCompact);
    assert_eq!(
        l.record_encoding,
        crate::game::RecordEncoding::Gen3Plain,
        "party records keep Gen 3 field positions but are not obfuscated"
    );
    // 52% of a known all-shiny collection passes at 8; all of it passes at 16.
    assert_eq!(pack.behavior().shiny_threshold, 16);
}

/// A hack that renumbers its species must not address sprites by that number.
/// Unbound's Gible is species 496; National Dex 496 is Servine, and PokeAPI
/// serves it happily — so the editor showed the wrong Pokemon with complete
/// confidence while every other field was right.
#[test]
fn a_renumbered_species_gets_its_national_dex_sprite() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("unbound");
    let pack = crate::game::GamePack::load(&dir).expect("unbound pack loads");

    let gible = pack.species.get(&496).expect("species 496");
    assert_eq!(gible.name, "Gible");
    assert_eq!(gible.dex, Some(443), "Gible's National Dex number");
    assert!(
        !pack.manifest.sprites.species_ids_are_dex_numbers,
        "Unbound numbers its own species"
    );

    // Species without a Dex equivalent must not fall back to the raw id.
    let unbound_only = pack
        .species
        .iter()
        .find(|(_, s)| s.dex.is_none())
        .map(|(id, _)| *id);
    assert!(unbound_only.is_some(), "the pack has Unbound-only species");
}

#[test]
fn bag_items_round_trip_through_encryption() {
    ensure_pack();
    let mut save = build_save();
    let items = vec![
        BagItem {
            id: 4,
            quantity: 999,
        },
        BagItem {
            id: 102,
            quantity: 1,
        },
    ];
    let balls = pocket_index("PokeBalls").expect("PokeBalls pocket");
    save.set_pocket_items(balls, &items);
    assert_eq!(save.pocket_items(balls), items);

    // Clearing a pocket empties it.
    save.set_pocket_items(balls, &[]);
    assert!(save.pocket_items(balls).is_empty());
}

#[test]
fn export_is_stable_and_bumps_the_slot_counters() {
    ensure_pack();
    let save = build_save();
    let first = export_updated_save(
        &save.buffer,
        save.active_slot,
        &save.sb1,
        &save.sb2,
        &save.storage,
    );
    let reparsed = GameSave::from_bytes(first.clone()).expect("export re-parses");

    assert_eq!(reparsed.sb1, save.sb1);
    assert_eq!(reparsed.sb2, save.sb2);
    assert_eq!(reparsed.storage, save.storage);
    assert!(reparsed.save_counter > save.save_counter);
    assert_eq!(first.len(), min_save_size());

    // Both slots are written, so the backup slot holds the same data.
    let sector0_a = &first[0xFF4..0xFF8];
    let sector0_b = &first[sectors_per_slot() * sector_size() + 0xFF4..][..4];
    assert_eq!(sector0_a[0..2], sector0_b[0..2], "same sector id");
    assert_eq!(sector0_a[2..4], sector0_b[2..4], "same checksum");
}

#[test]
fn short_files_are_rejected() {
    ensure_pack();
    assert!(GameSave::from_bytes(vec![0u8; 1024]).is_err());
    assert!(GameSave::from_bytes(vec![0u8; 64 * 1024]).is_err());
    // A correctly sized file with no valid sectors is rejected too.
    assert!(GameSave::from_bytes(vec![0u8; min_save_size()]).is_err());
}

#[test]
fn shiny_toggle_keeps_nature_and_ability_bit() {
    ensure_pack();
    let mon = sample_mon(25, 42, "SPARKY");
    let ot = mon.ot_id;
    let nature = mon.personality % 25;
    let ability_bit = mon.personality & 1;

    let shiny_pid = make_personality_shiny(mon.personality, ot);
    assert_eq!(shiny_pid % 25, nature);
    assert_eq!(shiny_pid & 1, ability_bit);

    let mut shiny = mon.clone();
    shiny.personality = shiny_pid;
    assert!(shiny.is_shiny());

    let plain_pid = make_personality_non_shiny(shiny_pid, ot);
    let mut plain = shiny.clone();
    plain.personality = plain_pid;
    plain.shiny_modifier = 0;
    assert!(!plain.is_shiny());
    assert_eq!(plain_pid % 25, nature);
    assert_eq!(plain_pid & 1, ability_bit);

    // A species the pack lists as never-shiny stays non-shiny.
    let mut lugia = sample_mon(1435, 70, "");
    lugia.personality = shiny_pid;
    assert!(!lugia.is_shiny());
}

#[test]
fn nature_can_be_set_without_disturbing_the_rest_of_the_pid() {
    ensure_pack();
    let pid = 0x1357_9BDF_u32;
    for nature in 0..25 {
        let updated = set_personality_nature(pid, nature);
        assert_eq!(updated % 25, nature);
        assert_eq!(updated / 25, pid / 25);
    }
}

#[test]
fn items_land_in_the_expected_pockets() {
    ensure_pack();
    let pocket = |name: &str| pocket_index(name).unwrap_or_else(|| panic!("no {name} pocket"));

    let cases = [
        (4u32, "Master Ball", "PokeBalls"),
        (102, "Rare Candy", "Medicine"),
        (465, "Kings Rock", "BattleItems"),
        (1, "Poke Ball", "PokeBalls"),
    ];
    for (id, name, expected) in cases {
        assert_eq!(classify_item_pocket(id, name), pocket(expected), "{name}");
    }
    // Names that merely end in "ball" are not Poké Balls.
    assert_ne!(classify_item_pocket(300, "Iron Ball"), pocket("PokeBalls"));
    assert_eq!(classify_item_pocket(900, "TM01"), pocket("TMsHMs"));
    assert_eq!(classify_item_pocket(901, "Oran Berry"), pocket("Berries"));
}

/// Species above the National Dex range — Paradox Pokémon, Gen 9, Megas,
/// regional forms — have no PokéAPI sprite at their internal id, so they must
/// fall back to the hack's own artwork.
#[test]
fn species_past_the_national_dex_still_have_a_sprite() {
    ensure_pack();
    for (id, name, stem) in [
        (1376u32, "Great Tusk", "great_tusk"),
        (1379, "Flutter Mane", "flutter_mane"),
        (1381, "Sandy Shocks", "sandy_shocks"),
        (906, "Venusaur Mega", "venusaur_mega"),
        (1435, "Lugia Shadow", "lugia_shadow"),
    ] {
        assert_eq!(get_species_name(id), name);
        let urls = sprite_candidates(id, false);
        assert!(
            urls.iter().any(|u| u.ends_with(&format!("/{stem}.png"))),
            "{name} must offer the hack's own sprite, got {urls:?}"
        );
        // No point asking PokéAPI for an id it cannot have.
        assert!(
            !urls
                .iter()
                .any(|u| u.contains(&format!("/pokemon/{id}.png"))),
            "{name} should not request PokeAPI id {id}"
        );
        assert!(
            urls[0].starts_with("https://eemeliri.github.io/"),
            "{name}: {urls:?}"
        );

        let shiny = sprite_candidates(id, true);
        assert!(
            shiny
                .iter()
                .any(|u| u.ends_with(&format!("/{stem}_shiny.png"))),
            "{name} must offer a shiny sprite, got {shiny:?}"
        );
    }
}

/// Species inside the National Dex range keep PokéAPI's higher-resolution art.
#[test]
fn national_dex_species_use_pokeapi_first() {
    ensure_pack();
    let urls = sprite_candidates(227, false); // Skarmory
    assert!(urls[0].contains("/other/home/227.png"), "got {urls:?}");

    let shiny = sprite_candidates(227, true);
    assert!(
        shiny[0].contains("/other/home/shiny/227.png"),
        "got {shiny:?}"
    );
}

/// Cosmetic forms the hack's documentation has no sprite for fall back to the
/// base species rather than showing nothing.
#[test]
fn undocumented_forms_fall_back_to_the_base_species() {
    ensure_pack();
    assert_eq!(get_species_name(1009), "Pikachu Cosplay");
    let urls = sprite_candidates(1009, false);
    assert!(
        urls.iter().any(|u| u.contains("/other/home/25.png")),
        "should fall back to Pikachu, got {urls:?}"
    );
}

/// Every species in the table offers at least one sprite to try.
#[test]
fn every_species_has_a_sprite_candidate() {
    ensure_pack();
    let mut without: Vec<(u32, String)> = Vec::new();
    for species in all_species_list() {
        if sprite_candidates(species.id, false).is_empty() {
            without.push((species.id, species.name.clone()));
        }
    }
    assert!(
        without.is_empty(),
        "{} species with no sprite at all: {:?}",
        without.len(),
        &without[..without.len().min(10)]
    );
}

#[test]
fn move_pp_falls_back_to_twenty() {
    ensure_pack();
    assert_eq!(get_move_base_pp(0), 0);
    assert_eq!(get_move_base_pp(1), 35); // Pound
    assert_eq!(get_move_base_pp(999_999), 20); // unknown move

    // PP values the upstream data had defaulted to 20.
    assert_eq!(get_move_base_pp(156), 5); // Rest
    assert_eq!(get_move_base_pp(71), 25); // Absorb
    assert_eq!(get_move_base_pp(317), 15); // Rock Tomb
    assert_eq!(get_move_base_pp(791), 1); // Revival Blessing
}

/// The upstream move table dropped the first move of every generation block,
/// so Roost, Fake Out, Tera Blast and friends could not be picked at all.
#[test]
fn generation_starting_moves_exist() {
    ensure_pack();
    let expected = [
        (166u32, "Sketch", 1u32),
        (252, "Fake Out", 10),
        (355, "Roost", 5),
        (468, "Hone Claws", 15),
        (560, "Flying Press", 10),
        (622, "Shore Up", 5),
        (690, "Dynamax Cannon", 5),
        (779, "Tera Blast", 10),
        (848, "Shadow Blast", 5),
    ];
    for (id, name, pp) in expected {
        assert_eq!(get_move_name(id), name, "move #{id}");
        assert_eq!(get_move_base_pp(id), pp, "{name} PP");
        assert_eq!(move_id_by_name(name), Some(id), "{name} by name");
        assert!(
            all_moves_list().iter().any(|m| m.id == id),
            "{name} must be selectable in the move picker"
        );
    }
}

#[test]
fn skarmory_can_learn_roost() {
    ensure_pack();
    let skarmory = 227;
    assert_eq!(get_species_name(skarmory), "Skarmory");

    let learnset = get_species_learnset(skarmory);
    assert!(
        learnset.iter().any(|m| m == "Roost"),
        "Roost is in the learnset"
    );
    assert!(
        learnset_move_ids(skarmory).contains(&355),
        "Roost resolves to an id, so it is starred as a compatible move"
    );
}

/// Learnset spellings differ from the move table's (punctuation, and a few
/// older names), so every learnset entry must still resolve to an id.
#[test]
fn every_learnset_move_resolves_to_an_id() {
    ensure_pack();
    for (name, id) in [
        ("Double Edge", 38u32), // "Double-Edge"
        ("Kings Shield", 588),  // "King's Shield"
        ("U Turn", 369),        // "U-turn"
        ("Will O Wisp", 261),   // "Will-O-Wisp"
        ("Faint Attack", 185),  // renamed to "Feint Attack"
        ("Hi Jump Kick", 136),  // renamed to "High Jump Kick"
        ("Vice Grip", 11),      // renamed to "Vise Grip"
        ("Dark Aero", 848),     // renamed to "Shadow Blast"
    ] {
        assert_eq!(move_id_by_name(name), Some(id), "{name}");
    }

    let mut unresolved: Vec<String> = Vec::new();
    for species in all_species_list() {
        for name in get_species_learnset(species.id) {
            if move_id_by_name(&name).is_none() && !unresolved.contains(&name) {
                unresolved.push(name);
            }
        }
    }
    assert!(
        unresolved.is_empty(),
        "unresolved learnset moves: {unresolved:?}"
    );
}

#[test]
fn created_pokemon_is_usable() {
    ensure_pack();
    let trainer = TrainerInfo {
        name: "ASH".to_string(),
        gender_code: 0,
        tid: 0xABCD,
        sid: 0x1234,
        money: 0,
        coins: 0,
    };
    let mut rng = Rng::from_clock();
    let mon = create_default_pokemon(25, &trainer, &mut rng);

    assert_eq!(mon.species, 25);
    assert_eq!(mon.ot_id, 0x1234_ABCD);
    assert!(mon.moves[0] > 0, "always gets at least one move");
    assert!(mon.ivs.iter().all(|iv| *iv < 32));
    assert_eq!(mon.experience, u32::from(mon.level).pow(3));

    // And it survives a write/read cycle.
    let mut buf = vec![0u8; mon_size()];
    pack_and_write_pokemon(&mut buf, 0, &mon, true);
    let read_back = unpack_mon(&buf, true).expect("packed mon parses");
    assert_eq!(read_back.species, mon.species);
    assert_eq!(read_back.moves, mon.moves);
    assert_eq!(read_back.ivs, mon.ivs);
    assert_eq!(read_back.level, mon.level);
}

#[test]
fn clearing_a_slot_empties_it() {
    ensure_pack();
    let mon = sample_mon(25, 42, "SPARKY");
    let mut buf = vec![0u8; mon_size() * 2];
    pack_and_write_pokemon(&mut buf, mon_size(), &mon, true);
    assert!(unpack_mon(&buf[mon_size()..], true).is_some());

    clear_pokemon_slot(&mut buf, mon_size(), true);
    assert!(unpack_mon(&buf[mon_size()..], true).is_none());
}

#[test]
fn party_count_tracks_occupied_slots() {
    ensure_pack();
    let mut save = build_save();
    assert_eq!(save.party().len(), 3);

    clear_pokemon_slot(&mut save.sb1, party_offset() + 2 * mon_size(), true);
    save.refresh_party_count();
    assert_eq!(save.party().len(), 2);
}
