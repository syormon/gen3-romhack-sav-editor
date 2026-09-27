//! Name and data lookups against the game pack that is currently loaded.
//!
//! Every table here comes from `assets/<game>/*.json` via
//! [`crate::game`]; nothing in this file is specific to one ROM hack. The
//! per-game knobs (never-shiny species, starting levels, sprite sources) live
//! in that game's `game.json`.

use std::collections::HashSet;

use crate::game::{self, NamedId};

pub use crate::game::move_key;

// ------------------------------------------------------------------- naming

pub fn get_species_name(id: u32) -> String {
    game::current()
        .species
        .get(&id)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| format!("Species #{id}"))
}

pub fn get_item_name(id: u32) -> String {
    if id == 0 {
        return "(None)".to_string();
    }
    game::current()
        .items
        .get(&id)
        .cloned()
        .unwrap_or_else(|| format!("Item #{id}"))
}

pub fn get_move_name(id: u32) -> String {
    if id == 0 {
        return "(None)".to_string();
    }
    game::current()
        .moves
        .get(&id)
        .cloned()
        .unwrap_or_else(|| format!("Move #{id}"))
}

pub fn get_move_base_pp(move_id: u32) -> u32 {
    if move_id == 0 {
        return 0;
    }
    match game::current().move_pps.get(&move_id) {
        Some(&pp) if pp != 0 => pp,
        _ => 20,
    }
}

/// A move's PP with `bonus` PP Ups applied, as the games compute it: each
/// adds a fifth of the base, rounded down.
pub fn max_pp(move_id: u32, bonus: u32) -> u32 {
    let base = get_move_base_pp(move_id);
    base + base * bonus.min(3) / 5
}

/// How many ability slots a record can say. Gen 3 records keep one bit for it,
/// so a species' hidden ability cannot be stored there.
pub fn storable_ability_slots() -> u8 {
    if game::current().layout().record_encoding.is_gen3() {
        2
    } else {
        3
    }
}

/// Looks up a move by name, tolerating punctuation and the pack's aliases.
pub fn move_id_by_name(name: &str) -> Option<u32> {
    game::current()
        .move_name_to_id
        .get(&move_key(name))
        .copied()
}

// -------------------------------------------------------------------- lists

pub fn all_species_list() -> Vec<NamedId> {
    game::current().species_sorted.clone()
}

pub fn alphabetical_item_list() -> Vec<NamedId> {
    game::current().items_alphabetical.clone()
}

pub fn all_moves_list() -> Vec<NamedId> {
    game::current().moves_alphabetical.clone()
}

/// Poké Balls the editor offers, from the pack.
pub fn pokeballs() -> Vec<NamedId> {
    game::current()
        .behavior()
        .pokeballs
        .iter()
        .map(|b| NamedId {
            id: b.id,
            name: b.name.clone(),
        })
        .collect()
}

/// Tera types the editor offers; empty means the game has none.
pub fn tera_types() -> Vec<NamedId> {
    game::current()
        .behavior()
        .tera_types
        .iter()
        .map(|t| NamedId {
            id: t.id,
            name: t.name.clone(),
        })
        .collect()
}

/// Natures are a property of the personality value, not of any one game.
pub struct Nature {
    pub id: u32,
    pub name: &'static str,
    pub modifier: &'static str,
}

