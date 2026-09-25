//! Level ⇄ experience conversion.
//!
//! A boxed Pokémon's 76-byte record has no level field — the game derives the
//! level from the experience total and the species' growth rate. The editor has
//! to do the same, or every boxed Pokémon reads as the placeholder level.
//!
//! The curves are the standard ones. A species' `growth` field names one of
//! "fast", "medium", "medium-slow", "slow", "slow-then-very-fast" (erratic) or
//! "fast-then-very-slow" (fluctuating); anything unrecognised is treated as
//! medium.

use crate::game;

pub const MAX_LEVEL: u8 = 100;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum GrowthRate {
    Fast,
    MediumFast,
    MediumSlow,
    Slow,
    /// "slow-then-very-fast"
    Erratic,
    /// "fast-then-very-slow"
    Fluctuating,
}

impl GrowthRate {
    fn from_label(label: &str) -> Self {
        match label {
            "fast" => Self::Fast,
            "medium-slow" => Self::MediumSlow,
            "slow" => Self::Slow,
            "slow-then-very-fast" => Self::Erratic,
            "fast-then-very-slow" => Self::Fluctuating,
            // "medium" and anything unrecognised
            _ => Self::MediumFast,
        }
    }

    /// Total experience needed to reach `level` on this curve.
    pub fn experience_for_level(self, level: u8) -> u32 {
        let n = i64::from(level.clamp(1, MAX_LEVEL));
        if n == 1 {
            // The game's tables start every curve at 0, though some formulas
            // evaluate to 1 here.
            return 0;
        }
        let cube = n * n * n;

        let exp = match self {
            Self::Fast => 4 * cube / 5,
            Self::MediumFast => cube,
            Self::MediumSlow => 6 * cube / 5 - 15 * n * n + 100 * n - 140,
            Self::Slow => 5 * cube / 4,
            Self::Erratic => {
                if n < 50 {
                    cube * (100 - n) / 50
                } else if n < 68 {
                    cube * (150 - n) / 100
                } else if n < 98 {
                    cube * ((1911 - 10 * n) / 3) / 500
                } else {
                    cube * (160 - n) / 100
                }
            }
            Self::Fluctuating => {
                if n < 15 {
                    cube * ((n + 1) / 3 + 24) / 50
                } else if n < 36 {
                    cube * (n + 14) / 50
                } else {
                    cube * (n / 2 + 32) / 50
                }
            }
        };

        exp.max(0) as u32
    }

    /// The highest level whose experience requirement `exp` meets.
    pub fn level_from_experience(self, exp: u32) -> u8 {
        let mut level = 1;
        for candidate in 2..=MAX_LEVEL {
            if self.experience_for_level(candidate) > exp {
                break;
            }
            level = candidate;
        }
        level
    }
}

pub fn growth_rate(species_id: u32) -> GrowthRate {
    game::current()
        .species
        .get(&species_id)
        .and_then(|s| s.growth.as_deref())
        .map(GrowthRate::from_label)
        .unwrap_or(GrowthRate::MediumFast)
}

pub fn level_from_experience(species_id: u32, exp: u32) -> u8 {
    growth_rate(species_id).level_from_experience(exp)
}

pub fn experience_for_level(species_id: u32, level: u8) -> u32 {
    growth_rate(species_id).experience_for_level(level)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_100_totals_match_the_standard_curves() {
        assert_eq!(GrowthRate::Fast.experience_for_level(100), 800_000);
        assert_eq!(GrowthRate::MediumFast.experience_for_level(100), 1_000_000);
        assert_eq!(GrowthRate::MediumSlow.experience_for_level(100), 1_059_860);
        assert_eq!(GrowthRate::Slow.experience_for_level(100), 1_250_000);
        assert_eq!(GrowthRate::Erratic.experience_for_level(100), 600_000);
        assert_eq!(GrowthRate::Fluctuating.experience_for_level(100), 1_640_000);
    }

    #[test]
    fn level_1_costs_nothing() {
        for rate in [
            GrowthRate::Fast,
            GrowthRate::MediumFast,
            GrowthRate::MediumSlow,
            GrowthRate::Slow,
            GrowthRate::Erratic,
            GrowthRate::Fluctuating,
        ] {
            assert_eq!(rate.experience_for_level(1), 0, "{rate:?}");
            assert_eq!(rate.level_from_experience(0), 1, "{rate:?}");
        }
    }

    #[test]
    fn level_and_experience_round_trip() {
        for rate in [
            GrowthRate::Fast,
            GrowthRate::MediumFast,
            GrowthRate::MediumSlow,
            GrowthRate::Slow,
            GrowthRate::Erratic,
            GrowthRate::Fluctuating,
        ] {
            for level in 1..=MAX_LEVEL {
                let exp = rate.experience_for_level(level);
                assert_eq!(
                    rate.level_from_experience(exp),
                    level,
                    "{rate:?} level {level}"
                );
                // One point short of the next level still reads as this level.
                if level < MAX_LEVEL {
                    let next = rate.experience_for_level(level + 1);
                    assert_eq!(
                        rate.level_from_experience(next - 1),
                        level,
                        "{rate:?} {level}"
                    );
                }
            }
        }
    }

    #[test]
    fn species_growth_rates_are_read_from_the_data() {
        crate::engine::tests::ensure_pack();
        // Dragonite and Skarmory are on the slow curve.
        assert_eq!(growth_rate(149), GrowthRate::Slow);
        assert_eq!(growth_rate(227), GrowthRate::Slow);
        assert_eq!(experience_for_level(149, 55), 5 * 55 * 55 * 55 / 4);
        assert_eq!(level_from_experience(149, 5 * 55 * 55 * 55 / 4), 55);
    }
}
