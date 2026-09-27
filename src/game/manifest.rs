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
    pub storage_boxes: usize,
    pub storage_box_names: usize,
    pub box_name_length: usize,

    /// How the Pokémon record is stored. `gen3_shuffled` has its field
    /// positions fixed by the format, so `record` is ignored for it.
    pub record_encoding: RecordEncoding,
    /// How boxed records are stored, when a game packs them more tightly than
    /// its party records.
    pub box_encoding: BoxEncoding,
    /// Boxes kept outside the storage block, numbered after its last box.
    pub extra_boxes: Option<ExtraBoxes>,
}

/// Boxes a hack keeps outside the storage block, in whatever space it found.
///
/// SoulGold stores 15 boxes in the storage block and four more in the spare
/// ends of other sectors, some of them split across two places. Each box here
/// lists the pieces its records occupy, in order; the editor joins them into
/// one run of `box_capacity` records and splits them back out on export.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtraBoxes {
    /// Sectors outside the two save slots that these boxes use, each as the
    /// pair `[copy that goes with slot 0, copy that goes with slot 1]`. An
    /// `extra` area's `sector` is an index into this list.
    pub sectors: Vec<[usize; 2]>,
    /// How many bytes of each of those sectors its footer checksum covers.
    pub sector_checksum_sizes: Vec<usize>,
    /// Bytes a save must hold for these boxes to be there at all — a save
    /// from an older version of the game has none of them.
    #[serde(default)]
    pub markers: Vec<Marker>,
    pub boxes: Vec<ExtraBox>,
    /// Checksums the hack keeps over its own blocks.
    #[serde(default)]
    pub checksums: Vec<BlockChecksum>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtraBox {
    /// Where the box's records are, in order.
    pub records: Vec<Span>,
    /// Where its name is, `box_name_length` bytes.
    pub name: Place,
}

/// Which buffer an extra-box offset is relative to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Area {
    /// The assembled storage block.
    Storage,
    /// The assembled SaveBlock1.
    Sb1,
    /// One of `ExtraBoxes::sectors`.
    Extra,
}

/// A position in an area.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Place {
    #[serde(rename = "in")]
    pub area: Area,
    /// For `extra`: which of the sectors.
    #[serde(default)]
    pub sector: usize,
    pub offset: usize,
}

/// A run of bytes in an area.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Span {
    #[serde(rename = "in")]
    pub area: Area,
    #[serde(default)]
    pub sector: usize,
    pub offset: usize,
    pub length: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marker {
    #[serde(rename = "in")]
    pub area: Area,
    #[serde(default)]
    pub sector: usize,
    pub offset: usize,
    /// The ASCII bytes expected there.
    pub text: String,
}

/// A u32 stored at `at` over the bytes of `over`, joined in order: the sector
/// checksum's 16-bit fold in the low half and its complement in the high.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockChecksum {
    pub at: Place,
    pub over: Vec<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RecordEncoding {
    /// Substructure stored in order, unencrypted (pokeemerald-expansion hacks
    /// that removed the obfuscation, including SoulGold).
    #[default]
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
            storage_boxes: 0x0004,
            storage_box_names: 0x859C,
            box_name_length: 9,

            record_encoding: RecordEncoding::Plain,
            box_encoding: BoxEncoding::default(),
            extra_boxes: None,
        }
    }
}

impl Layout {
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

    /// Checks that every offset and size fits the structure it points into.
    ///
    /// A pack is a hand-edited JSON file, and the engine indexes save data
    /// with these numbers directly, so a typo would otherwise surface as a
    /// crash the moment a save is opened — in the browser, as a dead page.
    pub fn validate(&self, record: &RecordLayout) -> Result<(), String> {
        let mut problems: Vec<String> = Vec::new();
        let mut require = |ok: bool, problem: String| {
            if !ok {
                problems.push(problem);
            }
        };

        // The sector footer (id, checksum, signature, counter) is at 0xFF4.
        const FOOTER: usize = 0xFF4;
        require(
            self.sector_size >= 0x1000,
            format!("sector_size {} is below 4096", self.sector_size),
        );
        require(
            (1..=FOOTER).contains(&self.sector_data_size),
            format!(
                "sector_data_size {} runs into the sector footer",
                self.sector_data_size
            ),
        );
        for entry in &self.sector_sizes {
            require(
                entry.size <= FOOTER,
                format!(
                    "sector_sizes: sector {} size {} runs into the footer",
                    entry.sector, entry.size
                ),
            );
        }
        let (first, last) = self.storage_sectors;
        require(
            4 < first && first <= last && last < self.sectors_per_slot,
            format!(
                "storage_sectors ({first}, {last}) must follow sectors 0-4 and end before sector {}",
                self.sectors_per_slot
            ),
        );
        require(
            self.sb2_size <= self.sector_data_size,
            format!("sb2_size {} does not fit one sector", self.sb2_size),
        );
        require(
            self.sb1_size <= 4 * self.sector_data_size,
            format!("sb1_size {} does not fit sectors 1-4", self.sb1_size),
        );
        let storage_sectors = (last + 1).saturating_sub(first);
        require(
            self.storage_size <= storage_sectors * self.sector_data_size,
            format!(
                "storage_size {} does not fit the storage sectors",
                self.storage_size
            ),
        );

        require(
            self.player_name + self.player_name_length < self.sb2_size
                && self.player_gender < self.sb2_size,
            "the player name or gender lies outside sb2".to_string(),
        );
        require(
            self.party + self.party_size * self.mon_size <= self.sb1_size,
            "the party does not fit in sb1".to_string(),
        );
        require(
            self.storage_boxes + self.total_boxes * self.box_capacity * self.box_mon_size
                <= self.storage_size,
            "the boxes do not fit in storage".to_string(),
        );
        require(
            self.box_mon_size <= self.mon_size,
            format!(
                "box_mon_size {} is larger than mon_size {}",
                self.box_mon_size, self.mon_size
            ),
        );

        let (party_min, box_min) = match (self.record_encoding.is_gen3(), self.box_encoding) {
            (true, BoxEncoding::CfruCompact) => (100, 58),
            (true, BoxEncoding::SameAsParty) => (100, 80),
            (false, _) => (record.party_end(), record.box_end()),
        };
        require(
            self.mon_size >= party_min && self.box_mon_size >= box_min,
            format!(
                "records need mon_size >= {party_min} and box_mon_size >= {box_min}, not {} and {}",
                self.mon_size, self.box_mon_size
            ),
        );

        if let Some(extra) = &self.extra_boxes {
            self.validate_extra_boxes(extra, &mut problems);
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "{} has problems:\n  {}",
                super::MANIFEST_NAME,
                problems.join("\n  ")
            ))
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Gen3Ball {
    /// Bits 11-14 of the origins word.
    #[default]
    OriginsBits,
    /// u16 at Growth+10, as an item id.
    GrowthU16,
}

impl Layout {
    /// The bytes an area holds, for range checks.
    fn area_len(&self, area: Area) -> usize {
        match area {
            Area::Storage => self.storage_size,
            Area::Sb1 => self.sb1_size,
            // Everything before the sector footer.
            Area::Extra => 0xFF4,
        }
    }

