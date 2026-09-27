# Pokémon ROM hack save editor

A `.sav` editor for Gen3 Pokémon ROM hacks (FireRed and Emerald), for Windows, Linux, macOS, and browser.
The browser version serves a `.wasm` file, where the binary runs locally in your browser.

⚠️ **Always keep a backup before overwriting a save**

The following editors were helpful in understanding how to read each romhack:
| Game | Existing Editor |  Record format | State |
| ---- | --------------- | -------------- | ----- |
| SoulGold | https://github.com/jozeton-app/soul-gold-web | `plain` | 100% |
| Unbound | https://github.com/Zannael/PUSE | `gen3_plain` + `cfru_compact` boxes | 100% |
| Seaglass | https://github.com/Ehsan516/seaglass-save-editor | `gen3_shuffled` | 100% |
| FireRed Rocket Edition | - | - | 0% |


## Adding a new game

The editor core is built to be agnostic. Every game folder under `assets/` is compiled into the binary. New games can be somewhat easily added through the `/assets/<game>`, where the `/<game>` folder can take the following .json files:
| File | Shape | Notes |
| --- | --- | --- |
| `game.json` | manifest | **required**; see sub-section |
| `species.json` | one entry per species | **required**; see sub-section |
| `moves.json` | `{"1": {"name": "Pound", "pp": 35}}` | base PP; a missing or 0 `pp` reads as 20 |
| `items.json` | `{"1": "Poke Ball"}` | |
| `item_pockets.json` | `{"1": "PokeBalls"}` | values match pocket names in the manifest |
| `abilities.json` | `{"1": "Stench"}` | |
| `charmap.json` | `{"187": "A"}` | optional; overrides the built-in text encoding |
| species `dex` | `{"496": {"dex": 443}}` | National Dex number, for games that renumber species |

> Run `cargo run --example validate_game` afterwards; it will check the save layout for self-consistency

> Only `game.json` and `species.json` are required; anything missing still opens but will show blanks. 

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
    "sprite": "bulbasaur",
    "stats": { "hp": 45, "attack": 49, "defense": 49, "speed": 45, "sp_attack": 65, "sp_defense": 65 },
    "learnset": ["Tackle", "Growl", "Vine Whip"]
  }
}
```

### The manifest

Every field except `name` has a default, so this is a valid `game.json`:

```json
{ "name": "SoulGold", "version": "1.0" }
```

### Boxes outside the storage block

Some ROMs expand the box size; Many times the extra boxes are not in the same block of data, so a manual offset is required. In `game.json`, you can add the optional field:

```json
"extra_boxes": {
  "sectors": [[28, 29], [30, 31]],
  "sector_checksum_sizes": [3968, 1324],
  "markers": [{ "in": "storage", "offset": 34740, "text": "BX16" }],
  "boxes": [
    {
      "records": [
        { "in": "storage", "offset": 34744, "length": 968 },
        { "in": "extra", "sector": 1, "offset": 0, "length": 1312 }
      ],
      "name": { "in": "extra", "sector": 1, "offset": 1312 }
    }
  ],
  "checksums": [
    { "at": { "in": "extra", "sector": 0, "offset": 8 },
      "over": [{ "in": "extra", "sector": 0, "offset": 12, "length": 3880 }] }
  ]
}
```

## Development

Run locally:
```
cargo run --release
```

The browser build uses [Trunk](https://trunkrs.dev):

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
trunk serve --open
```

```bash
cargo test 
```
