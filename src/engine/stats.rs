//! Battle stats, from base stats, IVs, EVs, level and nature.
//!
//! A party record stores its stats rather than deriving them, and the games
//! only recompute them at particular moments — levelling up, withdrawing from
//! the PC. A record the editor writes into the party therefore has to carry
//! the right numbers itself: a placeholder shows up in game as a Pokémon with
//! 1 HP, and stays that way.
//!
//! The formulas are the Generation III ones, which every later generation
//! kept:
//!
//! ```text
//!   HP    = (2B + IV + EV/4) * L / 100 + L + 10
//!   other = ((2B + IV + EV/4) * L / 100 + 5) * nature
//! ```

use super::save_parser::Pokemon;

/// Stats in the order the records and the editor use: HP, Attack, Defense,
/// Speed, Sp. Attack, Sp. Defense.
pub type Stats = [u16; 6];

/// Shedinja's base HP. A species with it always has exactly 1 HP.
const ONE_HP_BASE: u32 = 1;

/// The stats `mon` should have, or `None` if the pack has no base stats for its
/// species.
pub fn calculate(mon: &Pokemon) -> Option<Stats> {
    let base = crate::game::current()
        .species
        .get(&mon.species)
        .and_then(|s| s.stats)?;
    let base = [
        base.hp,
        base.attack,
        base.defense,
        base.speed,
        base.sp_attack,
        base.sp_defense,
    ];
    Some(from_parts(
        base,
        mon.ivs,
        mon.evs,
        u32::from(mon.level.clamp(1, 100)),
        effective_nature(mon),
    ))
}

/// The nature the stats are computed with. pokeemerald-expansion lets a mint
/// change it without touching the personality value, by storing an offset.
fn effective_nature(mon: &Pokemon) -> u32 {
    (mon.nature() + u32::from(mon.hidden_nature_modifier)) % 25
}

fn from_parts(base: [u32; 6], ivs: [u32; 6], evs: [u32; 6], level: u32, nature: u32) -> Stats {
    let core = |i: usize| (2 * base[i] + ivs[i].min(31) + evs[i].min(255) / 4) * level / 100;

    let mut stats = [0u16; 6];
    stats[0] = if base[0] == ONE_HP_BASE {
        1
    } else {
        (core(0) + level + 10) as u16
    };

    // Natures raise one of Atk/Def/Spe/SpA/SpD by 10% and lower another; the
    // two indices are the nature's id in base 5. Equal indices are neutral.
    let raised = (nature / 5) as usize + 1;
    let lowered = (nature % 5) as usize + 1;
    for (i, stat) in stats.iter_mut().enumerate().skip(1) {
        let value = core(i) + 5;
        let value = if raised == lowered {
            value
        } else if i == raised {
            value * 110 / 100
        } else if i == lowered {
            value * 90 / 100
        } else {
            value
        };
        *stat = value as u16;
    }
    stats
}

/// Rewrites a party Pokémon's stats to match its level, IVs, EVs and nature.
///
/// Current HP keeps its distance from full, so a full-health Pokémon stays at
/// full health and a fainted one stays fainted. Nothing changes if the pack
/// has no base stats for the species.
pub fn refresh(mon: &mut Pokemon) {
    let Some([hp, attack, defense, speed, sp_attack, sp_defense]) = calculate(mon) else {
        return;
    };
    let old_max = mon.max_hp.unwrap_or(0);
    let old_hp = mon.hp.unwrap_or(old_max);
    let damage = old_max.saturating_sub(old_hp);
    mon.hp = Some(if old_hp == 0 && old_max > 0 {
        0
    } else {
        hp.saturating_sub(damage).max(1)
    });
    mon.max_hp = Some(hp);
    mon.attack = Some(attack);
    mon.defense = Some(defense);
    mon.speed = Some(speed);
    mon.sp_attack = Some(sp_attack);
    mon.sp_defense = Some(sp_defense);
}

/// Whether an edit touched anything the stats are computed from.
pub fn inputs_changed(before: &Pokemon, after: &Pokemon) -> bool {
    before.species != after.species
        || before.level != after.level
        || before.ivs != after.ivs
        || before.evs != after.evs
        || effective_nature(before) != effective_nature(after)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bulbapedia's worked example: Garchomp, level 78, Adamant, with the
    /// IVs and EVs below, has 289/278/193/171/135/171.
    #[test]
    fn matches_the_published_worked_example() {
        let stats = from_parts(
            [108, 130, 95, 102, 80, 85],
            [24, 12, 30, 5, 16, 23],
            [74, 190, 91, 23, 48, 84],
            78,
            3, // Adamant: +Atk, -SpA
        );
        assert_eq!(stats, [289, 278, 193, 171, 135, 171]);
    }

    #[test]
    fn a_neutral_nature_changes_nothing() {
        let neutral = from_parts([80; 6], [31; 6], [0; 6], 50, 0);
        assert!(neutral[1..].iter().all(|s| *s == neutral[1]));
    }

    #[test]
    fn shedinja_always_has_one_hp() {
        let stats = from_parts([1, 90, 45, 40, 30, 30], [31; 6], [252; 6], 100, 0);
        assert_eq!(stats[0], 1);
    }
}
