//! The `game.json` manifest: everything about a game that is not a lookup
//! table — how its save file is laid out, where its sprites live, and the
//! handful of per-game behaviours the editor needs to know about.
//!
//! Every field has a default, so a manifest can be as short as:
//!
//! ```json
//! { "name": "Pokémon SoulGold", "version": "1.14" }
//! ```
//!
//! The defaults describe a pokeemerald-expansion save with a 96-byte party
//! record, which is what this editor was originally written against. A hack
//! that moved things around overrides just the fields that differ.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Shown in the window title and the game picker.
    pub name: String,
    #[serde(default)]
    pub version: String,
    /// Warning shown under the title, e.g. which ROM version is supported.
    #[serde(default)]
    pub notice: String,
    /// Suggested file name when exporting.
    #[serde(default)]
    pub export_name: Option<String>,
    /// Optional support/homepage link shown in the header.
    #[serde(default)]
    pub link: Option<Link>,

    #[serde(default)]
    pub layout: Layout,
    #[serde(default)]
    pub record: RecordLayout,
    #[serde(default)]
    pub sprites: SpriteConfig,
    #[serde(default)]
    pub behavior: Behavior,
}

impl Manifest {
    pub fn title(&self) -> String {
        if self.version.is_empty() {
            self.name.clone()
        } else {
            format!("{} (v{})", self.name, self.version)
        }
    }

    pub fn export_file_name(&self) -> String {
        self.export_name
            .clone()
            .unwrap_or_else(|| "edited_save.sav".to_string())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub label: String,
    pub url: String,
}

// ---------------------------------------------------------------- save layout

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Layout {
    pub sector_size: usize,
    pub sector_data_size: usize,
    pub sector_signature: u32,
    /// Sectors making up one save slot; the file holds two slots plus extras.
    pub sectors_per_slot: usize,
    /// Smallest acceptable file, in bytes.
    pub min_save_size: usize,
    /// Sectors whose checksum covers fewer bytes than `sector_data_size`.
    pub sector_sizes: Vec<SectorSize>,

    pub sb1_size: usize,
    pub sb2_size: usize,
    pub storage_size: usize,

    pub player_name: usize,
    pub player_name_length: usize,
    pub player_gender: usize,
    pub trainer_id: usize,
    /// Where the security key that money and item quantities are XOR-ed with
    /// lives in SaveBlock2. `null` for games that store those values plainly —
    /// FireRed-based hacks have no such key, and Emerald introduced it.
    pub encryption_key: Option<usize>,

    pub party_count: usize,
    pub party: usize,
    pub money: usize,
    pub coins: usize,
    pub max_money: u32,
    pub max_coins: u32,
    pub bag: usize,
    pub pockets: Vec<PocketDef>,

    pub party_size: usize,
    pub mon_size: usize,
    pub box_mon_size: usize,

    pub total_boxes: usize,
    pub box_capacity: usize,
    pub storage_sectors: (usize, usize),
    pub storage_current_box: usize,
    pub storage_boxes: usize,
    pub storage_box_names: usize,
    pub box_name_length: usize,
    pub storage_box_wallpapers: usize,

    /// How the Pokémon record is stored. `gen3_shuffled` has its field
    /// positions fixed by the format, so `record` is ignored for it.
    pub record_encoding: RecordEncoding,
    /// How boxed records are stored, when a game packs them more tightly than
    /// its party records.
    pub box_encoding: BoxEncoding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordEncoding {
    /// Substructure stored in order, unencrypted (pokeemerald-expansion hacks
    /// that removed the obfuscation, including SoulGold).
    Plain,
    /// Vanilla Gen 3: four 12-byte substructures permuted by the personality
    /// value and XOR-encrypted, as Emerald-based hacks use. See
    /// `engine::gen3`.
    Gen3Shuffled,
    /// Gen 3 field positions, but with the obfuscation switched off: the four
    /// substructures sit in G/A/E/M order, unencrypted, and the record's
    /// checksum is left alone rather than verified. CFRU-based hacks such as
    /// Unbound store records this way.
    Gen3Plain,
}

/// How a game stores *boxed* Pokémon, when that differs from its party records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BoxEncoding {
    /// Boxed records use the same encoding as the party ones.
    #[default]
    SameAsParty,
    /// Complete FireRed Upgrade's 58-byte compact record. See `engine::cfru`.
    CfruCompact,
}

impl RecordEncoding {
    /// Whether this encoding uses `engine::gen3` rather than the configurable
    /// `record` offsets.
    pub fn is_gen3(self) -> bool {
        matches!(self, Self::Gen3Shuffled | Self::Gen3Plain)
    }