    fn validate_extra_boxes(&self, extra: &ExtraBoxes, problems: &mut Vec<String>) {
        let mut require = |ok: bool, problem: String| {
            if !ok {
                problems.push(format!("extra_boxes: {problem}"));
            }
        };
        let slots_end = 2 * self.sectors_per_slot;
        let file_sectors = self.min_save_size / self.sector_size.max(1);
        for pair in &extra.sectors {
            require(
                pair.iter().all(|s| (slots_end..file_sectors).contains(s)) && pair[0] != pair[1],
                format!("sectors {pair:?} must be two different sectors after the save slots"),
            );
        }
        require(
            extra.sector_checksum_sizes.len() == extra.sectors.len()
                && extra.sector_checksum_sizes.iter().all(|n| *n <= 0xFF4),
            "sector_checksum_sizes needs one size, up to 4084, per sector".to_string(),
        );

        // Inside its area, and in storage past the last regular box so an
        // extra box can never overlap one.
        let boxes_end =
            self.storage_boxes + self.total_boxes * self.box_capacity * self.box_mon_size;
        let fits = |area: Area, sector: usize, offset: usize, length: usize| {
            offset + length <= self.area_len(area)
                && (area != Area::Extra || sector < extra.sectors.len())
                && (area != Area::Storage || offset >= boxes_end)
        };
        let per_box = self.box_capacity * self.box_mon_size;
        for (i, b) in extra.boxes.iter().enumerate() {
            let number = self.total_boxes + i + 1;
            for r in &b.records {
                require(
                    fits(r.area, r.sector, r.offset, r.length),
                    format!(
                        "box {number}'s records at {:?}+{:#x} do not fit",
                        r.area, r.offset
                    ),
                );
            }
            let n = &b.name;
            require(
                fits(n.area, n.sector, n.offset, self.box_name_length),
                format!(
                    "box {number}'s name at {:?}+{:#x} does not fit",
                    n.area, n.offset
                ),
            );
            let total: usize = b.records.iter().map(|s| s.length).sum();
            require(
                total == per_box,
                format!("box {number}'s records add up to {total} bytes, not {per_box}"),
            );
        }
        for m in &extra.markers {
            require(
                fits(m.area, m.sector, m.offset, m.text.len()),
                format!("marker at {:?}+{:#x} does not fit", m.area, m.offset),
            );
        }
        for c in &extra.checksums {
            let spans_fit = c
                .over
                .iter()
                .all(|s| fits(s.area, s.sector, s.offset, s.length));
            require(
                fits(c.at.area, c.at.sector, c.at.offset, 4) && spans_fit,
                format!(
                    "checksum at {:?}+{:#x} or what it covers does not fit",
                    c.at.area, c.at.offset
                ),
            );
        }
    }
}

impl RecordLayout {
    /// One past the last byte a boxed record's fields use.
    fn box_end(&self) -> usize {
        let sub = self.substructure;
        [
            self.personality + 4,
            self.ot_id + 4,
            self.nickname + self.nickname_length,
            self.language + 1,
            self.flags + 1,
            self.ot_name + self.ot_name_length,
            self.shiny_word + 2,
            sub + self.species + 2,
            sub + self.held_item + 2,
            sub + self.experience + 4,
            sub + self.pp_bonuses + 1,
            sub + self.friendship + 1,
            sub + self.moves + 8,
            sub + self.pps + 4,
            sub + self.evs + 6,
            sub + self.pokerus + 1,
            sub + self.met_location + 1,
            sub + self.met_level + 2,
            sub + self.ivs + 4,
        ]
        .into_iter()
        .max()
        .unwrap_or(0)
    }

    /// The same for a party record, which adds the battle stats.
    fn party_end(&self) -> usize {
        [
            self.box_end(),
            self.status + 4,
            self.level + 2,
            self.hp + 2,
            self.max_hp + 2,
            self.attack + 2,
            self.defense + 2,
            self.speed + 2,
            self.sp_attack + 2,
            self.sp_defense + 2,
        ]
        .into_iter()
        .max()
        .unwrap_or(0)
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
