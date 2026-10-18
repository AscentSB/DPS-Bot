//! src/config/itemUpgrades.js: items that upgrade into a better item and keep every upgrade on them.

use crate::config::pets::Part;

/// One upgrade target of JS `ITEM_UPGRADES[from]`: `{ id, parts: [{ id, qty }] }`.
#[derive(Clone, Copy, Debug)]
pub struct ItemUpgrade {
    pub id: &'static str,
    pub parts: &'static [Part],
}

const fn up(id: &'static str, part: &'static [Part]) -> ItemUpgrade {
    ItemUpgrade { id, parts: part }
}

const DIAMOND_BLOCKS: &[Part] = &[Part { id: "ENCHANTED_DIAMOND_BLOCK", qty: 8.0 }];
const FRAG_POWER: &[Part] = &[Part { id: "GIANT_FRAGMENT_DIAMOND", qty: 8.0 }];
const FRAG_WISE: &[Part] = &[Part { id: "GIANT_FRAGMENT_LASER", qty: 8.0 }];
const FRAG_SPEED: &[Part] = &[Part { id: "GIANT_FRAGMENT_BIGFOOT", qty: 8.0 }];
const FRAG_TANK: &[Part] = &[Part { id: "GIANT_FRAGMENT_BOULDER", qty: 8.0 }];

/// JS `ITEM_UPGRADES`, in key order: from id -> upgrades (golden boss heads, then wither armor by slot; each wither
/// piece lists POWER, WISE, SPEED, TANK in that order).
pub static ITEM_UPGRADES: &[(&str, &[ItemUpgrade])] = &[
    ("GOLD_BONZO_HEAD", &[up("DIAMOND_BONZO_HEAD", DIAMOND_BLOCKS)]),
    ("GOLD_SCARF_HEAD", &[up("DIAMOND_SCARF_HEAD", DIAMOND_BLOCKS)]),
    ("GOLD_PROFESSOR_HEAD", &[up("DIAMOND_PROFESSOR_HEAD", DIAMOND_BLOCKS)]),
    ("GOLD_THORN_HEAD", &[up("DIAMOND_THORN_HEAD", DIAMOND_BLOCKS)]),
    ("GOLD_LIVID_HEAD", &[up("DIAMOND_LIVID_HEAD", DIAMOND_BLOCKS)]),
    ("GOLD_SADAN_HEAD", &[up("DIAMOND_SADAN_HEAD", DIAMOND_BLOCKS)]),
    ("GOLD_NECRON_HEAD", &[up("DIAMOND_NECRON_HEAD", DIAMOND_BLOCKS)]),
    (
        "WITHER_HELMET",
        &[
            up("POWER_WITHER_HELMET", FRAG_POWER),
            up("WISE_WITHER_HELMET", FRAG_WISE),
            up("SPEED_WITHER_HELMET", FRAG_SPEED),
            up("TANK_WITHER_HELMET", FRAG_TANK),
        ],
    ),
    (
        "WITHER_CHESTPLATE",
        &[
            up("POWER_WITHER_CHESTPLATE", FRAG_POWER),
            up("WISE_WITHER_CHESTPLATE", FRAG_WISE),
            up("SPEED_WITHER_CHESTPLATE", FRAG_SPEED),
            up("TANK_WITHER_CHESTPLATE", FRAG_TANK),
        ],
    ),
    (
        "WITHER_LEGGINGS",
        &[
            up("POWER_WITHER_LEGGINGS", FRAG_POWER),
            up("WISE_WITHER_LEGGINGS", FRAG_WISE),
            up("SPEED_WITHER_LEGGINGS", FRAG_SPEED),
            up("TANK_WITHER_LEGGINGS", FRAG_TANK),
        ],
    ),
    (
        "WITHER_BOOTS",
        &[
            up("POWER_WITHER_BOOTS", FRAG_POWER),
            up("WISE_WITHER_BOOTS", FRAG_WISE),
            up("SPEED_WITHER_BOOTS", FRAG_SPEED),
            up("TANK_WITHER_BOOTS", FRAG_TANK),
        ],
    ),
];

/// JS `ITEM_UPGRADES[from]`.
pub fn item_upgrades(from: &str) -> Option<&'static [ItemUpgrade]> {
    ITEM_UPGRADES.iter().find(|(k, _)| *k == from).map(|&(_, v)| v)
}