    /// Gen 3 records only: whether the substructures are permuted, encrypted
    /// and checksummed.
    pub fn is_obfuscated(self) -> bool {
        matches!(self, Self::Gen3Shuffled)
    }
}

impl Default for RecordEncoding {
    fn default() -> Self {
        Self::Plain
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectorSize {
    pub sector: usize,
    pub size: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PocketDef {
    pub name: String,
    pub offset: usize,
    pub count: usize,
    /// Which part of the save file this pocket sits in.
    #[serde(default)]
    pub region: PocketRegion,
}

/// Where a bag pocket lives.
///
/// Most games keep the whole bag together in SaveBlock1. Unbound does not: its
/// main pocket is in the storage sectors and the other four are in a sector
/// outside the two save slots altogether, so a pocket has to say where it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PocketRegion {
    /// `layout.bag` + the pocket's offset, inside SaveBlock1.
    #[default]
    SaveBlock1,
    /// An offset into the assembled storage sectors.
    Storage,
    /// An absolute offset into the save file, for data outside the slots.
    File,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            sector_size: 4096,
            sector_data_size: 3968,
            sector_signature: 0x0801_2025,
            sectors_per_slot: 14,
            min_save_size: 32 * 4096,
            sector_sizes: vec![SectorSize {
                sector: 4,
                size: 3540,
            }],

            sb1_size: 15444,
            sb2_size: 2864,
            storage_size: 35712,

            player_name: 0x00,
            player_name_length: 7,
            player_gender: 0x08,
            trainer_id: 0x0A,
            encryption_key: Some(0xB4),

            party_count: 0x234,
            party: 0x238,
            money: 0x478,
            coins: 0x47C,
            max_money: 999_999,
            max_coins: 9_999,
            bag: 0x548,
            pockets: vec![
                PocketDef {
                    name: "Items".into(),
                    offset: 0,
                    count: 150,
                    region: PocketRegion::SaveBlock1,
                },
                PocketDef {
                    name: "Medicine".into(),
                    offset: 600,
                    count: 65,
                    region: PocketRegion::SaveBlock1,
                },
                PocketDef {
                    name: "KeyItems".into(),
                    offset: 860,
                    count: 50,
                    region: PocketRegion::SaveBlock1,
                },
                PocketDef {
                    name: "PokeBalls".into(),
                    offset: 1060,
                    count: 27,
                    region: PocketRegion::SaveBlock1,
                },
                PocketDef {
                    name: "TMsHMs".into(),
                    offset: 1168,
                    count: 128,
                    region: PocketRegion::SaveBlock1,
                },
                PocketDef {
                    name: "MegaStones".into(),
                    offset: 1680,
                    count: 35,
                    region: PocketRegion::SaveBlock1,
                },
                PocketDef {
                    name: "BattleItems".into(),
                    offset: 1820,
                    count: 100,
                    region: PocketRegion::SaveBlock1,
                },
                PocketDef {
                    name: "Berries".into(),
                    offset: 2220,
                    count: 70,
                    region: PocketRegion::SaveBlock1,
                },
            ],

            party_size: 6,
            mon_size: 96,
            box_mon_size: 76,

            total_boxes: 14,
            box_capacity: 30,
            storage_sectors: (5, 13),
            storage_current_box: 0x0000,
            storage_boxes: 0x0004,
            storage_box_names: 0x859C,
            box_name_length: 9,
            storage_box_wallpapers: 0x8623,

            record_encoding: RecordEncoding::Plain,
            box_encoding: BoxEncoding::default(),
        }
    }
}

impl Layout {
    /// Bytes covered by a sector's checksum.
    /// How many bytes of a sector its stored checksum covers.
    ///
    /// An explicit `sector_sizes` entry always wins, including for sector 0:
    /// the save block a sector carries and the region the game checksums are
    /// not always the same length, and only the save file can settle which.
    pub fn sector_checksum_size(&self, sector: usize) -> usize {
        if let Some(entry) = self.sector_sizes.iter().find(|s| s.sector == sector) {
            return entry.size;
        }
        if sector == 0 {
            return self.sb2_size;
        }
        self.sector_data_size
    }

