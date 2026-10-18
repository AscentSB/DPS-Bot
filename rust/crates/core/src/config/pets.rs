//! src/config/pets.js: pet items, pet perks (as a function), phase pets, Kat upgrades.

use crate::kinds::{Class, Target};
use crate::stat::{Stat, StatList};

/// One entry of JS `PET_ITEMS`: `mult` multiplies the pet's stat, `flat` adds to it (empty when the JS has none).
#[derive(Clone, Copy, Debug)]
pub struct PetItem {
    pub id: &'static str,
    pub name: &'static str,
    pub mult: &'static [(Stat, f64)],
    pub flat: &'static [(Stat, f64)],
}

/// JS `PET_ITEMS`, in key order.
pub static PET_ITEMS: &[PetItem] = &[
    PetItem { id: "ANTIQUE_REMEDIES", name: "Antique Remedies", mult: &[(Stat::Strength, 1.8)], flat: &[] },
    PetItem { id: "HEPHAESTUS_REMEDIES", name: "Hephaestus Remedies", mult: &[(Stat::Strength, 2.0)], flat: &[] },
    PetItem {
        id: "HEPHAESTUS_RELIC",
        name: "Hephaestus Relic",
        mult: &[
            (Stat::Strength, 1.5),
            (Stat::CritDamage, 1.5),
            (Stat::CritChance, 1.5),
            (Stat::Intelligence, 1.5),
            (Stat::Speed, 1.5),
            (Stat::AttackSpeed, 1.5),
            (Stat::AbilityDamage, 1.5),
        ],
        flat: &[],
    },
    PetItem { id: "GOLD_CLAWS", name: "Gold Claws", mult: &[(Stat::CritDamage, 1.5), (Stat::CritChance, 1.5)], flat: &[] },
    PetItem {
        id: "PET_ITEM_SHARPENED_CLAWS_UNCOMMON",
        name: "Sharpened Claws",
        mult: &[],
        flat: &[(Stat::CritDamage, 15.0)],
    },
    PetItem { id: "PET_ITEM_BIG_TEETH_COMMON", name: "Big Teeth", mult: &[], flat: &[(Stat::CritChance, 5.0)] },
    PetItem { id: "PET_ITEM_TEXTBOOK", name: "Textbook", mult: &[(Stat::Intelligence, 2.0)], flat: &[] },
    PetItem { id: "CROCHET_TIGER_PLUSHIE", name: "Crochet Tiger Plushie", mult: &[], flat: &[(Stat::AttackSpeed, 35.0)] },
    PetItem { id: "HEPHAESTUS_PLUSHIE", name: "Hephaestus Plushie", mult: &[], flat: &[(Stat::AttackSpeed, 50.0)] },
];

/// JS `PET_ITEMS[id]`.
pub fn pet_item(id: &str) -> Option<&'static PetItem> {
    PET_ITEMS.iter().find(|p| p.id == id)
}

pub const GOLDEN_DRAGON_BANK_CAP: f64 = 1e9;
pub const GOLD_COLLECTION_DIGITS: f64 = 9.0;

/// Lion Primal Force: stats on the weapon (JS `special.weaponStats`, keys `damage` then `strength`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WeaponStats {
    pub damage: f64,
    pub strength: f64,
}

impl WeaponStats {
    /// As an ordered stat slice (JS key order).
    pub fn as_stats(&self) -> [(Stat, f64); 2] {
        [(Stat::Damage, self.damage), (Stat::Strength, self.strength)]
    }
}

/// JS perk `special` object: perk effects that are not pet stats. `None` = the key is absent (JS `undefined`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PetSpecial {
    pub deaths_touch: Option<f64>,
    pub combo_strength: Option<f64>,
    pub potato_multiplier: Option<f64>,
    pub item_boost: Option<f64>,
    pub weapon_stats: Option<WeaponStats>,
    pub first_strike_boost: Option<f64>,
    pub perk_strength: Option<f64>,
    pub perk_speed: Option<f64>,
    pub speed_cap_bonus: Option<f64>,
}

/// What a JS `PET_PERKS[type](n, p)` call returns. `extra` holds the remaining keys (the stat adds such as
/// `additiveDamage`), in the JS key order.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PetPerk {
    pub pet_strength: Option<f64>,
    pub potion_boost: Option<f64>,
    pub extra: StatList,
    pub wither: StatList,
    pub dragon: StatList,
    pub special: PetSpecial,
}

/// The profile data a perk reads (JS `p`): bank coins and Gold collection, already defaulted by the caller.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerkProfile {
    pub bank: f64,
    pub gold_collection: f64,
}

/// JS `digits(n)`: digits of the integer part, 0 when `n <= 0` (or NaN).
fn digits(n: f64) -> f64 {
    if n > 0.0 {
        crate::jsfmt::floor_digits(n) as f64
    } else {
        0.0
    }
}

/// Pet types with a perk (JS `PET_PERKS` keys, in order).
pub const PET_PERK_TYPES: [&str; 13] = [
    "GOLDEN_DRAGON",
    "WITHER_SKELETON",
    "SKELETON",
    "BLAZE",
    "PIGMAN",
    "TIGER",
    "TYRANNOSAURUS",
    "LION",
    "GRIFFIN",
    "ANKYLOSAURUS",
    "BLACK_CAT",
    "ENDER_DRAGON",
    "JELLYFISH",
];