pub const NATURES_LIST: [Nature; 25] = [
    Nature {
        id: 0,
        name: "Hardy",
        modifier: "(Neutral)",
    },
    Nature {
        id: 1,
        name: "Lonely",
        modifier: "(+Atk, -Def)",
    },
    Nature {
        id: 2,
        name: "Brave",
        modifier: "(+Atk, -Spe)",
    },
    Nature {
        id: 3,
        name: "Adamant",
        modifier: "(+Atk, -SpA)",
    },
    Nature {
        id: 4,
        name: "Naughty",
        modifier: "(+Atk, -SpD)",
    },
    Nature {
        id: 5,
        name: "Bold",
        modifier: "(+Def, -Atk)",
    },
    Nature {
        id: 6,
        name: "Docile",
        modifier: "(Neutral)",
    },
    Nature {
        id: 7,
        name: "Relaxed",
        modifier: "(+Def, -Spe)",
    },
    Nature {
        id: 8,
        name: "Impish",
        modifier: "(+Def, -SpA)",
    },
    Nature {
        id: 9,
        name: "Lax",
        modifier: "(+Def, -SpD)",
    },
    Nature {
        id: 10,
        name: "Timid",
        modifier: "(+Spe, -Atk)",
    },
    Nature {
        id: 11,
        name: "Hasty",
        modifier: "(+Spe, -Def)",
    },
    Nature {
        id: 12,
        name: "Serious",
        modifier: "(Neutral)",
    },
    Nature {
        id: 13,
        name: "Jolly",
        modifier: "(+Spe, -SpA)",
    },
    Nature {
        id: 14,
        name: "Naive",
        modifier: "(+Spe, -SpD)",
    },
    Nature {
        id: 15,
        name: "Modest",
        modifier: "(+SpA, -Atk)",
    },
    Nature {
        id: 16,
        name: "Mild",
        modifier: "(+SpA, -Def)",
    },
    Nature {
        id: 17,
        name: "Quiet",
        modifier: "(+SpA, -Spe)",
    },
    Nature {
        id: 18,
        name: "Bashful",
        modifier: "(Neutral)",
    },
    Nature {
        id: 19,
        name: "Rash",
        modifier: "(+SpA, -SpD)",
    },
    Nature {
        id: 20,
        name: "Calm",
        modifier: "(+SpD, -Atk)",
    },
    Nature {
        id: 21,
        name: "Gentle",
        modifier: "(+SpD, -Def)",
    },
    Nature {
        id: 22,
        name: "Sassy",
        modifier: "(+SpD, -Spe)",
    },
    Nature {
        id: 23,
        name: "Careful",
        modifier: "(+SpD, -SpA)",
    },
    Nature {
        id: 24,
        name: "Quirky",
        modifier: "(Neutral)",
    },
];

// ------------------------------------------------------------------- items

const MEDICINE_KEYWORDS: [&str; 36] = [
    "potion",
    "antidote",
    "burn heal",
    "ice heal",
    "awakening",
    "paralyze heal",
    "full heal",
    "max potion",
    "hyper potion",
    "super potion",
    "full restore",
    "revive",
    "max revive",
    "hp up",
    "protein",
    "iron",
    "carbos",
    "calcium",
    "zinc",
    "rare candy",
    "pp up",
    "pp max",
    "heal",
    "elixir",
    "ether",
    "energy",
    "root",
    "powder",
    "soda pop",
    "fresh water",
    "lemonade",
    "moomoo milk",
    "berry juice",
    "sacred ash",
    "lava cookie",
    "sweet heart",
];

const BATTLE_KEYWORDS: [&str; 12] = [
    "x attack",
    "x defense",
    "x speed",
    "x accuracy",
    "x sp atk",
    "x sp def",
    "x sp. atk",
    "x sp. def",
    "dire hit",
    "guard spec",
    "dire hit 2",
    "dire hit 3",
];

const EXPLICIT_KEY_ITEMS: [&str; 17] = [
    "gs ball",
    "tm case",
    "berry pouch",
    "mach bike",
    "acro bike",
    "old rod",
    "good rod",
    "super rod",
    "coin case",
    "key item",
    "town map",
    "journal",
    "explorer kit",
    "pal pad",
    "vs seeker",
    "dowsing machine",
    "poke radar",
];

/// Which bag pocket an item belongs in, as an index into the pack's pockets.
///
/// Name-based rules are applied first, then the pack's `item_pockets.json` when
/// it names something other than the catch-all pocket, then the medicine
/// keywords — the same order the original editor used.
pub fn classify_item_pocket(item_id: u32, name: &str) -> usize {
    let pack = game::current();
    let layout = pack.layout();
    let default = 0;
    let index_of = |name: &str| layout.pocket_index(name);

    let lowered = name.to_lowercase();
    let n = lowered.trim();

    // Name-based rules first, as in the original editor.
    let keyword_guess = if EXPLICIT_KEY_ITEMS.contains(&n)
        || n.contains("key item")
        || n.ends_with(" ticket")
        || n.ends_with(" pass")
        || n.ends_with(" card")
    {
        Some("KeyItems")
    } else if n.starts_with("tm") || n.starts_with("hm") {
        Some("TMsHMs")
    } else if n.ends_with("ite")
        || n.contains("mega stone")
        || n.ends_with("ite x")
        || n.ends_with("ite y")
    {
        Some("MegaStones")
    } else if n.contains("berry") {
        Some("Berries")
    } else if is_ball_name(item_id, n) {
        Some("PokeBalls")
    } else if BATTLE_KEYWORDS.iter().any(|kw| n.contains(kw)) {
        Some("BattleItems")
    } else {
        None
    };
    if let Some(index) = keyword_guess.and_then(index_of) {
        return index;
    }

    // Then the pack's own mapping, when it names something other than the
    // catch-all pocket.
    if let Some(index) = pack.item_pockets.get(&item_id).and_then(|m| index_of(m))
        && index != default
    {
        return index;
    }

    if MEDICINE_KEYWORDS.iter().any(|kw| n.contains(kw))
        && let Some(index) = index_of("Medicine")
    {
        return index;
    }
    default
}