    pub fn pocket(&self, index: usize) -> Option<&PocketDef> {
        self.pockets.get(index)
    }

    pub fn pocket_index(&self, name: &str) -> Option<usize> {
        self.pockets.iter().position(|p| p.name == name)
    }
}

// -------------------------------------------------------------- mon record

/// Byte offsets within one Pokémon record.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RecordLayout {
    pub personality: usize,
    pub ot_id: usize,
    pub nickname: usize,
    pub nickname_length: usize,
    /// Language in the low 3 bits, hidden nature modifier in the next 5.
    pub language: usize,
    /// Bad egg / has species / is egg flags.
    pub flags: usize,
    pub ot_name: usize,
    pub ot_name_length: usize,
    /// Word whose bit 14 is the shiny modifier.
    pub shiny_word: usize,
    /// Start of the substructure; the offsets below are relative to it.
    pub substructure: usize,

    pub species: usize,
    pub held_item: usize,
    pub experience: usize,
    pub pp_bonuses: usize,
    pub friendship: usize,
    pub moves: usize,
    pub pps: usize,
    pub evs: usize,
    pub pokerus: usize,
    pub met_location: usize,
    pub met_level: usize,
    pub ivs: usize,

    /// Battle stats, present only on party records.
    pub status: usize,
    pub level: usize,
    pub hp: usize,
    pub max_hp: usize,
    pub attack: usize,
    pub defense: usize,
    pub speed: usize,
    pub sp_attack: usize,
    pub sp_defense: usize,

