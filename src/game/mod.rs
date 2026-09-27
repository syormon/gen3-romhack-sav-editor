//! Game packs: everything the editor knows about one ROM hack, loaded from a
//! folder at runtime rather than compiled in.
//!
//! A pack is a directory containing `game.json` plus the data tables:
//!
//! ```text
//! assets/soulgold/
//!   game.json          manifest: save layout, sprites, behaviour
//!   species.json       everything per species, keyed by internal id:
//!                      { "1": { "name": "Bulbasaur", "type": "Grass",
//!                               "growth": "medium-slow",
//!                               "abilities": [65, 0, 34],
//!                               "sprite": "bulbasaur",
//!                               "stats": { "hp": 45, ... },
//!                               "learnset": ["Tackle", ...] } }
//!   moves.json         { "1": { "name": "Pound", "pp": 35 }, ... }
//!   items.json         { "1": "Poke Ball", ... }
//!   item_pockets.json  { "1": "PokeBalls", ... }
//!   abilities.json     { "1": "Stench", ... }
//!   charmap.json       { "187": "A", ... }        (optional)
//! ```
//!
//! Only `game.json` and `species.json` are required, and within a species only
//! `name` is; everything else degrades, so a partially-mapped hack still opens.
//!
//! A species with no `learnset` of its own inherits the one belonging to the
//! species sharing its leading word, so forms ("Venusaur Mega", "Floette Red")
//! need not repeat their base form's moves.

pub mod manifest;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use serde::Deserialize;

// `Layout` exposes these types in its public fields, so they belong in this
// module's surface even when the binary itself never names them.
#[allow(unused_imports)]
pub use manifest::{
    Area, Behavior, BoxEncoding, ExtraBoxes, Gen3Ball, Layout, Manifest, PocketDef, PocketRegion,
    RecordEncoding, RecordLayout, Span,
};

pub const MANIFEST_NAME: &str = "game.json";

/// One species, as `species.json` describes it. Only `name` is required, and
/// fields the editor has no use for (a hack's innate abilities, say) are
/// ignored rather than refused.
#[derive(Debug, Clone, Deserialize)]
pub struct Species {
    pub name: String,
    /// Primary type, used to pick a default tera type.
    #[serde(default)]
    pub r#type: Option<String>,
    /// Experience curve: fast, medium, medium-slow, slow, slow-then-very-fast,
    /// fast-then-very-slow.
    #[serde(default)]
    pub growth: Option<String>,
    /// Ability ids: slot 1, slot 2, hidden.
    #[serde(default)]
    pub abilities: Vec<u32>,
    /// This species' National Dex number, when it has one.
    ///
    /// Only needed by games whose internal species ids are their own
    /// numbering. Sprites are addressed by National Dex number, so without this
    /// a hack that shifts its ids shows the wrong Pokémon — and shows it
    /// confidently, because the wrong number is still a valid one.
    #[serde(default)]
    pub dex: Option<u32>,
    /// Sprite file stem, joined with `sprites.base_url` from the manifest.
    #[serde(default)]
    pub sprite: Option<String>,
    /// Base stats, which a party member's battle stats are computed from.
    #[serde(default)]
    pub stats: Option<BaseStats>,
    /// Moves this species can learn. Empty means "inherit from the base form".
    #[serde(default)]
    pub learnset: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct BaseStats {
    #[serde(default)]
    pub hp: u32,
    #[serde(default)]
    pub attack: u32,
    #[serde(default)]
    pub defense: u32,
    #[serde(default)]
    pub speed: u32,
    #[serde(default)]
    pub sp_attack: u32,
    #[serde(default)]
    pub sp_defense: u32,
}

/// One move, as `moves.json` describes it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveData {
    name: String,
    /// Base PP. Missing or 0 reads as 20.
    #[serde(default)]
    pp: u32,
}

#[derive(Clone, Debug)]
pub struct NamedId {
    pub id: u32,
    pub name: String,
}

/// One game's data, ready to use.
pub struct GamePack {
    pub dir: PathBuf,
    pub manifest: Manifest,

