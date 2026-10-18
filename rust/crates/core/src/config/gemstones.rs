//! src/config/gemstones.js: gemstone stats by quality and item rarity.

use crate::stat::Stat;

/// One entry of JS `GEMSTONES`: the stat it gives and values by quality (COMMON..MYTHIC; DIVINE uses MYTHIC).
#[derive(Clone, Copy, Debug)]
pub struct Gemstone {
    pub gem: &'static str,
    pub stat: Stat,
    /// Quality -> values per rarity index (0 = COMMON .. 5 = MYTHIC), in JS key order.
    pub values: &'static [(&'static str, [f64; 6])],
}

impl Gemstone {
    /// JS `table.values[quality]`.
    pub fn values_for(&self, quality: &str) -> Option<&'static [f64; 6]> {
        self.values.iter().find(|(q, _)| *q == quality).map(|(_, v)| v)
    }
}

/// JS `GEMSTONES`, in key order.
pub static GEMSTONES: &[Gemstone] = &[
    Gemstone {
        gem: "JASPER",
        stat: Stat::Strength,
        values: &[
            ("ROUGH", [1.0, 1.0, 1.0, 2.0, 3.0, 4.0]),
            ("FLAWED", [2.0, 2.0, 3.0, 4.0, 4.0, 5.0]),
            ("FINE", [3.0, 3.0, 4.0, 5.0, 6.0, 7.0]),
            ("FLAWLESS", [5.0, 6.0, 7.0, 8.0, 10.0, 12.0]),
            ("PERFECT", [6.0, 7.0, 9.0, 11.0, 13.0, 16.0]),
        ],
    },
    Gemstone {
        gem: "ONYX",
        stat: Stat::CritDamage,
        values: &[
            ("ROUGH", [1.0, 1.0, 2.0, 2.0, 3.0, 4.0]),
            ("FLAWED", [2.0, 2.0, 3.0, 3.0, 4.0, 6.0]),
            ("FINE", [3.0, 3.0, 4.0, 5.0, 6.0, 8.0]),
            ("FLAWLESS", [4.0, 5.0, 6.0, 7.0, 8.0, 10.0]),
            ("PERFECT", [5.0, 6.0, 7.0, 8.0, 10.0, 12.0]),
        ],
    },
    Gemstone {
        gem: "SAPPHIRE",
        stat: Stat::Intelligence,
        values: &[
            ("ROUGH", [2.0, 3.0, 4.0, 5.0, 6.0, 7.0]),
            ("FLAWED", [5.0, 5.0, 6.0, 7.0, 8.0, 10.0]),
            ("FINE", [7.0, 8.0, 9.0, 10.0, 11.0, 12.0]),
            ("FLAWLESS", [10.0, 11.0, 12.0, 14.0, 17.0, 20.0]),
            ("PERFECT", [12.0, 14.0, 17.0, 20.0, 24.0, 30.0]),
        ],
    },
];

/// JS `GEMSTONES[gem]`.
pub fn gemstone(gem: &str) -> Option<&'static Gemstone> {
    GEMSTONES.iter().find(|g| g.gem == gem)
}

pub const GEM_QUALITIES: [&str; 5] = ["ROUGH", "FLAWED", "FINE", "FLAWLESS", "PERFECT"];
pub const UPGRADE_QUALITIES: [&str; 2] = ["FLAWLESS", "PERFECT"];

/// JS `SLOT_GEMS[type]`: gems a slot type accepts (`None` for typed slots, which accept only themselves).
pub fn slot_gems(slot_type: &str) -> Option<&'static [&'static str]> {
    Some(match slot_type {
        "COMBAT" => &["JASPER", "SAPPHIRE", "ONYX", "RUBY", "AMETHYST"],
        "OFFENSIVE" => &["JASPER", "SAPPHIRE"],
        "DEFENSIVE" => &["RUBY", "AMETHYST", "OPAL"],
        "UNIVERSAL" => &[
            "JASPER", "SAPPHIRE", "ONYX", "RUBY", "AMETHYST", "OPAL", "TOPAZ", "JADE", "AMBER", "PERIDOT", "CITRINE",
            "AQUAMARINE",
        ],
        _ => return None,
    })
}

/// JS `gemStat(gem, quality, rarity)` (stats.js): `Some((stat, value))`, or `None` for unknown gems / qualities.
/// The rarity index is `RARITIES.indexOf(rarity)` clamped to the table (unknown rarities count as COMMON).
pub fn gem_stat(gem: &str, quality: &str, rarity: &str) -> Option<(Stat, f64)> {
    let table = gemstone(gem)?;
    let row = table.values_for(quality)?;
    let idx = crate::config::constants::rarity_index(rarity).map(|i| i as i64).unwrap_or(-1);
    let i = idx.max(0).min(row.len() as i64 - 1) as usize;
    Some((table.stat, row[i]))
}