fn is_ball_name(item_id: u32, n: &str) -> bool {
    let looks_like_ball = (1..=27).contains(&item_id)
        || n.ends_with("ball")
        || n.contains("poke ball")
        || n.contains("great ball")
        || n.contains("ultra ball")
        || n.contains("master ball");
    looks_like_ball
        && !n.contains("iron ball")
        && !n.contains("light ball")
        && !n.contains("smoke ball")
        && !n.contains("ball capsule")
        && !n.contains("ball seal")
}

pub fn get_items_for_pocket(pocket: usize) -> Vec<NamedId> {
    alphabetical_item_list()
        .into_iter()
        .filter(|item| classify_item_pocket(item.id, &item.name) == pocket)
        .collect()
}

// --------------------------------------------------------------- abilities

pub struct AbilitySlot {
    pub slot: u8,
    pub name: String,
    pub available: bool,
}

pub fn get_species_abilities(species_id: u32) -> Vec<AbilitySlot> {
    let pack = game::current();
    let raw: Vec<u32> = pack
        .species
        .get(&species_id)
        .map(|s| s.abilities.clone())
        .unwrap_or_default();

    (0..3u8)
        .map(|slot| {
            let aid = raw.get(slot as usize).copied().unwrap_or(0);
            let name = pack.abilities.get(&aid).cloned().unwrap_or_else(|| {
                if aid > 0 {
                    format!("Ability #{aid}")
                } else {
                    "None".to_string()
                }
            });
            let available = aid > 0 && name != "None";
            AbilitySlot {
                slot,
                name,
                available,
            }
        })
        .collect()
}

// ------------------------------------------------------------- species info

pub fn get_suggested_encounter_level(species_id: u32) -> u8 {
    game::current().behavior().level_for_new_pokemon(species_id)
}

pub fn get_primary_tera_type_id(species_id: u32) -> u32 {
    let pack = game::current();
    let type_name = pack
        .species
        .get(&species_id)
        .and_then(|s| s.r#type.clone())
        .unwrap_or_default();
    let behavior = pack.behavior();
    behavior
        .tera_types
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(&type_name))
        .map(|t| t.id)
        .unwrap_or_else(|| behavior.tera_types.first().map_or(0, |t| t.id))
}

/// Moves a species can learn. Form inheritance was applied when the pack was
/// loaded, so this is a straight lookup.
pub fn get_species_learnset(species_id: u32) -> Vec<String> {
    if species_id == 0 {
        return Vec::new();
    }
    game::current()
        .learnsets
        .get(&species_id)
        .cloned()
        .unwrap_or_default()
}

pub fn get_suggested_moves_for_species(species_id: u32) -> [u32; 4] {
    let learnset = get_species_learnset(species_id);
    let mut moves: Vec<u32> = Vec::new();

    for move_name in learnset {
        if let Some(move_id) = move_id_by_name(&move_name)
            && move_id > 0
            && !moves.contains(&move_id)
        {
            moves.push(move_id);
            if moves.len() >= 4 {
                break;
            }
        }
    }
    while moves.len() < 4 {
        moves.push(0);
    }
    if moves[0] == 0 {
        moves[0] = game::current().behavior().fallback_move;
    }
    [moves[0], moves[1], moves[2], moves[3]]
}

/// Moves a species can learn, as a membership set for the move pickers.
pub fn learnset_move_ids(species_id: u32) -> HashSet<u32> {
    get_species_learnset(species_id)
        .iter()
        .filter_map(|name| move_id_by_name(name))
        .collect()
}

// ----------------------------------------------------------------- sprites

const POKEAPI_BASE: &str = "https://raw.githubusercontent.com/PokeAPI/sprites/master/sprites";
const SHOWDOWN_BASE: &str = "https://play.pokemonshowdown.com/sprites";