/// JS `PET_PERKS[type]?.(n, p)`: `None` when the type has no perk. `n` = the pet's perk numbers at its level.
pub fn pet_perk(kind: &str, n: &[f64], p: PerkProfile) -> Option<PetPerk> {
    let at = |i: usize| n.get(i).copied().unwrap_or(0.0); // JS n[i] ?? 0
    let mut perk = PetPerk::default();
    match kind {
        "GOLDEN_DRAGON" => {
            perk.pet_strength = Some(11.1 * crate::jsmath::min(GOLD_COLLECTION_DIGITS, digits(p.gold_collection)));
            perk.extra.set(Stat::AdditiveStrength, at(2) / 100.0);
            perk.extra.set(Stat::AdditiveDamage, crate::jsmath::min(at(4), at(3) * (p.bank / 1e6)));
        }
        "WITHER_SKELETON" => {
            perk.wither.set(Stat::AdditiveDamage, 100.0);
            perk.special.deaths_touch = Some(0.67);
        }
        "SKELETON" => {
            perk.extra.set(Stat::BowDamage, 2.0 * at(0));
            perk.special.combo_strength = Some(3.0 * at(1));
        }
        "BLAZE" => perk.special.potato_multiplier = Some(2.0),
        "PIGMAN" => perk.extra.set(Stat::AdditiveDamage, 75.0),
        "TIGER" => {
            perk.wither.set(Stat::AdditiveDamage, at(2));
            perk.extra.set(Stat::AdditiveFerocity, at(0) / 100.0);
        }
        "TYRANNOSAURUS" => perk.special.item_boost = Some(at(2) / 100.0),
        "LION" => {
            perk.special.weapon_stats = Some(WeaponStats { damage: at(0), strength: at(1) });
            perk.special.first_strike_boost = Some(at(2) / 100.0);
        }
        "GRIFFIN" => perk.extra.set(Stat::AdditiveStrength, at(0) / 100.0),
        "ANKYLOSAURUS" => perk.special.perk_strength = Some(500.0),
        "BLACK_CAT" => {
            perk.special.perk_speed = Some(at(0));
            perk.special.speed_cap_bonus = Some(at(0));
        }
        "ENDER_DRAGON" => {
            perk.dragon.set(Stat::AdditiveDamage, at(0));
            let s = at(3) / 100.0;
            perk.extra.set(Stat::AdditiveStrength, s);
            perk.extra.set(Stat::AdditiveCritDamage, s);
            perk.extra.set(Stat::AdditiveCritChance, s);
            perk.extra.set(Stat::AdditiveIntelligence, s);
            perk.extra.set(Stat::AdditiveAttackSpeed, s);
        }
        "JELLYFISH" => perk.potion_boost = Some(at(2) / 100.0),
        _ => return None,
    }
    Some(perk)
}

/// JS `PHASE_PETS[target]?.[cls]`: the pet used against each boss when owned.
#[inline]
pub const fn phase_pet(target: Target, cls: Class) -> Option<&'static str> {
    match (target, cls) {
        (Target::Wither, _) => Some("GOLDEN_DRAGON"),
        (Target::Dragon, Class::Arch | Class::Bers) => Some("ENDER_DRAGON"),
        (Target::Dragon, Class::Mage) => None,
    }
}

pub const EXCLUDED_PETS: [&str; 2] = ["HORSE", "SKELETON_HORSE"];
pub const TIER_BOOST_ID: &str = "PET_ITEM_TIER_BOOST";

pub const KAT_LEVEL_DISCOUNT: f64 = 0.003;
pub const KAT_FLOWER_ID: &str = "KAT_FLOWER";

/// One `{ id, qty }` part.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub id: &'static str,
    pub qty: f64,
}

/// One entry of JS `KAT_UPGRADES[type][tier]`.
#[derive(Clone, Copy, Debug)]
pub struct KatUpgrade {
    pub kind: &'static str,
    pub tier: &'static str,
    pub to: &'static str,
    pub coins: f64,
    pub items: &'static [Part],
}

pub static KAT_UPGRADES: &[KatUpgrade] = &[KatUpgrade {
    kind: "ENDER_DRAGON",
    tier: "EPIC",
    to: "LEGENDARY",
    coins: 400_000_000.0,
    items: &[Part { id: "SUMMONING_EYE", qty: 8.0 }],
}];

/// JS `KAT_UPGRADES[type]?.[tier]`.
pub fn kat_upgrade(kind: &str, tier: &str) -> Option<&'static KatUpgrade> {
    KAT_UPGRADES.iter().find(|k| k.kind == kind && k.tier == tier)
}

pub const BUY_RARITIES: [&str; 2] = ["MYTHIC", "LEGENDARY"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perks() {
        let p = PerkProfile { bank: 5e8, gold_collection: 1e9 };
        let n = [10.0, 0.5, 5.0, 0.25, 250.0];
        let g = pet_perk("GOLDEN_DRAGON", &n, p).unwrap();
        assert_eq!(g.pet_strength, Some(11.1 * 9.0));
        assert_eq!(g.extra.as_slice(), &[(Stat::AdditiveStrength, 0.05), (Stat::AdditiveDamage, 125.0)]);
        assert!(pet_perk("HORSE", &n, p).is_none());
        let short = pet_perk("LION", &[1.0], p).unwrap();
        assert_eq!(short.special.weapon_stats, Some(WeaponStats { damage: 1.0, strength: 0.0 }));
        assert_eq!(digits(0.5), 1.0);
        assert_eq!(digits(0.0), 0.0);
        assert_eq!(digits(123456789.0), 9.0);
    }
}
