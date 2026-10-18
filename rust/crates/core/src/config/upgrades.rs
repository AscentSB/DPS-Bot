//! src/config/upgrades.js: the extra upgrades database (ability items).

use crate::kinds::Class;
use crate::stat::Stat;

/// A cost part `{ id, qty? }` (`qty` undefined = 1 for pricing).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CostPart {
    pub id: &'static str,
    pub qty: Option<f64>,
}

/// One entry of JS `EXTRA_UPGRADES`. `None` = the JS key is absent.
#[derive(Clone, Copy, Debug)]
pub struct ExtraUpgrade {
    pub key: &'static str,
    pub name: &'static str,
    pub group: Option<&'static str>,
    pub stats: Option<&'static [(Stat, f64)]>,
    pub wither: Option<&'static [(Stat, f64)]>,
    pub dragon: Option<&'static [(Stat, f64)]>,
    pub cost: &'static [CostPart],
    /// `owned.items`: any of these anywhere in the profile.
    pub owned_items: Option<&'static [&'static str]>,
    pub requires: Option<&'static str>,
    pub classes: Option<&'static [Class]>,
    /// `activeWith`: only counts while that key is active.
    pub active_with: Option<&'static str>,
}

/// JS `EXTRA_UPGRADES`, in order.
pub static EXTRA_UPGRADES: &[ExtraUpgrade] = &[
    ExtraUpgrade {
        key: "WEIRD_TUBA",
        name: "Weird Tuba",
        group: Some("tuba"),
        stats: None,
        wither: None,
        dragon: Some(&[(Stat::Strength, 30.0), (Stat::Speed, 30.0)]),
        cost: &[CostPart { id: "WEIRD_TUBA", qty: None }],
        owned_items: Some(&["WEIRD_TUBA"]),
        requires: None,
        classes: None,
        active_with: None,
    },
    ExtraUpgrade {
        key: "WEIRDER_TUBA",
        name: "Weirder Tuba",
        group: Some("tuba"),
        stats: None,
        wither: None,
        dragon: Some(&[(Stat::Strength, 40.0), (Stat::AttackSpeed, 5.0), (Stat::Speed, 30.0), (Stat::CritDamage, 10.0)]),
        cost: &[CostPart { id: "WEIRDER_TUBA", qty: None }],
        owned_items: Some(&["WEIRDER_TUBA"]),
        requires: None,
        classes: None,
        active_with: None,
    },
    ExtraUpgrade {
        key: "SWORD_OF_BAD_HEALTH",
        name: "Sword of Bad Health (ability)",
        group: None,
        stats: Some(&[(Stat::Strength, 100.0)]),
        wither: None,
        dragon: None,
        cost: &[CostPart { id: "SWORD_OF_BAD_HEALTH", qty: None }],
        owned_items: Some(&["SWORD_OF_BAD_HEALTH"]),
        requires: None,
        classes: None,
        active_with: None,
    },
];

/// JS `EXTRAS_BY_KEY.get(key)` (stats.js).
pub fn extra_upgrade(key: &str) -> Option<&'static ExtraUpgrade> {
    EXTRA_UPGRADES.iter().find(|e| e.key == key)
}

/// Index of an extra in `EXTRA_UPGRADES` (for bit sets of owned / active extras).
pub fn extra_index(key: &str) -> Option<usize> {
    EXTRA_UPGRADES.iter().position(|e| e.key == key)
}
