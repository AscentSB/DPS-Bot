//! Compares dps-core with the JS reference dump in testdata/core.json (written by oracle/core-dump.mjs): every config
//! table (values and iteration order), the data files as the JS sees them, pet stats, item requirements, floor drops,
//! cleanName and the formulas (bit-exact). Skips when the file is missing.

use dps_core::config::{
    accessories as acc, armor_sets as ar, constants as c, enchants as en, essence as es, gemstones as gm, item_stats as ist,
    item_upgrades as iu, pets as pc, profile as pr, shards as sh, upgrades as up,
};
use dps_core::formulas::{self as f, FinalStats};
use dps_core::model::{Blessings, PetProfile, Resource};
use dps_core::requirements::{meets_requirements, Player};
use dps_core::{data, drops, pets, Class, FloorMode, Mode, Stat, StatTarget};
use serde_json::{json, Value};
use std::sync::OnceLock;

// ---------------------------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------------------------

fn core() -> Option<&'static Value> {
    static CORE: OnceLock<Option<Value>> = OnceLock::new();
    CORE.get_or_init(|| {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/core.json");
        match std::fs::read_to_string(path) {
            Ok(t) => Some(serde_json::from_str(&t).expect("core.json")),
            Err(_) => {
                eprintln!("skipping: {path} missing (run oracle/core-dump.mjs)");
                None
            }
        }
    })
    .as_ref()
}

fn resource() -> &'static Resource {
    static R: OnceLock<Resource> = OnceLock::new();
    R.get_or_init(|| {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/snapshot.raw.json");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("snapshot.raw.json")).unwrap();
        let mut r = Resource::default();
        for pair in v["items"].as_array().unwrap() {
            let id = pair[0].as_str().unwrap().to_string();
            let info = serde_json::from_value(pair[1].clone()).unwrap_or_else(|e| panic!("{id}: {e}"));
            r.insert(id, info);
        }
        r
    })
}

/// A JS number as the oracle encodes it.
fn n(x: f64) -> Value {
    if x.is_nan() {
        json!("NaN")
    } else if x == f64::INFINITY {
        json!("Infinity")
    } else if x == f64::NEG_INFINITY {
        json!("-Infinity")
    } else if x == 0.0 && x.is_sign_negative() {
        json!("-0")
    } else {
        json!(x)
    }
}

fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(x) => x.as_f64(),
        Value::String(s) => match s.as_str() {
            "NaN" => Some(f64::NAN),
            "Infinity" => Some(f64::INFINITY),
            "-Infinity" => Some(f64::NEG_INFINITY),
            "-0" => Some(-0.0),
            _ => None,
        },
        _ => None,
    }
}

fn nums(xs: &[f64]) -> Value {
    Value::Array(xs.iter().map(|&x| n(x)).collect())
}

fn strs(xs: &[&str]) -> Value {
    Value::Array(xs.iter().map(|&x| json!(x)).collect())
}

fn st(stats: &[(Stat, f64)]) -> Value {
    Value::Object(stats.iter().map(|&(k, v)| (k.js_name().to_string(), n(v))).collect())
}

fn stat_names(stats: &[Stat]) -> Value {
    Value::Array(stats.iter().map(|s| json!(s.js_name())).collect())
}

fn classes(cls: &[Class]) -> Value {
    Value::Array(cls.iter().map(|c| json!(c.js_name())).collect())
}