    /// Gen 3 only: where the Poké Ball is kept. Vanilla packs it into four bits
    /// of the origins word, which cannot hold more than 15 balls; newer
    /// pokeemerald-expansion builds moved it to a u16 in the Growth
    /// substructure, holding the ball's item id.
    pub gen3_ball: Gen3Ball,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gen3Ball {
    /// Bits 11-14 of the origins word.
    OriginsBits,
    /// u16 at Growth+10, as an item id.
    GrowthU16,
}

impl Default for Gen3Ball {
    fn default() -> Self {
        Self::OriginsBits
    }
}

impl Default for RecordLayout {
    fn default() -> Self {
        Self {
            personality: 0,
            ot_id: 4,
            nickname: 8,
            nickname_length: 12,
            language: 20,
            flags: 21,
            ot_name: 22,
            ot_name_length: 7,
            shiny_word: 30,
            substructure: 32,

            species: 0,
            held_item: 2,
            experience: 4,
            pp_bonuses: 10,
            friendship: 11,
            moves: 12,
            pps: 20,
            evs: 24,
            pokerus: 36,
            met_location: 37,
            met_level: 38,
            ivs: 40,

            status: 76,
            level: 80,
            hp: 82,
            max_hp: 84,
            attack: 86,
            defense: 88,
            speed: 90,
            sp_attack: 92,
            sp_defense: 94,
            gen3_ball: Gen3Ball::OriginsBits,
        }
    }
}

// ----------------------------------------------------------------- sprites

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SpriteConfig {
    /// Species ids up to this value are National Dex numbers, so PokéAPI's
    /// numeric sprites line up. 0 disables that source.
    pub national_dex_max_id: u32,
    /// Base URL for the game's own sprites, joined with a species' `sprite`
    /// stem.
    pub base_url: Option<String>,
    /// Appended to the stem for shiny artwork.
    pub shiny_suffix: String,
    pub extension: String,
    /// Try Pokémon Showdown's slug-addressed sprites as a last resort.
    pub use_showdown_fallback: bool,
    /// Whether a species id may stand in for its National Dex number.
    ///
    /// True for games that number their species the same way the Dex does.
    /// Set it false for a hack with its own numbering: PokéAPI is then used
    /// only for species carrying an explicit `dex`, and the rest fall back to
    /// Showdown's sprites, which are addressed by name.
    pub species_ids_are_dex_numbers: bool,
}

impl Default for SpriteConfig {
    fn default() -> Self {
        Self {
            national_dex_max_id: 0,
            base_url: None,
            shiny_suffix: "_shiny".to_string(),
            extension: "png".to_string(),
            use_showdown_fallback: true,
            species_ids_are_dex_numbers: true,
        }
    }
}

// ---------------------------------------------------------------- behaviour

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Behavior {
    /// How close the shiny xor has to be to zero. Vanilla Gen 3 uses 8;
    /// CFRU hacks commonly widen it, and Unbound uses 16.
    pub shiny_threshold: u32,
    /// Species that never read as shiny (SoulGold's Shadow Lugia).
    pub never_shiny: Vec<u32>,
    /// Level given to a newly created Pokémon.
    pub new_level: u8,
    /// Per-species overrides for the level above.
    pub new_level_species: Vec<SpeciesLevel>,
    /// Species id ranges with their own starting level.
    pub new_level_ranges: Vec<LevelRange>,
    /// Move given to a new Pokémon when its learnset yields nothing.
    pub fallback_move: u32,
    /// Default trainer name written to created Pokémon.
    pub default_ot_name: String,
    /// Met location written to created Pokémon.
    pub met_location: u8,
    /// Learnset move names that refer to a differently named move table entry.
    pub move_aliases: Vec<MoveAlias>,
    /// Selectable Poké Balls, as (item id, label).
    pub pokeballs: Vec<IdName>,
    /// Selectable Tera types, as (type id, label). Empty hides the control.
    pub tera_types: Vec<IdName>,
    /// One-click "fill the bag" buttons shown on a pocket.
    pub quick_add: Vec<QuickAdd>,
}

/// A button offered in the bag, e.g. "Add 999 Master Balls".
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuickAdd {
    /// Pocket name it appears on, matching one in `layout.pockets`.
    pub pocket: String,
    pub item: u32,
    pub quantity: u32,
    pub label: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeciesLevel {
    pub species: u32,
    pub level: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelRange {
    pub from: u32,
    pub to: u32,
    pub level: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveAlias {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdName {
    pub id: u32,
    pub name: String,
}

impl Default for Behavior {
    fn default() -> Self {
        Self {
            shiny_threshold: 8,
            never_shiny: Vec::new(),
            new_level: 5,
            new_level_species: Vec::new(),
            new_level_ranges: Vec::new(),
            fallback_move: 33,
            default_ot_name: "Trainer".to_string(),
            met_location: 0,
            move_aliases: Vec::new(),
            pokeballs: Vec::new(),
            tera_types: Vec::new(),
            quick_add: Vec::new(),
        }
    }
}

impl Behavior {
    pub fn level_for_new_pokemon(&self, species_id: u32) -> u8 {
        if let Some(entry) = self
            .new_level_species
            .iter()
            .find(|e| e.species == species_id)
        {
            return entry.level;
        }
        if let Some(range) = self
            .new_level_ranges
            .iter()
            .find(|r| (r.from..=r.to).contains(&species_id))
        {
            return range.level;
        }
        self.new_level
    }
}