    pub species: HashMap<u32, Species>,
    pub moves: HashMap<u32, String>,
    pub move_pps: HashMap<u32, u32>,
    pub items: HashMap<u32, String>,
    pub item_pockets: HashMap<u32, String>,
    pub abilities: HashMap<u32, String>,
    /// Learnsets after form inheritance, so a lookup is one map hit.
    pub learnsets: HashMap<u32, Vec<String>>,
    /// Byte -> character. Empty means "use the built-in Latin table".
    pub charmap: HashMap<u8, char>,

    // Derived once at load time.
    pub species_sorted: Vec<NamedId>,
    pub items_alphabetical: Vec<NamedId>,
    pub moves_alphabetical: Vec<NamedId>,
    pub move_name_to_id: HashMap<String, u32>,
}

// ------------------------------------------------------------- pack files
//
// A pack is read from a folder on disk, or from the copy of `assets/` that
// build.rs compiles into the binary. The browser build has no disk, so the
// built-in copy is all it has; natively it means a release works even without
// its `assets` folder, while a folder on disk still takes precedence so packs
// can be edited and added without rebuilding.

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded_packs.rs"));
}

/// Stands in for a directory when a pack comes from the binary itself.
const BUILT_IN: &str = "<built-in>";

fn built_in_dir(folder: &str) -> PathBuf {
    Path::new(BUILT_IN).join(folder)
}

/// The built-in pack a path refers to, if it is one.
fn built_in_files(dir: &Path) -> Option<&'static [(&'static str, &'static [u8])]> {
    if dir.parent()? != Path::new(BUILT_IN) {
        return None;
    }
    let folder = dir.file_name()?.to_str()?;
    embedded::PACKS
        .iter()
        .find(|(name, _)| *name == folder)
        .map(|(_, files)| *files)
}