fn obj(entries: Vec<(&str, Value)>) -> Value {
    Value::Object(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn opt<T>(entries: &mut Vec<(&'static str, Value)>, key: &'static str, v: Option<T>, f: impl FnOnce(T) -> Value) {
    if let Some(v) = v {
        entries.push((key, f(v)));
    }
}

/// Structural comparison. Objects must have the same keys in the same order, except objects whose path matches one
/// of `records` (records: field order is not iterated by the JS). Numbers compare bit for bit.
struct Cmp {
    errs: Vec<String>,
}

fn matches(path: &[String], pattern: &str) -> bool {
    let segs: Vec<&str> = pattern.split('.').collect();
    segs.len() == path.len() && segs.iter().zip(path).all(|(s, p)| *s == "*" || s == p)
}

impl Cmp {
    fn new() -> Cmp {
        Cmp { errs: Vec::new() }
    }

    fn check(&mut self, path: &[String], js: &Value, rs: &Value, records: &[&str]) {
        let at = || path.join(".");
        if let (Some(a), Some(b)) = (num(js), num(rs)) {
            if !((a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()) {
                self.errs.push(format!("{}: js {js} rust {rs}", at()));
            }
            return;
        }
        match (js, rs) {
            (Value::Array(a), Value::Array(b)) => {
                if a.len() != b.len() {
                    self.errs.push(format!("{}: js len {} rust len {} (js {js}, rust {rs})", at(), a.len(), b.len()));
                    return;
                }
                for (i, (x, y)) in a.iter().zip(b).enumerate() {
                    let mut p = path.to_vec();
                    p.push(i.to_string());
                    self.check(&p, x, y, records);
                }
            }
            (Value::Object(a), Value::Object(b)) => {
                let ka: Vec<&String> = a.keys().collect();
                let kb: Vec<&String> = b.keys().collect();
                let record = records.iter().any(|r| matches(path, r));
                let same = if record {
                    let mut sa = ka.clone();
                    let mut sb = kb.clone();
                    sa.sort();
                    sb.sort();
                    sa == sb
                } else {
                    ka == kb
                };
                if !same {
                    self.errs.push(format!("{}: keys js {ka:?} rust {kb:?}", at()));
                    return;
                }
                for (k, x) in a {
                    let mut p = path.to_vec();
                    p.push(k.clone());
                    self.check(&p, x, &b[k], records);
                }
            }
            _ => {
                if js != rs {
                    self.errs.push(format!("{}: js {js} rust {rs}", at()));
                }
            }
        }
    }

    /// Compare a JS module dump with the Rust entries: every JS export must be ported (or listed in `skip`).
    fn module(&mut self, name: &str, js: &Value, rust: Vec<(&str, Value)>, skip: &[&str], records: &[&str]) {
        let js = js.as_object().unwrap();
        for k in js.keys() {
            if !skip.contains(&k.as_str()) && !rust.iter().any(|(r, _)| r == k) {
                self.errs.push(format!("{name}.{k}: exported by the JS but not compared"));
            }
        }
        for (k, v) in &rust {
            match js.get(*k) {
                Some(j) => self.check(&[name.to_string(), k.to_string()], j, v, records),
                None => self.errs.push(format!("{name}.{k}: not in the JS dump")),
            }
        }
    }

    fn finish(self, what: &str) {
        for e in self.errs.iter().take(50) {
            eprintln!("{e}");
        }
        assert!(self.errs.is_empty(), "{what}: {} mismatches", self.errs.len());
    }
}

fn root(s: &str) -> Vec<String> {
    vec![s.to_string()]
}

// ---------------------------------------------------------------------------------------------------------------
// Config tables
// ---------------------------------------------------------------------------------------------------------------

fn potato(p: &c::PotatoBook) -> Value {
    obj(vec![("id", json!(p.id)), ("max", n(p.max)), ("damage", n(p.damage)), ("strength", n(p.strength))])
}

fn constants() -> Vec<(&'static str, Value)> {
    let blessings = Value::Object(
        c::BLESSINGS
            .iter()
            .map(|b| (b.name.to_string(), obj(vec![("level", n(b.level as f64)), ("stats", stat_names(b.stats))])))
            .collect(),
    );
    vec![
        ("ACCESSORY_CATEGORY", json!(c::ACCESSORY_CATEGORY)),
        ("ARCHER_ARROW_DAMAGE_PER_LEVEL", n(c::ARCHER_ARROW_DAMAGE_PER_LEVEL)),
        ("ARMOR_REFORGE", json!(c::ARMOR_REFORGE)),
        ("ARMOR_SLOTS", strs(&c::ARMOR_SLOTS)),
        ("ARROW_DAMAGE", obj(vec![("arch", n(c::arrow_damage(Class::Arch))), ("bers", n(c::arrow_damage(Class::Bers)))])),
        ("ART_OF_WAR", obj(vec![("id", json!(c::ART_OF_WAR_ID)), ("strength", n(c::ART_OF_WAR_STRENGTH))])),
        ("ATTACK_SPEED_CAP", n(c::ATTACK_SPEED_CAP)),
        ("AUCTION_REFRESH_MS", n(c::AUCTION_REFRESH_MS as f64)),
        ("AUCTION_SMOOTH_DOWNLOADS", n(c::AUCTION_SMOOTH_DOWNLOADS as f64)),
        ("BASE_CRIT_CHANCE", n(c::BASE_CRIT_CHANCE)),
        ("BASE_DAMAGE", n(c::BASE_DAMAGE)),
        ("BASE_SPEED", n(c::BASE_SPEED)),
        ("BAZAAR_REFRESH_MS", n(c::BAZAAR_REFRESH_MS as f64)),
        ("BERSERK_DAMAGE_BASE", n(c::BERSERK_DAMAGE_BASE)),
        ("BERSERK_DAMAGE_PER_LEVEL", n(c::BERSERK_DAMAGE_PER_LEVEL)),
        ("BLACKSMITH_REFORGE_COST", n(c::BLACKSMITH_REFORGE_COST)),
        ("BLESSINGS", blessings),
        ("BLESSING_FLAT_PER_LEVEL", n(c::BLESSING_FLAT_PER_LEVEL)),
        ("BLESSING_FLOOR_BUFF", n(c::BLESSING_FLOOR_BUFF)),
        (
            "BLESSING_LIMITS",
            obj(vec![
                ("power", obj(vec![("min", n(c::BLESSING_POWER_MIN as f64)), ("max", n(c::BLESSING_POWER_MAX as f64))])),
                ("wisdom", obj(vec![("min", n(c::BLESSING_WISDOM_MIN as f64)), ("max", n(c::BLESSING_WISDOM_MAX as f64))])),
                ("time", obj(vec![("values", nums(&c::BLESSING_TIME_VALUES.map(|x| x as f64)))])),
            ]),
        ),
        ("BLESSING_PERCENT_PER_LEVEL", n(c::BLESSING_PERCENT_PER_LEVEL)),
        ("BOSS_SWARM_ENEMIES", n(c::BOSS_SWARM_ENEMIES)),
        ("CATA_STAT_BONUS", nums(&c::CATA_STAT_BONUS)),
        ("CATA_XP_PER_LEVEL", nums(&c::CATA_XP_PER_LEVEL)),
        ("CHIMERA_PER_LEVEL", n(c::CHIMERA_PER_LEVEL)),
        (
            "CLASSES",
            Value::Object(
                c::CLASSES
                    .iter()
                    .map(|ci| {
                        (
                            ci.key.to_string(),
                            obj(vec![
                                ("label", json!(ci.label)),
                                ("weights", obj(vec![("wither", n(ci.weights.wither)), ("dragon", n(ci.weights.dragon))])),
                            ]),
                        )
                    })
                    .collect(),
            ),
        ),
        (
            "CLASS_WEAPON_CATEGORIES",
            Value::Object(Class::ALL.iter().map(|&cl| (cl.js_name().to_string(), strs(c::class_weapon_categories(cl)))).collect()),
        ),
        ("CLEAR_BOOTS_CLASSES", classes(&c::CLEAR_BOOTS_CLASSES)),
        ("COMBAT_CRIT_CHANCE_PER_LEVEL", n(c::COMBAT_CRIT_CHANCE_PER_LEVEL)),
        ("COMBAT_MAX_LEVEL", n(c::COMBAT_MAX_LEVEL as f64)),
        ("DEFAULT_MAXED_COLLECTIONS", n(c::DEFAULT_MAXED_COLLECTIONS)),
        ("DOUBLESHOT_BASE", n(c::DOUBLESHOT_BASE)),
        ("DOUBLESHOT_PER_LEVEL", n(c::DOUBLESHOT_PER_LEVEL)),
        ("DUAL_WIELD_ADDITIVE", n(c::DUAL_WIELD_ADDITIVE)),
        ("DUAL_WIELD_WEAPONS", strs(&c::DUAL_WIELD_WEAPONS)),
        ("DUNGEON_BASE", n(c::DUNGEON_BASE)),
        ("DUNGEON_DROP_BOOST", n(c::DUNGEON_DROP_BOOST)),
        ("DUNGEON_FLOOR_CATA", nums(&c::DUNGEON_FLOOR_CATA)),
        ("DUNGEON_POTION", st(c::DUNGEON_POTION)),
        ("ENCHANTING_ABILITY_DAMAGE_PER_LEVEL", n(c::ENCHANTING_ABILITY_DAMAGE_PER_LEVEL)),
        ("ENCHANTING_MAX_LEVEL", n(c::ENCHANTING_MAX_LEVEL as f64)),
        ("EQUIPMENT_SLOTS", strs(&c::EQUIPMENT_SLOTS)),
        ("EQUIPMENT_SLOT_OF", obj(vec![("BRACELET", json!(c::equipment_slot("BRACELET").unwrap()))])),
        ("EXTRA_SPEED", n(c::EXTRA_SPEED)),
        ("FABLED_MULTIPLIER", n(c::FABLED_MULTIPLIER)),
        ("FEROCITY_EFFECTIVENESS", n(c::FEROCITY_EFFECTIVENESS)),
        ("FUMING_POTATO", potato(&c::FUMING_POTATO)),
        ("GENERAL_MEDALLION_ID", json!(c::GENERAL_MEDALLION_ID)),
        ("GLOBAL_WEIGHT", n(c::GLOBAL_WEIGHT)),
        ("HOT_POTATO", potato(&c::HOT_POTATO)),
        ("IMPLOSION_BELT_ID", json!(c::IMPLOSION_BELT_ID)),
        ("IMPLOSION_BELT_MULTIPLIER", n(c::IMPLOSION_BELT_MULTIPLIER)),
        ("ITEMS_REFRESH_MS", n(c::ITEMS_REFRESH_MS as f64)),
        ("LEGION_GROUPED_PLAYERS", n(c::LEGION_GROUPED_PLAYERS)),
        ("LEGION_PER_LEVEL", n(c::LEGION_PER_LEVEL)),
        ("LEGION_PLAYERS", n(c::LEGION_PLAYERS)),
        ("LOVING_ABILITY_MULTIPLIER", n(c::LOVING_ABILITY_MULTIPLIER)),
        ("MAGE_ABILITY_DAMAGE_BASE", n(c::MAGE_ABILITY_DAMAGE_BASE)),
        ("MAGE_ABILITY_DAMAGE_PER_5_LEVELS", n(c::MAGE_ABILITY_DAMAGE_PER_5_LEVELS)),
        ("MAGE_BEAM_BASE", n(c::MAGE_BEAM_BASE)),
        ("MAGE_BEAM_PER_INTELLIGENCE", n(c::MAGE_BEAM_PER_INTELLIGENCE)),
        ("MAGE_BEAM_TICKS", n(c::MAGE_BEAM_TICKS)),
        ("MAGE_COOLDOWN_REDUCTION_MAX", n(c::MAGE_COOLDOWN_REDUCTION_MAX)),
        ("MAGE_COOLDOWN_REDUCTION_PER_LEVEL", n(c::MAGE_COOLDOWN_REDUCTION_PER_LEVEL)),
        ("MAGE_FIRST_STRIKE_DPS", n(c::MAGE_FIRST_STRIKE_DPS)),
        ("MAGE_INT_BASE", n(c::MAGE_INT_BASE)),
        ("MAGE_INT_PER_LEVEL", n(c::MAGE_INT_PER_LEVEL)),
        ("MASTER_FLOOR_CATA", nums(&c::MASTER_FLOOR_CATA)),
        ("MASTER_STAR_IDS", strs(&c::MASTER_STAR_IDS)),
        ("MASTER_STAR_STEP", n(c::MASTER_STAR_STEP)),
        ("MAXOR_BOOTS_ID", json!(c::MAXOR_BOOTS_ID)),
        ("MAX_MASTER_STARS", n(c::MAX_MASTER_STARS)),
        ("MAX_STARS", n(c::MAX_STARS)),
        ("MAX_UPGRADES_LISTED", n(c::MAX_UPGRADES_LISTED as f64)),
        ("NECRON_BLADE_WITHER_MULTIPLIER", n(c::NECRON_BLADE_WITHER_MULTIPLIER)),
        ("NECRON_BOOTS_ID", json!(c::NECRON_BOOTS_ID)),
        ("OVERLOAD_MULTIPLIER", n(c::OVERLOAD_MULTIPLIER)),
        ("PROFILE_CACHE_MS", n(c::PROFILE_CACHE_MS as f64)),
        ("RAGNAROCK_ID", json!(c::RAGNAROCK_ID)),
        ("RAGNAROCK_MULTIPLIER", n(c::RAGNAROCK_MULTIPLIER)),
        ("RARITIES", strs(&c::RARITIES)),
        ("RCM_ENCHANTS", strs(&c::RCM_ENCHANTS)),
        ("RCM_SWARM_ENEMIES", n(c::RCM_SWARM_ENEMIES)),
        ("RECOMBOBULATOR_ID", json!(c::RECOMBOBULATOR_ID)),
        ("SA_CLOAK_COOLDOWN_S", n(c::SA_CLOAK_COOLDOWN_S)),
        ("SA_CLOAK_IDS", strs(&c::SA_CLOAK_IDS)),
        ("SA_CLOAK_MULTIPLIER", n(c::SA_CLOAK_MULTIPLIER)),
        ("SECRET_DIGIT_CAP", n(c::SECRET_DIGIT_CAP)),
        ("SECRET_STEP", n(c::SECRET_STEP)),
        ("SKELETON_MASTER_CHESTS", strs(&c::SKELETON_MASTER_CHESTS)),
        ("SKELETON_MASTER_CHEST_CATA", n(c::SKELETON_MASTER_CHEST_CATA)),
        ("SKELETON_MASTER_CHEST_CLASSES", classes(&c::SKELETON_MASTER_CHEST_CLASSES)),
        ("SKILL_XP_PER_LEVEL", nums(&c::SKILL_XP_PER_LEVEL)),
        ("SLAYER_XP", Value::Object(c::SLAYER_XP.iter().map(|(k, v)| (k.to_string(), nums(v))).collect())),
        ("SOLO_CLASS_MULTIPLIER", n(c::SOLO_CLASS_MULTIPLIER)),
        ("SPECIAL_RARITIES", strs(&c::SPECIAL_RARITIES)),
        ("SPEED_CAP", n(c::SPEED_CAP)),
        ("SPEED_CAP_BY_CLASS", obj(vec![("mage", n(c::speed_cap(Class::Mage, Mode::Lcm)))])),
        ("STAR_STEP", n(c::STAR_STEP)),
        (
            "TARGET_DEFENSE",
            obj(vec![
                ("wither", n(c::target_defense(dps_core::Target::Wither))),
                ("dragon", n(c::target_defense(dps_core::Target::Dragon))),
            ]),
        ),
        ("THUNDERLORD_EVERY", n(c::THUNDERLORD_EVERY)),
        ("TICKS_PER_SECOND", n(c::TICKS_PER_SECOND)),
        ("ULTIMATE_WITHER_SCROLL", json!(c::ULTIMATE_WITHER_SCROLL)),
        ("WARRIOR_PER_LEVEL", n(c::WARRIOR_PER_LEVEL)),
        ("WARRIOR_PER_LEVEL_AFTER_50", n(c::WARRIOR_PER_LEVEL_AFTER_50)),
        ("WITHER_IMPACT_BASE", n(c::WITHER_IMPACT_BASE)),
        ("WITHER_IMPACT_SCALING", n(c::WITHER_IMPACT_SCALING)),
        ("WITHER_IMPACT_WEAPONS", strs(&c::WITHER_IMPACT_WEAPONS)),
        ("WITHER_SCROLLS", strs(&c::WITHER_SCROLLS)),
        ("WOLF_SLAYER_SPEED_LEVELS", nums(&c::WOLF_SLAYER_SPEED_LEVELS.map(|x| x as f64))),
        ("WOLF_SLAYER_XP", nums(&c::WOLF_SLAYER_XP)),
    ]
}

fn enchant_value(e: &en::Enchant) -> Value {
    let mut v: Vec<(&'static str, Value)> = Vec::new();
    opt(&mut v, "name", e.name, |x| json!(x));
    v.push(("categories", strs(e.categories)));
    v.push(("target", json!(e.target.js_name())));
    opt(&mut v, "floorMode", e.floor_mode, |x: FloorMode| json!(x.js_name()));
    if !e.stats.is_empty() {
        v.push(("stats", Value::Object(e.stats.iter().map(|(s, vals)| (s.js_name().to_string(), nums(vals))).collect())));
    }
    if !e.conflicts.is_empty() {
        v.push(("conflicts", strs(e.conflicts)));
    }
    opt(&mut v, "classes", e.classes, classes);
    opt(&mut v, "items", e.items, strs);
    for (key, b) in [("melee", e.melee), ("swarm", e.swarm), ("clearMobs", e.clear_mobs), ("legion", e.legion), ("chimera", e.chimera)] {
        if b {
            v.push((key, json!(true)));
        }
    }
    opt(&mut v, "thunder", e.thunder, nums);
    opt(&mut v, "fire", e.fire, nums);
    opt(&mut v, "theOne", e.the_one, nums);
    opt(&mut v, "minLevel", e.min_level, |x| n(x as f64));
    opt(&mut v, "maxLevel", e.max_level, |x| n(x as f64));
    obj(v)
}

fn enchants() -> Vec<(&'static str, Value)> {
    vec![
        ("ENCHANTS", Value::Object(en::ENCHANTS.iter().map(|e| (e.key.to_string(), enchant_value(e))).collect())),
        ("ENCHANT_TABLE_TWO_BOTTLES", n(en::ENCHANT_TABLE_TWO_BOTTLES)),
        (
            "ENCHANT_UPGRADE_ITEMS",
            Value::Object(
                en::ENCHANT_UPGRADE_ITEMS
                    .iter()
                    .map(|u| (u.enchant.to_string(), obj(vec![("level", n(u.level as f64)), ("id", json!(u.id)), ("name", json!(u.name))])))
                    .collect(),
            ),
        ),
        ("GRAND_EXP_BOTTLE_ID", json!(en::GRAND_EXP_BOTTLE_ID)),
        ("TABLE_ENCHANT_XP", Value::Object(en::TABLE_ENCHANT_XP.iter().map(|(k, v)| (k.to_string(), nums(v))).collect())),
    ]
}

fn by_class<T>(f: impl Fn(Class) -> T, g: impl Fn(T) -> Value) -> Value {
    Value::Object(Class::ALL.iter().map(|&cl| (cl.js_name().to_string(), g(f(cl)))).collect())
}

fn accessories() -> Vec<(&'static str, Value)> {
    let rarities = ["COMMON", "UNCOMMON", "RARE", "EPIC", "LEGENDARY", "MYTHIC", "DIVINE", "SPECIAL", "VERY_SPECIAL"];
    vec![
        ("AP_BY_RARITY", Value::Object(rarities.iter().map(|&r| (r.to_string(), n(acc::ap_by_rarity(r).unwrap()))).collect())),
        ("AP_OVERRIDES", obj(vec![(acc::RIFT_PRISM_ID, n(acc::ap_override(acc::RIFT_PRISM_ID).unwrap()))])),
        ("AP_PER_TUNING_POINT", n(acc::AP_PER_TUNING_POINT)),
        ("BEST_ENRICHMENT", by_class(acc::best_enrichment, |x| json!(x))),
        ("CLASS_ENRICHMENTS", by_class(acc::class_enrichments, strs)),
        ("CLASS_TUNING_STATS", by_class(acc::class_tuning_stats, stat_names)),
        ("DEFAULT_POWER", by_class(acc::default_power, |x| json!(x))),
        ("DEFAULT_TUNING_SPLIT", by_class(acc::default_tuning_split, st)),
        ("DOUBLE_AP_ITEMS", strs(&acc::DOUBLE_AP_ITEMS)),
        ("DUNGEON_AP_MULTIPLIER", n(acc::DUNGEON_AP_MULTIPLIER)),
        ("ENRICHABLE_RARITIES", strs(&acc::ENRICHABLE_RARITIES)),
        (
            "ENRICHMENTS",
            Value::Object(acc::ENRICHMENTS.iter().map(|e| (e.kind.to_string(), obj(vec![("key", json!(e.key)), ("stats", st(e.stats))]))).collect()),
        ),
        ("ENRICHMENT_SWAPPER_ID", json!(acc::ENRICHMENT_SWAPPER_ID)),
        ("FREE_POWERS", strs(&acc::FREE_POWERS)),
        ("NO_RECOMB", strs(&acc::NO_RECOMB)),
        (
            "POWERS",
            Value::Object(
                acc::POWERS
                    .iter()
                    .map(|p| {
                        let mut v = vec![("stats", st(p.stats))];
                        // stone() always sets `bonus` (default {}); the starter powers have none.
                        if !acc::FREE_POWERS.contains(&p.key) {
                            v.push(("bonus", st(p.bonus)));
                        }
                        (p.key.to_string(), obj(v))
                    })
                    .collect(),
            ),
        ),
        ("POWER_EXPONENT", n(acc::POWER_EXPONENT)),
        ("POWER_LOG_FACTOR", n(acc::POWER_LOG_FACTOR)),
        ("POWER_SCALE", n(acc::POWER_SCALE)),
        ("POWER_STONES", Value::Object(acc::POWER_STONES.iter().map(|(k, v)| (k.to_string(), json!(v))).collect())),
        ("RCM_TUNING_STATS", stat_names(&acc::RCM_TUNING_STATS)),
        ("RIFT_PRISM_ID", json!(acc::RIFT_PRISM_ID)),
        ("TUNING_KEYS", Value::Object(acc::TUNING_KEYS.iter().map(|(k, v)| (k.to_string(), json!(v.js_name()))).collect())),
        ("TUNING_PER_POINT", st(acc::TUNING_PER_POINT)),
    ]
}

fn part(p: &pc::Part) -> Value {
    obj(vec![("id", json!(p.id)), ("qty", n(p.qty))])
}

fn pets_config() -> Vec<(&'static str, Value)> {
    vec![
        ("BUY_RARITIES", strs(&pc::BUY_RARITIES)),
        ("EXCLUDED_PETS", strs(&pc::EXCLUDED_PETS)),
        ("GOLDEN_DRAGON_BANK_CAP", n(pc::GOLDEN_DRAGON_BANK_CAP)),
        ("GOLD_COLLECTION_DIGITS", n(pc::GOLD_COLLECTION_DIGITS)),
        ("KAT_FLOWER_ID", json!(pc::KAT_FLOWER_ID)),
        ("KAT_LEVEL_DISCOUNT", n(pc::KAT_LEVEL_DISCOUNT)),
        (
            "KAT_UPGRADES",
            Value::Object(
                pc::KAT_UPGRADES
                    .iter()
                    .map(|k| {
                        let inner = obj(vec![
                            ("to", json!(k.to)),
                            ("coins", n(k.coins)),
                            ("items", Value::Array(k.items.iter().map(part).collect())),
                        ]);
                        (k.kind.to_string(), obj(vec![(k.tier, inner)]))
                    })
                    .collect(),
            ),
        ),
        (
            "PET_ITEMS",
            Value::Object(
                pc::PET_ITEMS
                    .iter()
                    .map(|p| {
                        let mut v = vec![("name", json!(p.name))];
                        if !p.mult.is_empty() {
                            v.push(("mult", st(p.mult)));
                        }
                        if !p.flat.is_empty() {
                            v.push(("flat", st(p.flat)));
                        }
                        (p.id.to_string(), obj(v))
                    })
                    .collect(),
            ),
        ),
        (
            "PHASE_PETS",
            obj(vec![
                (
                    "wither",
                    Value::Object(
                        Class::ALL
                            .iter()
                            .filter_map(|&cl| pc::phase_pet(dps_core::Target::Wither, cl).map(|p| (cl.js_name().to_string(), json!(p))))
                            .collect(),
                    ),
                ),
                (
                    "dragon",
                    Value::Object(
                        Class::ALL
                            .iter()
                            .filter_map(|&cl| pc::phase_pet(dps_core::Target::Dragon, cl).map(|p| (cl.js_name().to_string(), json!(p))))
                            .collect(),
                    ),
                ),
            ]),
        ),
        ("TIER_BOOST_ID", json!(pc::TIER_BOOST_ID)),
    ]
}

fn shards() -> Vec<(&'static str, Value)> {
    let rarities = ["COMMON", "UNCOMMON", "RARE", "EPIC", "LEGENDARY"];
    vec![
        (
            "SHARDS",
            Value::Object(
                sh::SHARDS
                    .iter()
                    .map(|s| {
                        let mut v = vec![("name", json!(s.name)), ("attribute", json!(s.attribute)), ("rarity", json!(s.rarity))];
                        opt(&mut v, "family", s.family, |x| json!(x.js_name()));
                        opt(&mut v, "echo", s.echo, |x| json!(x.js_name()));
                        opt(&mut v, "target", s.target, |x: StatTarget| json!(x.js_name()));
                        opt(&mut v, "classes", s.classes, classes);
                        v.push(("perLevel", st(s.per_level)));
                        (s.id.to_string(), obj(v))
                    })
                    .collect(),
            ),
        ),
        ("SHARDS_CUMULATIVE", Value::Object(rarities.iter().map(|&r| (r.to_string(), nums(sh::shards_cumulative(r).unwrap()))).collect())),
        ("SHARD_HUNTING_REQ", Value::Object(rarities.iter().map(|&r| (r.to_string(), n(sh::shard_hunting_req(r).unwrap()))).collect())),
        ("SHARD_MAX_LEVEL", n(sh::SHARD_MAX_LEVEL)),
    ]
}

fn essence() -> Vec<(&'static str, Value)> {
    vec![
        ("ESSENCE_MIN_CATA", n(es::ESSENCE_MIN_CATA)),
        (
            "ESSENCE_PERKS",
            Value::Object(
                es::ESSENCE_PERKS
                    .iter()
                    .map(|p| {
                        let mut v = vec![("name", json!(p.name)), ("essence", json!(p.essence))];
                        opt(&mut v, "pet", p.pet, |x| json!(x));
                        v.push(("perLevel", st(p.per_level)));
                        v.push(("costs", nums(p.costs)));
                        if p.profile_only {
                            v.push(("profileOnly", json!(true)));
                        }
                        (p.key.to_string(), obj(v))
                    })
                    .collect(),
            ),
        ),
    ]
}

fn gemstones() -> Vec<(&'static str, Value)> {
    vec![
        (
            "GEMSTONES",
            Value::Object(
                gm::GEMSTONES
                    .iter()
                    .map(|g| {
                        let values = Value::Object(g.values.iter().map(|(q, v)| (q.to_string(), nums(v))).collect());
                        (g.gem.to_string(), obj(vec![("stat", json!(g.stat.js_name())), ("values", values)]))
                    })
                    .collect(),
            ),
        ),
        ("GEM_QUALITIES", strs(&gm::GEM_QUALITIES)),
        (
            "SLOT_GEMS",
            Value::Object(["COMBAT", "OFFENSIVE", "DEFENSIVE", "UNIVERSAL"].iter().map(|&t| (t.to_string(), strs(gm::slot_gems(t).unwrap()))).collect()),
        ),
        ("UPGRADE_QUALITIES", strs(&gm::UPGRADE_QUALITIES)),
    ]
}

fn armor_sets() -> Vec<(&'static str, Value)> {
    vec![(
        "ARMOR_SETS",
        Value::Array(
            ar::ARMOR_SETS
                .iter()
                .map(|s| {
                    let mut v = vec![("name", json!(s.name)), ("pieces", strs(s.pieces))];
                    if !s.per_piece.is_empty() {
                        v.push(("perPiece", st(s.per_piece)));
                    }
                    if !s.full.is_empty() {
                        v.push(("full", st(s.full)));
                    }
                    opt(&mut v, "classes", s.classes, classes);
                    obj(v)
                })
                .collect(),
        ),
    )]
}

fn item_stats() -> Vec<(&'static str, Value)> {
    let misc = |cl: Class| {
        let m = ist::misc_buffs(cl);
        obj(vec![("stats", st(m.stats)), ("wither", st(m.wither)), ("dragon", st(m.dragon))])
    };
    vec![
        (
            "ITEM_STATS",
            Value::Object(
                ist::ITEM_STATS
                    .iter()
                    .map(|d| {
                        let mut v: Vec<(&'static str, Value)> = Vec::new();
                        if d.replace_stats {
                            v.push(("replaceStats", json!(true)));
                        }
                        opt(&mut v, "dungeon", d.dungeon, |x| json!(x));
                        if !d.stats.is_empty() {
                            v.push(("stats", st(d.stats)));
                        }
                        if !d.wither.is_empty() {
                            v.push(("wither", st(d.wither)));
                        }
                        if !d.dragon.is_empty() {
                            v.push(("dragon", st(d.dragon)));
                        }
                        opt(&mut v, "classes", d.classes, classes);
                        opt(&mut v, "arrows", d.arrows, n);
                        opt(&mut v, "shotTicks", d.shot_ticks, n);
                        opt(&mut v, "statMultiplier", d.stat_multiplier, n);
                        if !d.per_cata_level.is_empty() {
                            v.push(("perCataLevel", st(d.per_cata_level)));
                        }
                        opt(&mut v, "note", d.note, |x| json!(x));
                        if d.master_mode_only {
                            v.push(("masterModeOnly", json!(true)));
                        }
                        (d.id.to_string(), obj(v))
                    })
                    .collect(),
            ),
        ),
        ("MISC_BUFFS", by_class(|cl| cl, misc)),
        ("REFORGE_OVERRIDES", obj(vec![])),
    ]
}

fn item_upgrades() -> Vec<(&'static str, Value)> {
    vec![(
        "ITEM_UPGRADES",
        Value::Object(
            iu::ITEM_UPGRADES
                .iter()
                .map(|(from, ups)| {
                    let list = ups
                        .iter()
                        .map(|u| obj(vec![("id", json!(u.id)), ("parts", Value::Array(u.parts.iter().map(part).collect()))]))
                        .collect();
                    (from.to_string(), Value::Array(list))
                })
                .collect(),
        ),
    )]
}

fn profile() -> Vec<(&'static str, Value)> {
    let level_map = |steps: &[(f64, &[(Stat, f64)])]| {
        Value::Object(steps.iter().map(|(l, s)| (dps_core::jsfmt::js_string(*l), st(s))).collect())
    };
    vec![
        ("BASE_CRIT_DAMAGE", n(pr::BASE_CRIT_DAMAGE)),
        ("BOOK_DAMAGE_OPTION", json!(pr::BOOK_DAMAGE_OPTION)),
        ("BOOK_DAMAGE_PER_LEVEL", n(pr::BOOK_DAMAGE_PER_LEVEL)),
        ("BOOK_DAMAGE_UNLOCK_LEVEL", n(pr::BOOK_DAMAGE_UNLOCK_LEVEL)),
        ("BOOK_MAX_LEVEL", n(pr::BOOK_MAX_LEVEL)),
        ("BOOK_OF_PROGRESSION_ID", json!(pr::BOOK_OF_PROGRESSION_ID)),
        ("BOOK_SWAP_COINS_PER_LEVEL", n(pr::BOOK_SWAP_COINS_PER_LEVEL)),
        ("CAKE_STATS", Value::Object(pr::CAKE_STATS.iter().map(|(k, s)| (k.to_string(), json!(s.js_name()))).collect())),
        (
            "HARP_SONGS",
            Value::Object(
                pr::HARP_SONGS
                    .iter()
                    .map(|h| (h.key.to_string(), obj(vec![("name", json!(h.name)), ("intelligence", n(h.intelligence))])))
                    .collect(),
            ),
        ),
        ("SB_LEVEL_XP", n(pr::SB_LEVEL_XP)),
        ("SB_STRENGTH_EVERY", n(pr::SB_STRENGTH_EVERY)),
        (
            "SKILL_BONUSES",
            Value::Object(
                pr::SKILL_BONUSES
                    .iter()
                    .map(|s| (s.skill.to_string(), obj(vec![("max", n(s.max)), ("steps", level_map(s.steps))])))
                    .collect(),
            ),
        ),
        ("SLAYER_BONUSES", Value::Object(pr::SLAYER_BONUSES.iter().map(|s| (s.boss.to_string(), level_map(s.rewards))).collect())),
        ("SLAYER_MAX_LEVEL", n(pr::SLAYER_MAX_LEVEL)),
    ]
}

fn extra_upgrades() -> Vec<(&'static str, Value)> {
    vec![(
        "EXTRA_UPGRADES",
        Value::Array(
            up::EXTRA_UPGRADES
                .iter()
                .map(|e| {
                    let mut v = vec![("key", json!(e.key)), ("name", json!(e.name))];
                    opt(&mut v, "group", e.group, |x| json!(x));
                    opt(&mut v, "stats", e.stats, st);
                    opt(&mut v, "wither", e.wither, st);
                    opt(&mut v, "dragon", e.dragon, st);
                    let cost = e
                        .cost
                        .iter()
                        .map(|p| {
                            let mut pv = vec![("id", json!(p.id))];
                            opt(&mut pv, "qty", p.qty, n);
                            obj(pv)
                        })
                        .collect();
                    v.push(("cost", Value::Array(cost)));
                    opt(&mut v, "owned", e.owned_items, |x| obj(vec![("items", strs(x))]));
                    opt(&mut v, "requires", e.requires, |x| json!(x));
                    opt(&mut v, "classes", e.classes, classes);
                    opt(&mut v, "activeWith", e.active_with, |x| json!(x));
                    obj(v)
                })
                .collect(),
        ),
    )]
}

#[test]
fn config_tables_match_js() {
    let Some(core) = core() else { return };
    let cfg = &core["config"];
    let mut cmp = Cmp::new();
    // Records: objects whose own field order the JS never iterates.
    let records = [
        "enchants.ENCHANTS.*",
        "enchants.ENCHANT_UPGRADE_ITEMS.*",
        "accessories.POWERS.*",
        "accessories.ENRICHMENTS.*",
        "pets.PET_ITEMS.*",
        "pets.KAT_UPGRADES.*.*",
        "shards.SHARDS.*",
        "essence.ESSENCE_PERKS.*",
        "armorSets.ARMOR_SETS.*",
        "itemStats.ITEM_STATS.*",
        "upgrades.EXTRA_UPGRADES.*",
        "constants.CLASSES.*",
        "constants.HOT_POTATO",
        "constants.FUMING_POTATO",
        "constants.ART_OF_WAR",
    ];
    cmp.module("constants", &cfg["constants"], constants(), &["CLASS_EXCLUDED_ITEMS"], &records);
    cmp.module("enchants", &cfg["enchants"], enchants(), &[], &records);
    cmp.module("accessories", &cfg["accessories"], accessories(), &[], &records);
    cmp.module("pets", &cfg["pets"], pets_config(), &["PET_PERKS"], &records);
    cmp.module("shards", &cfg["shards"], shards(), &[], &records);
    cmp.module("essence", &cfg["essence"], essence(), &[], &records);
    cmp.module("gemstones", &cfg["gemstones"], gemstones(), &[], &records);
    cmp.module("armorSets", &cfg["armorSets"], armor_sets(), &[], &records);
    cmp.module("itemStats", &cfg["itemStats"], item_stats(), &[], &records);
    cmp.module("itemUpgrades", &cfg["itemUpgrades"], item_upgrades(), &[], &records);
    cmp.module("profile", &cfg["profile"], profile(), &[], &records);
    cmp.module("upgrades", &cfg["upgrades"], extra_upgrades(), &[], &records);
    cmp.finish("config tables");
}

// ---------------------------------------------------------------------------------------------------------------
// Function-valued config
// ---------------------------------------------------------------------------------------------------------------

fn class_of(v: &Value) -> Class {
    Class::from_js(v.as_str().unwrap()).unwrap()
}

#[test]
fn config_functions_match_js() {
    let Some(core) = core() else { return };
    let calls = &core["calls"];
    let mut cmp = Cmp::new();
    for case in calls["blessingsWith"].as_array().unwrap() {
        let l = &case["levels"];
        let g = |k: &str| l.get(k).and_then(|v| v.as_f64());
        let b = c::blessings_with(g("power"), g("wisdom"), g("time"));
        let rs = obj(vec![("power", n(b.power as f64)), ("wisdom", n(b.wisdom as f64)), ("time", n(b.time as f64))]);
        cmp.check(&root("blessingsWith"), &case["out"], &rs, &[]);
    }
    for case in calls["speedCap"].as_array().unwrap() {
        let rs = n(c::speed_cap(class_of(&case["cls"]), Mode::from_js(case["mode"].as_str().unwrap()).unwrap()));
        cmp.check(&root("speedCap"), &case["out"], &rs, &[]);
    }
    for case in calls["equipmentSlot"].as_array().unwrap() {
        let rs = match case["category"].as_str() {
            Some(cat) => c::equipment_slot(cat).map(|s| json!(s)).unwrap_or(Value::Null),
            None => Value::Null,
        };
        cmp.check(&root("equipmentSlot"), &case["out"], &rs, &[]);
    }
    let ids: Vec<&str> = resource().keys().map(|s| s.as_str()).collect();
    let rs = strs(&ids.iter().copied().filter(|id| c::is_max_reforge_item(id)).collect::<Vec<_>>());
    cmp.check(&root("maxReforgeItem"), &calls["maxReforgeItem"], &rs, &[]);
    for case in calls["classExcluded"].as_array().unwrap() {
        let cl = class_of(&case["cls"]);
        let rs = strs(&ids.iter().copied().filter(|id| c::class_excluded_item(cl, id)).collect::<Vec<_>>());
        cmp.check(&root("classExcluded"), &case["ids"], &rs, &[]);
    }
    let rs = Value::Object(en::ENCHANTS.iter().map(|e| (e.key.to_string(), n(e.max_level() as f64))).collect());
    cmp.check(&root("enchantMaxLevel"), &calls["enchantMaxLevel"], &rs, &[]);
    for case in calls["tuningStats"].as_array().unwrap() {
        let rs = stat_names(acc::tuning_stats(class_of(&case["cls"]), Mode::from_js(case["mode"].as_str().unwrap()).unwrap()));
        cmp.check(&root("tuningStats"), &case["out"], &rs, &[]);
    }
    for case in calls["enrichmentKinds"].as_array().unwrap() {
        let rs = strs(acc::enrichment_kinds(class_of(&case["cls"]), Mode::from_js(case["mode"].as_str().unwrap()).unwrap()));
        cmp.check(&root("enrichmentKinds"), &case["out"], &rs, &[]);
    }
    let rs = strs(&ids.iter().copied().filter(|id| acc::is_hatcessory(id)).collect::<Vec<_>>());
    cmp.check(&root("hatcessory"), &calls["hatcessory"], &rs, &[]);
    for case in calls["itemRarity"].as_array().unwrap() {
        let rs = c::item_rarity(case["baseRarity"].as_str().unwrap(), case["recomb"].as_bool().unwrap(), case["id"].as_str().unwrap());
        cmp.check(&root(&format!("itemRarity{case}")), &case["out"], &json!(rs), &[]);
    }
    for row in calls["statRarity"].as_array().unwrap() {
        cmp.check(&root("statRarity"), &row[1], &json!(c::stat_rarity(row[0].as_str().unwrap())), &[]);
    }
    for row in calls["isWeaponCategory"].as_array().unwrap() {
        let rs = row[0].as_str().is_some_and(c::is_weapon_category);
        cmp.check(&root(&format!("isWeaponCategory{}", row[0])), &row[1], &json!(rs), &[]);
    }
    for case in calls["gemStat"].as_array().unwrap() {
        let rs = match gm::gem_stat(case["gem"].as_str().unwrap(), case["quality"].as_str().unwrap(), case["rarity"].as_str().unwrap()) {
            Some((s, v)) => st(&[(s, v)]),
            None => Value::Null,
        };
        cmp.check(&root("gemStat"), &case["out"], &rs, &[]);
    }
    cmp.finish("config functions");
}

// ---------------------------------------------------------------------------------------------------------------
// Data files
// ---------------------------------------------------------------------------------------------------------------

fn reforge_value(r: &data::Reforge) -> Value {
    let mut v = vec![("name", json!(r.name)), ("stone", r.stone.map(|s| json!(s)).unwrap_or(Value::Null)), ("types", strs(r.types))];
    opt(&mut v, "items", r.items, strs);
    v.push(("stats", Value::Object(r.stats.iter().map(|(rar, s)| (rar.to_string(), st(s))).collect())));
    opt(&mut v, "cataStat", r.cata_stat, |s| json!(s.js_name()));
    obj(v)
}

#[test]
fn data_files_match_js() {
    let Some(core) = core() else { return };
    let d = &core["data"];
    let mut cmp = Cmp::new();
    assert_eq!(data::reforge_dropped_keys(), 0, "reforges.json has stat keys Stat does not know");
    assert_eq!(data::pet_dropped_keys(), 0, "pets.json has stat keys Stat does not know");

    let order = strs(&data::all_reforges().iter().map(|r| r.key).collect::<Vec<_>>());
    cmp.check(&root("reforgeOrder"), &d["reforgeOrder"], &order, &[]);
    let reforges = Value::Object(data::all_reforges().iter().map(|r| (r.key.to_string(), reforge_value(r))).collect());
    cmp.check(&root("reforges"), &d["reforges"], &reforges, &["reforges.*"]);
    for (i, r) in data::all_reforges().iter().enumerate() {
        assert_eq!(r.index, i);
        assert!(std::ptr::eq(data::get_reforge(r.name).unwrap(), r), "{}", r.key);
    }
    for case in d["getReforge"].as_array().unwrap() {
        let rs = data::get_reforge(case["name"].as_str().unwrap()).map(|r| json!(r.key)).unwrap_or(Value::Null);
        cmp.check(&root("getReforge"), &case["out"], &rs, &[]);
    }
    let candidates: Vec<(String, Option<String>)> = d["reforgeCandidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["id"].as_str().unwrap().to_string(), c["category"].as_str().map(|s| s.to_string())))
        .collect();
    for case in d["reforgeAppliesTo"].as_array().unwrap() {
        let r = data::reforge_by_key(case["key"].as_str().unwrap()).unwrap();
        let ok: Vec<f64> = candidates
            .iter()
            .enumerate()
            .filter(|(_, (id, cat))| r.applies_to(id, cat.as_deref()))
            .map(|(i, _)| i as f64)
            .collect();
        cmp.check(&root(&format!("reforgeAppliesTo.{}", r.key)), &case["ok"], &nums(&ok), &[]);
    }
    for row in d["accessoryFamily"].as_array().unwrap() {
        let id = row[0].as_str().unwrap();
        let rs = json!([id, data::accessory_family(id), data::accessory_chain_position(id)]);
        cmp.check(&root("accessoryFamily"), row, &rs, &[]);
    }
    let fragged = Value::Object(
        data::all_fragged()
            .iter()
            .map(|f| (f.from.to_string(), obj(vec![("id", json!(f.id)), ("parts", Value::Array(f.parts.iter().map(part).collect()))])))
            .collect(),
    );
    cmp.check(&root("fragged"), &d["fragged"], &fragged, &[]);
    for f in data::all_fragged() {
        assert!(std::ptr::eq(data::fragged(f.from).unwrap(), f));
    }
    cmp.check(&root("petTypes"), &d["petTypes"], &strs(&pets::all_pet_types().collect::<Vec<_>>()), &[]);
    let tiers = Value::Object(pets::all_pet_types().map(|t| (t.to_string(), strs(&pets::pet_tiers(t).collect::<Vec<_>>()))).collect());
    cmp.check(&root("petTiers"), &d["petTiers"], &tiers, &[]);
    let max = Value::Object(
        pets::all_pet_types().chain(["NOT_A_PET"]).map(|t| (t.to_string(), n(pets::max_pet_level(t)))).collect(),
    );
    cmp.check(&root("maxPetLevel"), &d["maxPetLevel"], &max, &[]);
    for case in d["petLevel"].as_array().unwrap() {
        let rs = n(pets::pet_level(case["type"].as_str().unwrap(), case["tier"].as_str().unwrap(), case["exp"].as_f64().unwrap()));
        cmp.check(&root(&format!("petLevel.{case}")), &case["out"], &rs, &[]);
    }
    for case in d["effectiveTier"].as_array().unwrap() {
        let rs = json!(pets::effective_tier(case["tier"].as_str().unwrap(), case["heldItem"].as_str()));
        cmp.check(&root("effectiveTier"), &case["out"], &rs, &[]);
    }
    cmp.finish("data files");
}

// ---------------------------------------------------------------------------------------------------------------
// Pet stats
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn pet_stats_match_js() {
    let Some(core) = core() else { return };
    let mut cmp = Cmp::new();
    let cases = core["petStats"].as_array().unwrap();
    for case in cases {
        let pet = &case["pet"];
        let p = &case["profile"];
        let profile = PetProfile { bank: p.get("bank").and_then(|v| v.as_f64()), gold_collection: p.get("goldCollection").and_then(|v| v.as_f64()) };
        let got = pets::compute_pet_stats(
            pet["type"].as_str().unwrap(),
            pet["tier"].as_str().unwrap(),
            pet["heldItem"].as_str(),
            pet.get("level").and_then(|v| v.as_f64()),
            &profile,
        );
        let rs = serde_json::to_value(got).unwrap();
        cmp.check(&root(&format!("petStats[{pet} {p}]")), &case["out"], &rs, &[]);
    }
    eprintln!("{} pet stat cases", cases.len());
    cmp.finish("pet stats");
}

// ---------------------------------------------------------------------------------------------------------------
// Requirements, drops, cleanName
// ---------------------------------------------------------------------------------------------------------------

fn player_of(v: &Value) -> Option<Player> {
    if v.is_null() {
        return None;
    }
    let map = |m: &Value| -> Option<dps_core::model::Map<i32>> {
        m.as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_i64().unwrap() as i32)).collect())
    };
    Some(Player { cata_level: v["cataLevel"].as_f64().unwrap(), skills: map(&v["skills"]), slayers: map(&v["slayers"]) })
}

#[test]
fn requirements_drops_names_match_js() {
    let Some(core) = core() else { return };
    let res = resource();
    let mut cmp = Cmp::new();
    let with_reqs: Vec<&str> = res
        .iter()
        .filter(|(_, i)| i.requirements.is_some() || i.catacombs_requirements.is_some())
        .map(|(id, _)| id.as_str())
        .collect();
    assert_eq!(with_reqs.len() as u64, core["requirementsChecked"].as_u64().unwrap());
    for case in core["requirements"].as_array().unwrap() {
        let player = player_of(&case["player"]);
        let failing: Vec<&str> = with_reqs.iter().copied().filter(|id| !meets_requirements(res.get(*id), player.as_ref())).collect();
        cmp.check(&root(&format!("requirements[{}]", case["player"])), &case["failing"], &strs(&failing), &[]);
    }

    let dr = &core["drops"];
    let ids: Vec<&str> = res.keys().map(|s| s.as_str()).collect();
    let floor: Vec<&str> = ids.iter().copied().filter(|id| drops::is_floor_drop(id, res)).collect();
    cmp.check(&root("floorDrops"), &dr["floorDrops"], &strs(&floor), &[]);
    let base: Vec<Value> = ids
        .iter()
        .filter(|id| drops::drop_base_rarity(id, res) != res[**id].tier.as_deref().unwrap_or("COMMON"))
        .map(|id| json!([id, drops::drop_base_rarity(id, res)]))
        .collect();
    cmp.check(&root("baseRarity"), &dr["baseRarity"], &Value::Array(base), &[]);
    cmp.check(&root("rarityMissing"), &dr["rarityMissing"], &json!(drops::drop_base_rarity("NOT_AN_ITEM", res)), &[]);
    let mut hf: Vec<f64> = (0..52).map(|c| drops::highest_floor(c as f64) as f64).collect();
    hf.extend([drops::highest_floor(f64::INFINITY) as f64, drops::highest_floor(f64::NAN) as f64, drops::highest_floor(0.0) as f64]);
    cmp.check(&root("highestFloor"), &dr["highestFloor"], &nums(&hf), &[]);
    for row in dr["floorTierIndex"].as_array().unwrap() {
        let cl = num(&row[0]).unwrap();
        let len = row[1].as_u64().unwrap() as usize;
        cmp.check(&root("floorTierIndex"), &row[2], &n(drops::floor_tier_index(cl, len) as f64), &[]);
    }
    for row in dr["buyPriceId"].as_array().unwrap() {
        let rs = drops::buy_price_id(row[0].as_str().unwrap(), res, row[1].as_f64().unwrap());
        cmp.check(&root("buyPriceId"), &row[2], &json!(rs), &[]);
    }
    for row in core["cleanName"].as_array().unwrap() {
        let rs = dps_core::clean_name(row[0].as_str().unwrap());
        cmp.check(&root(&format!("cleanName({})", row[0])), &row[1], &json!(rs), &[]);
    }
    cmp.finish("requirements / drops / cleanName");
}

// ---------------------------------------------------------------------------------------------------------------
// Formulas
// ---------------------------------------------------------------------------------------------------------------

fn stats_of(v: &Value) -> FinalStats {
    let g = |k: &str| v.get(k).and_then(num).unwrap_or(0.0);
    FinalStats {
        damage: g("damage"),
        strength: g("strength"),
        crit_damage: g("critDamage"),
        intelligence: g("intelligence"),
        crit_chance: g("critChance"),
        additive_damage: g("additiveDamage"),
        ability_damage: g("abilityDamage"),
    }
}

fn blessings_of(v: &Value) -> Blessings {
    if v.is_null() {
        return f::DEFAULT_BLESSINGS;
    }
    Blessings {
        power: v["power"].as_i64().unwrap() as i32,
        wisdom: v["wisdom"].as_i64().unwrap() as i32,
        time: v["time"].as_i64().unwrap() as i32,
    }
}

#[test]
fn formulas_match_js() {
    let Some(core) = core() else { return };
    let mut cmp = Cmp::new();
    let cases = core["formulas"].as_array().unwrap();
    for case in cases {
        let a = case["args"].as_array().unwrap();
        let x = |i: usize| num(&a[i]).unwrap();
        let got = match case["fn"].as_str().unwrap() {
            "calcStrength" => f::calc_strength(x(0), x(1), x(2), x(3), x(4), &blessings_of(&a[5])),
            "calcCritDamage" => f::calc_crit_damage(x(0), x(1), x(2), x(3), x(4), &blessings_of(&a[5])),
            "calcIntelligence" => f::calc_intelligence(x(0), x(1), x(2), x(3), x(4), &blessings_of(&a[5])),
            "calcCritChance" => f::calc_crit_chance(x(0), x(1), x(2)),
            "blessingEffect" => f::blessing_effect(x(0), x(1)),
            "expectedHit" => f::expected_hit(&stats_of(&a[0]), x(1), a[2].as_bool().unwrap()),
            "archerDamage" => f::archer_damage(&stats_of(&a[0]), x(1), x(2), a[3].as_bool().unwrap()),
            "berserkerDamage" => f::berserker_damage(&stats_of(&a[0]), x(1), x(2), x(3), a[4].as_bool().unwrap()),
            "mageDamage" => f::mage_damage(&stats_of(&a[0]), x(1), x(2)),
            "rcmDamage" => f::rcm_damage(&stats_of(&a[0]), x(1), x(2), x(3), x(4)),
            "dungeonMultiplier" => f::dungeon_multiplier(x(0), x(1), x(2), x(3)),
            "dungeonCritMultiplier" => f::dungeon_crit_multiplier(x(0), x(1), x(2)),
            "dungeonStat" => f::dungeon_stat(x(0), x(1), x(2), x(3), x(4)),
            "dungeonCritChance" => f::dungeon_crit_chance(x(0), x(1), x(2), x(3)),
            "cataLevelFromXp" => f::cata_level_from_xp(x(0)) as f64,
            "skillLevelFromXp" => f::skill_level_from_xp(x(0), a[1].as_i64().unwrap()) as f64,
            "secretDigits" => f::secret_digits(x(0)),
            "powerMultiplier" => f::power_multiplier(x(0)),
            "defenseMultiplier" => f::defense_multiplier(x(0)),
            "calcDamage" => f::calc_damage(x(0)),
            "cataBoost" => f::cata_boost(x(0)),
            other => panic!("unknown formula {other}"),
        };
        cmp.check(&root(&format!("{}{}", case["fn"], case["args"])), &case["out"], &n(got), &[]);
    }
    for case in core["applyBlessings"].as_array().unwrap() {
        let defs: Vec<c::BlessingDef> = case["blessings"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, b)| {
                let stats: Vec<Stat> = b["stats"].as_array().unwrap().iter().map(|s| Stat::from_js(s.as_str().unwrap()).unwrap()).collect();
                c::BlessingDef {
                    name: Box::leak(name.clone().into_boxed_str()),
                    level: b["level"].as_i64().unwrap() as i32,
                    stats: Box::leak(stats.into_boxed_slice()),
                }
            })
            .collect();
        let r = f::apply_blessings_with(
            case["value"].as_f64().unwrap(),
            Stat::from_js(case["stat"].as_str().unwrap()).unwrap(),
            case["effect"].as_f64().unwrap(),
            &defs,
        );
        cmp.check(&root("applyBlessings"), &case["out"], &obj(vec![("flat", n(r.flat)), ("pct", n(r.pct))]), &[]);
    }
    eprintln!("{} formula vectors", cases.len());
    cmp.finish("formulas");
}

#[test]
fn comparator_catches_differences() {
    let mut cmp = Cmp::new();
    cmp.check(&root("order"), &json!({"a": 1, "b": 2}), &json!({"b": 2, "a": 1}), &[]);
    cmp.check(&root("value"), &json!({"a": 1}), &json!({"a": 1.0000000000000002}), &[]);
    cmp.check(&root("int-vs-float"), &json!(5), &json!(5.0), &[]);
    cmp.check(&root("negzero"), &json!("-0"), &n(0.0), &[]);
    cmp.check(&root("missing"), &json!({"a": 1, "b": 2}), &json!({"a": 1}), &[]);
    cmp.check(&root("record"), &json!({"a": 1, "b": 2}), &json!({"b": 2, "a": 1}), &["record"]);
    cmp.check(&root("len"), &json!([1, 2]), &json!([1]), &[]);
    // order, value, negzero, missing, len: five errors; int-vs-float and record are equal.
    assert_eq!(cmp.errs.len(), 5, "{:?}", cmp.errs);
    let Some(core) = core() else { return };
    // The dump really holds data.
    assert!(core["petStats"].as_array().unwrap().len() > 5000);
    assert!(core["formulas"].as_array().unwrap().len() > 5000);
    assert!(core["config"]["enchants"]["ENCHANTS"].as_object().unwrap().len() == en::ENCHANTS.len());
}