/// Pokémon Showdown addresses sprites by a lowercase alphanumeric slug.
fn showdown_slug(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

pub fn get_item_sprite_url(item_id: u32, name: Option<&str>) -> String {
    let base = format!("{POKEAPI_BASE}/items");
    if item_id == 0 {
        return format!("{base}/poke-ball.png");
    }
    let owned;
    let item_name = match name {
        Some(n) => n,
        None => {
            owned = get_item_name(item_id);
            &owned
        }
    };
    let lowered = item_name.to_lowercase();
    let n = lowered.trim();

    if n.starts_with("hm") {
        return format!("{base}/hm-normal.png");
    }
    if n.starts_with("tm") {
        return format!("{base}/tm-normal.png");
    }

    let stripped: String = lowered
        .chars()
        .filter(|c| *c != '\'' && *c != '.' && *c != ':')
        .collect();
    let mut dashed = String::new();
    let mut prev_space = false;
    for c in stripped.chars() {
        if c.is_whitespace() {
            if !prev_space {
                dashed.push('-');
            }
            prev_space = true;
        } else {
            prev_space = false;
            dashed.push(c);
        }
    }
    let slug: String = dashed
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
        .collect();
    format!("{base}/{slug}.png")
}

/// Sprite URLs to try, in order; the first that loads is drawn.
///
/// Sources, each optional and configured per game in `game.json`:
///
/// 1. PokéAPI artwork, for ids that are National Dex numbers;
/// 2. the game's own sprites, from each species' `sprite` field;
/// 3. Pokémon Showdown's slug-addressed sprites;
/// 4. the base species' artwork, for forms nothing else covers.
pub fn sprite_candidates(species_id: u32, is_shiny: bool) -> Vec<String> {
    let pack = game::current();
    let cfg = &pack.manifest.sprites;
    let mons = format!("{POKEAPI_BASE}/pokemon");
    let mut urls: Vec<String> = Vec::new();

    let push_pokeapi = |urls: &mut Vec<String>, id: u32| {
        if is_shiny {
            urls.push(format!("{mons}/other/home/shiny/{id}.png"));
            urls.push(format!("{mons}/shiny/{id}.png"));
        } else {
            urls.push(format!("{mons}/other/home/{id}.png"));
            urls.push(format!("{mons}/{id}.png"));
        }
    };

    // PokéAPI addresses sprites by National Dex number. A game whose species
    // ids are its own numbering has to say so, or every shifted species gets a
    // real sprite of the wrong Pokémon.
    let dex = pack
        .species
        .get(&species_id)
        .and_then(|s| s.dex)
        .or_else(|| cfg.species_ids_are_dex_numbers.then_some(species_id));
    if let Some(dex) = dex
        && cfg.national_dex_max_id > 0
        && dex <= cfg.national_dex_max_id
    {
        push_pokeapi(&mut urls, dex);
    }

    let own_sprite = pack
        .species
        .get(&species_id)
        .and_then(|s| s.sprite.as_ref());
    if let (Some(base), Some(stem)) = (cfg.base_url.as_ref(), own_sprite) {
        let ext = &cfg.extension;
        if is_shiny {
            urls.push(format!("{base}/{stem}{}.{ext}", cfg.shiny_suffix));
        }
        urls.push(format!("{base}/{stem}.{ext}"));
    }

    if cfg.use_showdown_fallback
        && let Some(name) = pack.species.get(&species_id).map(|s| s.name.as_str())
    {
        let mut slugs = vec![showdown_slug(name)];
        if let Some(first) = name.split_whitespace().next() {
            let first = showdown_slug(first);
            if first != slugs[0] {
                slugs.push(first);
            }
        }
        for slug in slugs.into_iter().filter(|s| !s.is_empty()) {
            if is_shiny {
                urls.push(format!("{SHOWDOWN_BASE}/gen5-shiny/{slug}.png"));
            }
            urls.push(format!("{SHOWDOWN_BASE}/dex/{slug}.png"));
            urls.push(format!("{SHOWDOWN_BASE}/gen5/{slug}.png"));
        }
    }

    if let Some(base_id) = base_form_id(species_id) {
        push_pokeapi(&mut urls, base_id);
    }

    urls
}

/// Maps a form to the base species it belongs to — "Pikachu Cosplay" and
/// "Castform Sunny" to Pikachu and Castform — for forms the pack has no sprite
/// of its own for.
///
/// Base forms are sometimes themselves named "<species> Normal", so the match
/// is on the leading word, and the lowest id wins to keep the result stable.
fn base_form_id(species_id: u32) -> Option<u32> {
    let pack = game::current();
    let max_id = pack.manifest.sprites.national_dex_max_id;
    if max_id == 0 {
        return None;
    }
    let name = pack.species.get(&species_id).map(|s| s.name.clone())?;
    let base = name.split_whitespace().next()?;
    if base == name.as_str() {
        return None;
    }
    pack.species
        .iter()
        .filter(|(id, s)| {
            **id <= max_id && **id != species_id && s.name.split_whitespace().next() == Some(base)
        })
        .map(|(id, _)| *id)
        .min()
}
