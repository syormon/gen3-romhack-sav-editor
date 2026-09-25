# Pokémon ROM hack save editor

A native Rust + [egui](https://github.com/emilk/egui) save editor for Pokémon ROM hacks.

The program knows nothing about any particular game. Each game is a folder of JSON under
`assets/`, and the editor discovers whatever is there at startup. Adding a hack means
adding data, not changing code.

⚠️ **Always keep a backup** before overwriting a save.

It began as a port of [`soul-gold-web`](https://github.com/jozeton-app/soul-gold-web) by
Jozeton. Two games ship with it:

| Game | Base | Record format | State |
| --- | --- | --- | --- |
| Pokémon SoulGold | pokeemerald-expansion | `plain` | complete |
| Pokémon Emerald Seaglass | pokeemerald-expansion (Emerald save format) | `gen3_shuffled` | complete |
| Pokémon Unbound | CFRU / FireRed | `gen3_plain` + `cfru_compact` boxes | complete |

Unbound's tables come from [PUSE](https://github.com/Zannael/PUSE)'s ROM extraction: 1267
species with stats, types, growth curves, abilities and learnsets, 922 moves with PP, 728
items and 254 abilities. Its save layout was worked out from the save file itself, using
the sector checksums as the oracle — a stored checksum only reproduces under the right
data length, so trying every length says what the game actually does instead of guessing:

```
sector data   0xFF0, not vanilla's 0xF80   (sector 1's checksum needs 0xFEC bytes)
SaveBlock1    3 x 0xFF0 + 0xD98 = 0x3D68   (FireRed's SaveBlock1 size, exactly)
storage       8 x 0xFF0 + 0x450 = 0x83D0   (= 4 + 14*30*80 + 14*9 + 14, exactly)
signature     0x01121999, not Emerald's 0x08012025
```

Its records are Gen 3 in layout but **not obfuscated**: substructures sit in plain G/A/E/M
order, unencrypted, and the record checksum is left at zero. Reading the party as vanilla
Gen 3 produced junk; reading it plainly produced a Lv.11 Gible knowing Tackle, Sand Attack
and Metal Claw with 807 experience — which is Lv.11 on the medium-slow curve, matching the
battle stats stored beside it. That is what `"record_encoding": "gen3_plain"` selects.

Its **boxes are not Gen 3 records at all**. CFRU shrinks them to **58 bytes** and packs the
fields by hand, squeezing the four move ids into five bytes as 10-bit fields:

```
0x1C species   0x24 PP bonuses   0x2C EVs (6)
0x1E held item 0x25 friendship   0x32 met data, not modelled
0x20 exp       0x26 ball         0x36 IV word
               0x27 moves: 4 x 10 bits in 5 bytes
```

That was derived from the save rather than from documentation. The 58-byte stride is the
spacing between occurrences of the trainer's OT id (330 consecutive gaps agree); the move
offset is the only one of 23 candidates whose four decoded ids are moves the species can
actually learn — **91.8%** against learnsets, where every other offset scores under 17%.
The results speak for themselves: Gastly with Shadow Ball / Sludge Bomb / Psychic /
Destiny Bond, Pawniard with Sucker Punch / Iron Head / Knock Off / Stealth Rock.

Unlike the Gen 3 record there is no checksum to tell a used slot from noise, so the games'
own occupancy flag does it. Skipping that check decoded 28 slots of leftover bytes into
convincing Pokémon.

Its bag — Unbound calls it the **Cube** — is not in SaveBlock1 where every other Gen 3 game
keeps it, and it is not in one piece:

```
Items      326 slots   storage sectors, sector 13 + 0xAD8
KeyItems    75 slots   sector 30 + 0x1F0
PokeBalls   50 slots   sector 30 + 0x31C
TMsHMs     128 slots   sector 30 + 0x3E4
Berries     75 slots   sector 30 + 0x5E4
```

Sector 30 sits outside both save slots, is not slot-rotated and carries no save signature,
which is why PUSE's anchors there looked save-specific — `0x1E31C` is simply sector 30 plus
`0x31C`. A pocket therefore declares which part of the file it lives in (`region`:
`save_block_1`, `storage` or `file`), and pockets in unsigned sectors are left strictly
alone otherwise: the editor no longer stamps a save counter into a sector the game does not
mark as one.

**Box names** are the one thing still not located; the UI shows "Box N".

## Running

```bash
cargo run --release
```

Drop a `.sav` on the window or click **Browse…**. The editor then asks which game wrote
it, and applies the answer live — the save opens behind the dialog as you change the
dropdown, so a wrong answer is usually obvious before you commit to it:

![choosing the game for a loaded save](docs/picker.png)

It has to ask. A save file carries no marker of the game that wrote it — every game here
uses a 128 KB container of otherwise indistinguishable bytes — and reading one with
another game's layout does not fail. It finds Pokémon at the wrong offsets and produces a
boxful of plausible rubbish: `??Q?Blaziken` at Lv.197 holding `Species #2047`, and a
trainer suspiciously flush at $999,999. Guessing silently was the old behaviour, and it
was worse than asking.

The question is skipped when there is nothing to ask — a single game installed, or one
named on the command line:

```bash
cargo run --release                                     # load a save, then answer
cargo run --release -- /path/to/save.sav                # same, with the save already read
cargo run --release -- --game soulgold save.sav         # straight in, no question
```

`--game` takes either the folder under `assets/` or the game's display name.

**Load Different .sav** asks again, because a new file is exactly the moment the answer can
change — nothing ties the next save to the game the last one came from. The current game is
pre-selected, so staying put is one click, and Cancel puts back the save you had open. The
header's game menu still switches games and re-reads the *open* file under the new layout,
which is the fix for having answered wrong.

```bash
cargo test                                    # engine round-trip tests
cargo run --example validate_game             # check every installed game pack
cargo run --example names -- save.sav         # nickname report for a save
cargo run --example dump  -- save.sav out.txt out.sav   # dump everything the parser reads
cargo run --example roundtrip -- seaglass save.sav      # write every record back untouched
```

Examples take `--game <folder|name>` too, and default to the first game found.

## What it edits

Party and boxes with drag-and-drop between any two slots, creating and releasing Pokémon,
the full inspector (species, ability, nature, tera type, level, ball, held item, four moves
with PP and PP Ups, IVs and EVs, shiny toggle), the trainer and wallet, and every bag
pocket with per-pocket search and quantity editing.

## Adding a game

Create `assets/<yourgame>/` containing `game.json` and the tables below. Only
`game.json` and `species.json` are required — anything missing degrades to an empty table,
so a partially mapped hack still opens. Run `cargo run --example validate_game` afterwards;
it reports what is missing and checks the save layout for self-consistency.

| File | Shape | Notes |
| --- | --- | --- |
| `game.json` | manifest | see below |
| `species.json` | one entry per species | **required**; see below |
| `moves.json` | `{"1": "Pound"}` | |
| `move_pps.json` | `{"1": 35}` | missing entries report 20 PP |
| `items.json` | `{"1": "Poke Ball"}` | |
| `item_pockets.json` | `{"1": "PokeBalls"}` | values match pocket names in the manifest |
| `abilities.json` | `{"1": "Stench"}` | |
| `charmap.json` | `{"187": "A"}` | optional; overrides the built-in text encoding |
| species `dex` | `{"496": {"dex": 443}}` | National Dex number, for games that renumber species |
| `icon.png` | image | optional window icon |

### species.json

Everything per species lives in one table, keyed by the game's internal species
id. Only `name` is required:

```json
{
  "1": {
    "name": "Bulbasaur",
    "type": "Grass",
    "growth": "medium-slow",
    "abilities": [65, 0, 34],
    "innates": [269],
    "sprite": "bulbasaur",
    "stats": { "hp": 45, "attack": 49, "defense": 49, "speed": 45, "sp_attack": 65, "sp_defense": 65 },
    "learnset": ["Tackle", "Growl", "Vine Whip"]
  }
}
```

| Field | Meaning |
| --- | --- |
| `name` | shown everywhere, and written into the save for un-nicknamed Pokémon, so it must match the game's spelling exactly |
| `type` | primary type; picks the default tera type |
| `growth` | `fast`, `medium`, `medium-slow`, `slow`, `slow-then-very-fast`, `fast-then-very-slow` — this is what turns a boxed Pokémon's experience into a level |
| `abilities` | ability ids: slot 1, slot 2, hidden |
| `innates` | carried through for hacks that have them |
| `sprite` | file stem joined with `sprites.base_url` |
| `stats` | base stats, carried through |
| `learnset` | move names; omit it and the species inherits from the one sharing its leading word, so `"Venusaur Mega"` picks up `"Venusaur"`'s moves |

### The manifest

Every field except `name` has a default, so this is a valid `game.json`:

```json
{ "name": "My Hack", "version": "1.0" }
```

The defaults describe a pokeemerald-expansion save with a 96-byte party record — what
SoulGold uses. A hack that moved things overrides only what differs. The full field list
with its defaults is in [`src/game/manifest.rs`](src/game/manifest.rs); the interesting
sections are:

* **`layout`** — sector geometry, save block sizes, the offsets of the party, money, coins
  and bag, box counts, and the bag pockets (`{ "name", "offset", "count" }` each). Pocket
  names are what `item_pockets.json` refers to, and `KeyItems` and `MegaStones` are treated
  as holding one of each.
* **`record`** — byte offsets inside one Pokémon record: nickname (and its width), OT name,
  the substructure, and where species, moves, EVs, IVs and the battle stats sit inside it.
* **`sprites`** — `national_dex_max_id` (ids up to it are National Dex numbers, so PokéAPI
  artwork is used), `base_url` for the game's own sprites, and whether to fall back to
  Pokémon Showdown's slug-addressed sprites.
* **`behavior`** — never-shiny species, the level and met location given to a created
  Pokémon, the fallback move, selectable Poké Balls and tera types, `move_aliases` for
  learnsets that spell a move differently from the move table, and `quick_add` buttons for
  the bag ("Add 999 Master Balls").

### Record encodings

`layout.record_encoding` picks how a Pokémon record is stored:

* **`plain`** (default) — the substructure sits at a fixed offset in the clear, as
  SoulGold stores it. The `record` block in the manifest gives the field offsets.
* **`gen3_shuffled`** — the vanilla Generation III format that Emerald-based hacks use:
  four 12-byte substructures (Growth, Attacks, EVs, Misc) XOR-encrypted with
  `personality ^ ot_id` and permuted by `personality % 24`, with a checksum over the
  decrypted block. Field positions are fixed by the format, so `record` is ignored apart
  from the nickname and OT name widths. See [`src/engine/gen3.rs`](src/engine/gen3.rs).

The two differ in more than encryption, which is why an encoding is a code path rather
than a table of offsets: Gen 3 keeps the ability in a single bit of the IV word, the egg
flag in another, and has no shiny flag at all — shininess is purely a function of the
personality value.

Within Gen 3, `record.gen3_ball` says where the Poké Ball lives: `origins_bits` (vanilla's
four bits, which cannot express more than 15 balls) or `growth_u16` (a u16 item id at
Growth+10, which newer pokeemerald-expansion builds use). Seaglass needs the latter — it
has 27 balls.

## Layout

```
assets/
  <game>/              one folder per game: manifest + data tables
  icon.png             the application icon, for both the window and the .exe
build.rs               turns icon.png into the Windows executable's icon
src/
  main.rs              pack discovery, window setup
  game/
    mod.rs             GamePack: loading, discovery, the active game
    manifest.rs        game.json schema and its defaults
  engine/
    layout.rs          save geometry, read from the active pack
    gen3.rs            the vanilla Gen 3 encrypted/shuffled record
    bytes.rs           little-endian accessors
    charmap.rs         in-game text encoding (a pack can override it)
    experience.rs      level <-> experience, per growth curve
    lookup.rs          names, learnsets, item pockets, sprite URLs
    personality.rs     PID maths (shiny toggle, nature)
    save_parser.rs     GameSave: sectors, trainer, party, boxes, bag
    save_writer.rs     checksums, slot counters, export
    mon_writer.rs      pack/clear a Pokémon, create a new one
    tests.rs           round-trip tests
  ui/                  header, party, boxes, inspector, bag, trainer
examples/
  validate_game.rs     check a pack
  gen3_probe.rs        write/read a Gen 3 record, to compare with another editor
  names.rs             nickname report for a save
  dump.rs              dump everything the parser reads
```

### The icon

`assets/icon.png` is used twice, and the two need different machinery.

The **window** icon — title bar, Alt-Tab, the taskbar button of a running
instance — is set through egui, with `ViewportBuilder::with_icon` at startup and
`ViewportCommand::Icon` again whenever the game changes, so a pack shipping its
own `icon.png` gets it. The **executable** icon — Explorer, the Start menu, a
pinned shortcut, the taskbar before the window exists — cannot come from egui at
all: Windows reads it from a resource compiled into the binary. `build.rs`
generates a multi-resolution `.ico` from the same PNG (16 through 256px, so
Windows never has to scale a mismatched size) and embeds it via `winresource`.
Nothing is committed in `.ico` form, and on non-Windows targets the build script
does nothing.

A decode failure used to produce a 1x1 transparent icon, which on screen is
indistinguishable from having none. `decode_icon` now returns `None` instead,
and a test asserts the bundled PNG decodes square, at a usable size, with a
buffer length matching its dimensions — winit silently ignores an `IconData`
that fails that last check — and with at least one pixel actually opaque.

## Correctness

Seaglass was verified end to end against a real save and its ROM: the party, all 42 boxed
Pokémon, the trainer, money and every bag pocket read correctly; exporting and reloading
reproduces an identical parse; and the name of every un-nicknamed Pokémon in that save
matches the species name extracted from the ROM byte for byte.

The Gen 3 record codec was cross-checked against an independent implementation,
[`Ehsan516/seaglass-save-editor`](https://github.com/Ehsan516/seaglass-save-editor): a
record written here decodes field-for-field there with a valid checksum, and one written
there decodes here — including under the fully-permuted `MEAG` substructure order. Tests
cover all 24 orders, checksum rejection and field preservation.

The engine was validated byte-for-byte against the original TypeScript implementation: a
synthetic save built by the TS code parses to an identical dump, every parsed Pokémon
re-packs to identical bytes, and re-exporting produces a byte-identical 128 KB file,
checksums and slot counters included. `examples/dump.rs` is the Rust side of that harness.

Since then the editor has deliberately diverged where the original was wrong — see below.
Save-file *layout* still matches byte for byte.

## Bugs fixed relative to the web version

All of these are upstream defects, reproduced in `soul-gold-web` itself.

* **Nine moves were missing from the move table**, each the first move of a generation block
  — Sketch (166), Fake Out (252), Roost (355), Hone Claws (468), Flying Press (560), Shore
  Up (622), Dynamax Cannon (690), Tera Blast (779), Shadow Blast (848). They could not be
  picked at all. The tables were regenerated from the game's own move data, which also
  corrected **37 PP values** that had been defaulted to 20 (Rest 20→5, Absorb 20→25,
  Revival Blessing 20→1) and 21 names that had lost their punctuation. Move *ids* are
  unchanged, so existing saves are unaffected.
* **Learnset names did not always match the move table.** Lookups now compare on letters and
  digits only, plus a per-game alias table, so the ★ "compatible move" marking is right.
* **Every boxed Pokémon showed as Lv.5.** A box record has no level field — the game derives
  it from experience and the growth curve — and the original returned a hardcoded 5. The
  level is computed now, and editing Level or Species restates the experience so the change
  sticks. Nothing was ever corrupted by this; a test asserts that re-applying a boxed
  Pokémon leaves its bytes unchanged.
* **Names containing a space were truncated.** A space encodes to `0x00`, which the decoder
  treated as a terminator, so `"LUGIA SHADOW"` read back — and was written back — as
  `"LUGIA"`. Only `0xFF` ends a string now.
* **Un-nicknamed Pokémon were stored UPPERCASED and clipped to 10 characters.** The game
  decides whether a Pokémon is nicknamed by comparing the stored name to the species name,
  and renames it on evolution only when they match, so `SKARMORY` vs `Skarmory` left every
  edited Pokémon permanently "nicknamed". The species name is now written verbatim at the
  full field width. `cargo run --example names -- save.sav` lists affected Pokémon.
* **The text table was missing its punctuation.** `'`, `é`, `&`, `+`, `…` and the quote
  marks had no entry, so a name carrying one decoded as `?` and was written back that way —
  `Sirfetch'd` lost its apostrophe the moment the editor touched it. Same failure mode as
  the nickname bug below, and it showed up in a real Unbound box.
* **Friendship 0 was silently rewritten to 70.** Both the reader and the writer treated 0 as
  "unset" and substituted a default, so every Pokémon whose friendship really is 0 — any
  freshly caught legendary, among others — had it changed by the act of saving. Seven of the
  42 Pokémon in the Seaglass test save were affected. 0 is now read and written verbatim.
* **673 species had no sprite.** Sprites were requested from PokéAPI by internal species id,
  which only matches the National Dex up to 905 — so every Paradox Pokémon, all of Gen 9,
  every Mega and every regional form showed a placeholder. Sources are now tried in order,
  per the manifest.

### Fixed here, not present upstream

* **Name fields were padded with terminators.** Writing a record filled the bytes after a
  name's `0xFF` terminator with more `0xFF`, where the games leave whatever was there. The
  decoded name was the same either way, but it meant 31 of the 42 records in the Seaglass
  test save changed the moment they were written back untouched — which buries a real edit
  in noise and makes "did the editor change anything?" unanswerable. Names are now written
  in place, terminator and all, and `cargo run --example roundtrip` reports **0 of 42
  records changed**.

* **Sprites were addressed by species id.** That works only for a game whose ids are
  National Dex numbers. Unbound's Gible is species 496, and National Dex 496 is Servine —
  which PokéAPI serves happily, so the editor showed the wrong Pokémon with complete
  confidence while its name, moves, IVs and nature were all correct. Species now carry an
  optional `dex`, and `sprites.species_ids_are_dex_numbers: false` stops a renumbered game
  falling back to the raw id. Unbound's offsets are not even constant: +0 through Gen 2,
  +53 for Gen 3-5, +108, then +217.
* **An erased save slot won every comparison.** A slot the game has never written is erased
  flash, every byte `0xFF`, so its counter reads as `0xFFFFFFFF` and beats any real save.
  The editor picked the blank one and reported a perfectly good file as unreadable; export
  then overflowed adding 1 to it. A slot now only counts if it holds a correctly signed
  sector.
* **The writer ignored the manifest's sector sizes.** It carried SoulGold's numbers as
  constants, so Seaglass's declared sector-13 length never applied — and fixing that
  exposed a second one: Seaglass checksums the full `0xF80` of sector 0, not `sb2_size`'s
  `0xF2C`. Every Seaglass export before this carried a wrong sector-0 checksum.
* **Export zero-filled bytes it did not model.** A sector's checksum covers only the save
  block it carries, and CFRU keeps live data past that point. Sectors are now built from
  what the game last wrote and overwritten in place, so all 14 reproduce the game's own
  bytes and checksums.

### Quirks kept on purpose

* The ability slot shares its 16-bit word with the fourth move, and both are read and
  written that way.
* Money, coins and EVs clamp rather than wrap, and the inspector only *warns* when the EV
  total passes 510.

## Differences from the web version

* **Sprites** are PNGs rather than Showdown's animated GIFs; egui's image loader draws only
  a GIF's first frame.
* **Short or unrecognised files are rejected** instead of being parsed into garbage.
* **Changing species keeps the current moves**; the web build reset them to "(None)".
* **Long lists are type-to-filter pickers**, since a combo box with 1,500 species is
  unusable. Learnset moves are starred and sorted first.
* **Saving** uses a native save dialog; `alert()`/`confirm()` are modal dialogs.
* **The game is chosen explicitly** for each save. The web build only ever had one game to
  be; with several installed, picking silently means reading a save with the wrong layout
  and showing rubbish that looks real.