/// Reads one file of a pack, wherever the pack lives. `None` if it has no such
/// file.
pub fn read_pack_file(dir: &Path, file: &str) -> Option<Vec<u8>> {
    if let Some(files) = built_in_files(dir) {
        return files
            .iter()
            .find(|(name, _)| *name == file)
            .map(|(_, bytes)| bytes.to_vec());
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::read(dir.join(file)).ok()
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

fn read_pack_text(dir: &Path, file: &str) -> Option<Result<String, String>> {
    read_pack_file(dir, file)
        .map(|bytes| String::from_utf8(bytes).map_err(|e| format!("{file}: not UTF-8 ({e})")))
}

fn read_map<T: for<'de> Deserialize<'de>>(
    dir: &Path,
    file: &str,
) -> Result<HashMap<u32, T>, String> {
    let raw = match read_pack_text(dir, file) {
        Some(raw) => raw?,
        None => return Ok(HashMap::new()), // optional table
    };
    let parsed: HashMap<String, T> =
        serde_json::from_str(&raw).map_err(|e| format!("{file}: {e}"))?;
    Ok(parsed
        .into_iter()
        .filter_map(|(k, v)| k.parse::<u32>().ok().map(|k| (k, v)))
        .collect())
}

/// Folds a move name to a comparison key: lowercase, letters and digits only.
///
/// Learnset tables tend to drop punctuation ("Double Edge", "Kings Shield")
/// while move tables keep the game's spelling ("Double-Edge", "King's Shield"),
/// so neither side is matched literally.
pub fn move_key(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Normalised species name: lowercase, letters and digits only, with the
/// gender symbols spelled out the way learnset exports tend to.
fn species_key(name: &str) -> String {
    let expanded = name.replace('\u{2640}', "-f").replace('\u{2642}', "-m");
    expanded
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Gives every species its learnset, letting forms inherit from the species
/// that shares their leading word ("Venusaur Mega" from "Venusaur").
fn resolve_learnsets(species: &HashMap<u32, Species>) -> HashMap<u32, Vec<String>> {
    let mut by_name: HashMap<String, u32> = HashMap::new();
    let mut by_first_word: HashMap<String, u32> = HashMap::new();
    for (id, s) in species.iter().filter(|(_, s)| !s.learnset.is_empty()) {
        // Lowest id wins, so the result does not depend on map order.
        let name_key = species_key(&s.name);
        by_name
            .entry(name_key)
            .and_modify(|e| *e = (*e).min(*id))
            .or_insert(*id);
        if let Some(first) = s.name.split_whitespace().next() {
            by_first_word
                .entry(species_key(first))
                .and_modify(|e| *e = (*e).min(*id))
                .or_insert(*id);
        }
    }

    species
        .iter()
        .map(|(id, s)| {
            if !s.learnset.is_empty() {
                return (*id, s.learnset.clone());
            }
            let owner = by_name.get(&species_key(&s.name)).or_else(|| {
                s.name
                    .split_whitespace()
                    .next()
                    .and_then(|first| by_first_word.get(&species_key(first)))
            });
            let inherited = owner
                .and_then(|owner| species.get(owner))
                .map(|owner| owner.learnset.clone())
                .unwrap_or_default();
            (*id, inherited)
        })
        .collect()
}

fn by_name_then_id(a: &NamedId, b: &NamedId) -> std::cmp::Ordering {
    a.name
        .to_lowercase()
        .cmp(&b.name.to_lowercase())
        .then(a.id.cmp(&b.id))
}

impl GamePack {
    pub fn load(dir: impl AsRef<Path>) -> Result<Self, String> {
        let dir = dir.as_ref().to_path_buf();

        let raw = read_pack_text(&dir, MANIFEST_NAME)
            .ok_or_else(|| format!("{}: no {MANIFEST_NAME}", dir.display()))??;
        let manifest: Manifest =
            serde_json::from_str(&raw).map_err(|e| format!("{MANIFEST_NAME}: {e}"))?;
        manifest.layout.validate(&manifest.record)?;

        let species: HashMap<u32, Species> = read_map(&dir, "species.json")?;
        if species.is_empty() {
            return Err(format!(
                "{}: species.json is missing or empty",
                dir.display()
            ));
        }

        let move_data: HashMap<u32, MoveData> = read_map(&dir, "moves.json")?;
        let move_pps: HashMap<u32, u32> = move_data.iter().map(|(id, m)| (*id, m.pp)).collect();
        let moves: HashMap<u32, String> =
            move_data.into_iter().map(|(id, m)| (id, m.name)).collect();
        let items: HashMap<u32, String> = read_map(&dir, "items.json")?;
        let item_pockets: HashMap<u32, String> = read_map(&dir, "item_pockets.json")?;
        let abilities: HashMap<u32, String> = read_map(&dir, "abilities.json")?;
        let learnsets = resolve_learnsets(&species);

        let charmap_raw: HashMap<u32, String> = read_map(&dir, "charmap.json")?;
        let charmap: HashMap<u8, char> = charmap_raw
            .into_iter()
            .filter_map(|(code, text)| {
                let c = text.chars().next()?;
                u8::try_from(code).ok().map(|code| (code, c))
            })
            .collect();

        let mut species_sorted: Vec<NamedId> = species
            .iter()
            .filter(|(id, _)| **id > 0)
            .map(|(id, s)| NamedId {
                id: *id,
                name: s.name.clone(),
            })
            .collect();
        species_sorted.sort_by_key(|s| s.id);

        let mut items_alphabetical: Vec<NamedId> = items
            .iter()
            .filter(|(id, _)| **id > 0)
            .map(|(id, name)| NamedId {
                id: *id,
                name: name.clone(),
            })
            .collect();
        items_alphabetical.sort_by(by_name_then_id);

        let mut moves_alphabetical: Vec<NamedId> = moves
            .iter()
            .filter(|(id, _)| **id > 0)
            .map(|(id, name)| NamedId {
                id: *id,
                name: name.clone(),
            })
            .collect();
        moves_alphabetical.sort_by(by_name_then_id);

        let mut move_name_to_id: HashMap<String, u32> = moves
            .iter()
            .map(|(id, name)| (move_key(name), *id))
            .collect();
        for alias in &manifest.behavior.move_aliases {
            if let Some(id) = move_name_to_id.get(&move_key(&alias.to)).copied() {
                move_name_to_id.insert(move_key(&alias.from), id);
            }
        }

        Ok(Self {
            dir,
            manifest,
            species,
            moves,
            move_pps,
            items,
            item_pockets,
            abilities,
            learnsets,
            charmap,
            species_sorted,
            items_alphabetical,
            moves_alphabetical,
            move_name_to_id,
        })
    }

    pub fn layout(&self) -> &Layout {
        &self.manifest.layout
    }

    pub fn record(&self) -> &RecordLayout {
        &self.manifest.record
    }

    pub fn behavior(&self) -> &Behavior {
        &self.manifest.behavior
    }

    /// Problems worth showing the user, without refusing to load the pack.
    pub fn warnings(&self) -> Vec<String> {
        let mut out = Vec::new();
        let missing = |table: &str, empty: bool| -> Option<String> {
            empty.then(|| format!("{table} is missing or empty"))
        };
        out.extend(missing("moves.json", self.moves.is_empty()));
        out.extend(missing("items.json", self.items.is_empty()));
        out.extend(missing("abilities.json", self.abilities.is_empty()));
        out.extend(missing(
            "growth rates on every species",
            self.species.values().all(|s| s.growth.is_none()),
        ));

        let unresolved = self
            .learnsets
            .values()
            .flatten()
            .filter(|name| !self.move_name_to_id.contains_key(&move_key(name)))
            .count();
        if unresolved > 0 {
            out.push(format!(
                "{unresolved} learnset entries do not match any move (check move_aliases)"
            ));
        }
        out
    }
}

// ------------------------------------------------------------------ discovery

#[derive(Debug, Clone)]
pub struct PackInfo {
    pub dir: PathBuf,
    pub name: String,
    pub version: String,
}

impl PackInfo {
    /// The folder under `assets/`, which is what `--game` accepts alongside
    /// the display name.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))] // `--game` is native-only
    pub fn folder_name(&self) -> String {
        self.dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }

    /// "Pokémon SoulGold (v1.14)", for menus and buttons.
    pub fn label(&self) -> String {
        if self.version.is_empty() {
            self.name.clone()
        } else {
            format!("{} (v{})", self.name, self.version)
        }
    }
}

/// Finds a pack by folder name or display name, ignoring case.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // `--game` is native-only
pub fn find<'a>(packs: &'a [PackInfo], wanted: &str) -> Option<&'a PackInfo> {
    let wanted = wanted.trim().to_lowercase();
    packs
        .iter()
        .find(|p| p.folder_name().to_lowercase() == wanted)
        .or_else(|| packs.iter().find(|p| p.name.to_lowercase() == wanted))
}

/// Directories searched for `<pack>/game.json`, nearest first.
#[cfg(not(target_arch = "wasm32"))]
pub fn search_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut push = |p: PathBuf| {
        if !roots.contains(&p) {
            roots.push(p);
        }
    };

    if let Ok(exe) = std::env::current_exe() {
        // Next to the binary, and a couple of levels up for `target/debug`.
        let mut dir = exe.parent().map(Path::to_path_buf);
        for _ in 0..4 {
            let Some(d) = dir else { break };
            push(d.join("assets"));
            dir = d.parent().map(Path::to_path_buf);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        push(cwd.join("assets"));
    }
    push(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"));
    roots
}

fn pack_info(dir: PathBuf) -> PackInfo {
    let (name, version) = match read_pack_text(&dir, MANIFEST_NAME)
        .and_then(Result::ok)
        .and_then(|raw| serde_json::from_str::<Manifest>(&raw).ok())
    {
        Some(m) => (m.name, m.version),
        None => (
            dir.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            String::new(),
        ),
    };
    PackInfo { dir, name, version }
}

/// Every pack available: folders on the search path first, nearest root
/// winning, then the built-in ones any folder did not already provide.
pub fn discover() -> Vec<PackInfo> {
    let mut found = discover_on_disk();
    for (folder, _) in embedded::PACKS {
        let info = pack_info(built_in_dir(folder));
        if !found.iter().any(|p| p.name == info.name) {
            found.push(info);
        }
    }
    found.sort_by_key(|p| p.name.to_lowercase());
    found
}

#[cfg(target_arch = "wasm32")]
fn discover_on_disk() -> Vec<PackInfo> {
    Vec::new()
}

#[cfg(not(target_arch = "wasm32"))]
fn discover_on_disk() -> Vec<PackInfo> {
    let mut found: Vec<PackInfo> = Vec::new();

    for root in search_roots() {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.join(MANIFEST_NAME).is_file() {
                continue;
            }
            let info = pack_info(dir);
            if found.iter().any(|p| p.name == info.name) {
                continue; // a nearer root already provided this game
            }
            found.push(info);
        }
    }
    found
}

// -------------------------------------------------------------- current game

static CURRENT: RwLock<Option<Arc<GamePack>>> = RwLock::new(None);

/// The pack in use. Panics if nothing has been loaded — the app loads one
/// before any UI or engine code runs.
pub fn current() -> Arc<GamePack> {
    CURRENT
        .read()
        .expect("game pack lock")
        .clone()
        .expect("no game pack loaded")
}

/// The pack in use, if one has been chosen. UI drawn before that point must
/// use this rather than `current()`.
pub fn current_opt() -> Option<Arc<GamePack>> {
    CURRENT.read().expect("game pack lock").clone()
}

pub fn is_loaded() -> bool {
    CURRENT.read().expect("game pack lock").is_some()
}

/// Forgets the active game, putting the editor back to knowing nothing about
/// how to read a save.
pub fn clear() {
    *CURRENT.write().expect("game pack lock") = None;
}

pub fn set_current(pack: Arc<GamePack>) {
    *CURRENT.write().expect("game pack lock") = Some(pack);
}

/// Loads a pack from a directory and makes it current.
pub fn activate(dir: impl AsRef<Path>) -> Result<Arc<GamePack>, String> {
    let pack = Arc::new(GamePack::load(dir)?);
    set_current(pack.clone());
    Ok(pack)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The browser has nothing but the built-in packs, so if one of them fails
    /// to load the web build has no such game — and nothing natively would
    /// notice, because a folder on disk shadows it there.
    #[test]
    fn every_built_in_pack_loads_and_matches_its_folder() {
        assert!(!embedded::PACKS.is_empty(), "build.rs embedded no packs");

        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        for (folder, files) in embedded::PACKS {
            let built_in = GamePack::load(built_in_dir(folder))
                .unwrap_or_else(|e| panic!("built-in {folder} failed to load: {e}"));
            let on_disk = GamePack::load(assets.join(folder)).expect("disk copy loads");

            assert_eq!(built_in.manifest.name, on_disk.manifest.name, "{folder}");
            assert_eq!(built_in.species.len(), on_disk.species.len(), "{folder}");
            assert_eq!(built_in.moves.len(), on_disk.moves.len(), "{folder}");
            assert_eq!(built_in.items.len(), on_disk.items.len(), "{folder}");

            // Byte-for-byte, so a stale build cannot pass for a current one.
            for (name, bytes) in *files {
                let disk = std::fs::read(assets.join(folder).join(name)).expect("disk file");
                assert_eq!(*bytes, disk.as_slice(), "{folder}/{name} is out of date");
            }
        }
    }

    #[test]
    fn a_built_in_path_is_never_looked_for_on_disk() {
        let dir = built_in_dir("no-such-pack");
        assert!(read_pack_file(&dir, MANIFEST_NAME).is_none());
        assert!(GamePack::load(&dir).is_err());
    }

    /// A folder on disk must win over the built-in copy of the same game, or
    /// editing a pack's JSON would silently do nothing.
    #[test]
    fn a_folder_on_disk_takes_precedence_over_the_built_in_copy() {
        let packs = discover();
        for (folder, _) in embedded::PACKS {
            let name = pack_info(built_in_dir(folder)).name;
            let hits: Vec<_> = packs.iter().filter(|p| p.name == name).collect();
            assert_eq!(hits.len(), 1, "{name} listed {} times", hits.len());
            assert!(
                built_in_files(&hits[0].dir).is_none(),
                "{name} should come from assets/, not the binary"
            );
        }
    }
}
