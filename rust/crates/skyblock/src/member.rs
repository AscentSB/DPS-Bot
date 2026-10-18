//! Port of `readMember` and its helpers from src/api/hypixel.js, plus the order-preserving profile JSON type.

use crate::nbt::{array_index, decode_items, to_item_state};
use dps_core::config::accessories::tuning_key;
use dps_core::config::constants::{COMBAT_MAX_LEVEL, SLAYER_XP, WOLF_SLAYER_SPEED_LEVELS, WOLF_SLAYER_XP};
use dps_core::config::profile::{cake_stat, HARP_SONGS, SB_LEVEL_XP};
use dps_core::config::shards::{shards_cumulative, SHARDS};
use dps_core::formulas::{cata_level_from_xp, skill_level_from_xp};
use dps_core::model::{
    Cake, Collections, CollectionsResource, Member, Pet, PetProfile, ProfileStats, Resource, Tuning, UnmaxedCollection,
};
use dps_core::text::js_number;
use indexmap::IndexMap;
use rustc_hash::FxHashMap;
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

// ---------------------------------------------------------------------------------------------------------------
// Json: parsed like JS `JSON.parse` (object keys in JS order: integer-like keys first, ascending).
// ---------------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(Box<str>),
    Arr(Vec<Json>),
    Obj(Vec<(Box<str>, Json)>),
}

impl Json {
    pub fn parse(bytes: &[u8]) -> Result<Json, String> {
        serde_json::from_slice(bytes).map_err(|e| e.to_string())
    }

    /// `obj[key]`; `None` when missing or `null` (so `?.` and `??` chains read naturally).
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(o) => o.iter().rev().find(|(k, _)| &**k == key).map(|(_, v)| v).filter(|v| !matches!(v, Json::Null)),
            _ => None,
        }
    }

    /// `a?.b?.c`.
    pub fn path(&self, keys: &[&str]) -> Option<&Json> {
        let mut cur = self;
        for k in keys {
            cur = cur.get(k)?;
        }
        Some(cur)
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(x) => Some(*x),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    /// JS truthiness.
    pub fn truthy(&self) -> bool {
        match self {
            Json::Null => false,
            Json::Bool(b) => *b,
            Json::Num(x) => *x != 0.0 && !x.is_nan(),
            Json::Str(s) => !s.is_empty(),
            _ => true,
        }
    }

    /// JS `Number(value)`.
    pub fn to_number(&self) -> f64 {
        match self {
            Json::Null => 0.0,
            Json::Bool(b) => *b as u8 as f64,
            Json::Num(x) => *x,
            Json::Str(s) => js_number(s),
            Json::Arr(a) => match a.len() {
                0 => 0.0,
                1 => a[0].to_number(),
                _ => f64::NAN,
            },
            Json::Obj(_) => f64::NAN,
        }
    }

    /// JS `String(value)` for the shapes that occur (strings, numbers, booleans).
    pub fn to_js_string(&self) -> String {
        match self {
            Json::Null => "null".into(),
            Json::Bool(b) => b.to_string(),
            Json::Num(x) => dps_core::jsfmt::js_string(*x),
            Json::Str(s) => s.to_string(),
            Json::Arr(a) => a.iter().map(|v| if matches!(v, Json::Null) { String::new() } else { v.to_js_string() }).collect::<Vec<_>>().join(","),
            Json::Obj(_) => "[object Object]".into(),
        }
    }

    /// `Object.values(x)` for an object (JS key order) or an array; empty otherwise.
    pub fn values(&self) -> impl Iterator<Item = &Json> {
        let (a, b): (Option<std::slice::Iter<'_, (Box<str>, Json)>>, Option<std::slice::Iter<'_, Json>>) = match self {
            Json::Obj(o) => (Some(o.iter()), None),
            Json::Arr(v) => (None, Some(v.iter())),
            _ => (None, None),
        };
        a.into_iter().flatten().map(|(_, v)| v).chain(b.into_iter().flatten())
    }

    /// `Object.entries(x)` for an object; empty otherwise.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &Json)> {
        let it = match self {
            Json::Obj(o) => Some(o.iter()),
            _ => None,
        };
        it.into_iter().flatten().map(|(k, v)| (&**k, v))
    }
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Json, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Json;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("any JSON value")
            }
            fn visit_bool<E>(self, v: bool) -> Result<Json, E> {
                Ok(Json::Bool(v))
            }
            fn visit_i64<E>(self, v: i64) -> Result<Json, E> {
                Ok(Json::Num(v as f64))
            }
            fn visit_u64<E>(self, v: u64) -> Result<Json, E> {
                Ok(Json::Num(v as f64))
            }
            fn visit_f64<E>(self, v: f64) -> Result<Json, E> {
                Ok(Json::Num(v))
            }
            fn visit_str<E>(self, v: &str) -> Result<Json, E> {
                Ok(Json::Str(v.into()))
            }
            fn visit_string<E>(self, v: String) -> Result<Json, E> {
                Ok(Json::Str(v.into_boxed_str()))
            }
            fn visit_unit<E>(self) -> Result<Json, E> {
                Ok(Json::Null)
            }
            fn visit_none<E>(self) -> Result<Json, E> {
                Ok(Json::Null)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Json, A::Error> {
                let mut v = Vec::with_capacity(a.size_hint().unwrap_or(0).min(4096));
                while let Some(x) = a.next_element()? {
                    v.push(x);
                }
                Ok(Json::Arr(v))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<Json, A::Error> {
                let mut v: Vec<(Box<str>, Json)> = Vec::with_capacity(m.size_hint().unwrap_or(0).min(4096));
                let mut has_index = false;
                while let Some((k, x)) = m.next_entry::<Box<str>, Json>()? {
                    has_index |= array_index(&k).is_some();
                    v.push((k, x));
                }
                if has_index {
                    let (mut ints, rest): (Vec<_>, Vec<_>) = v.into_iter().partition(|(k, _)| array_index(k).is_some());
                    ints.sort_by_key(|(k, _)| array_index(k));
                    ints.extend(rest);
                    v = ints;
                }
                Ok(Json::Obj(v))
            }
        }
        d.deserialize_any(V)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// readMember
// ---------------------------------------------------------------------------------------------------------------

/// `Number(x) || 0`.
fn num_or0(v: &Json) -> f64 {
    let n = v.to_number();
    if n.is_nan() {
        0.0
    } else {
        n
    }
}

/// `Number(opt ?? 0)`.
fn num_default(v: Option<&Json>) -> f64 {
    v.map_or(0.0, Json::to_number)
}

/// Every inventory blob (base64 `data` strings) on the member that can hold gear, including wardrobe armor and
/// equipment sets stored under `member.loadout`. Non-string truthy `data` counts as a blob (the inventory API is on)
/// but yields no items.
fn inventory_blobs(member: &Json) -> Vec<Option<&str>> {
    // `if (!inv) return []`: a truthy non-object (never sent by Hypixel) still reaches the loadout blobs below.
    let Some(inv) = member.get("inventory").filter(|v| v.truthy()) else { return Vec::new() };
    let mut out: Vec<Option<&str>> = Vec::new();
    fn push<'a>(out: &mut Vec<Option<&'a str>>, blob: Option<&'a Json>) {
        if let Some(d) = blob.and_then(|b| b.get("data")).filter(|d| d.truthy()) {
            out.push(d.as_str());
        }
    }
    for k in ["inv_contents", "inv_armor", "equipment_contents", "ender_chest_contents", "wardrobe_contents", "personal_vault_contents"] {
        push(&mut out, inv.get(k));
    }
    push(&mut out, inv.path(&["bag_contents", "talisman_bag"]));
    if let Some(bp) = inv.get("backpack_contents") {
        for b in bp.values() {
            push(&mut out, Some(b));
        }
    }
    for g in ["armor", "equipment"] {
        if let Some(group) = member.path(&["loadout", g]) {
            for set in group.values() {
                for piece in set.values() {
                    push(&mut out, Some(piece));
                }
            }
        }
    }
    out
}

/// Pull the player's items, Catacombs level and other DPS-relevant data from one profile response.
/// `now_ms` is `Date.now()` (cake buffs expire).
pub fn read_member(
    profile: &Json,
    uuid: &str,
    resource: &Resource,
    collections: Option<&CollectionsResource>,
    now_ms: f64,
) -> Result<Member, String> {
    let empty = Json::Obj(Vec::new());
    let member = match profile.path(&["members", uuid]) {
        Some(m) if m.truthy() => m,
        _ => return Err("Player is not a member of this profile.".into()),
    };
    let member = if matches!(member, Json::Obj(_)) { member } else { &empty };
    let blobs = inventory_blobs(member);
    if blobs.is_empty() {
        return Err("Inventory API is disabled for this profile. Turn it on in SkyBlock settings.".into());
    }

    let mut items = Vec::new();
    for blob in blobs.into_iter().flatten() {
        for extra in decode_items(blob)? {
            if let Some(state) = to_item_state(&extra, resource) {
                items.push(state);
            }
        }
    }

    let cata_xp = num_default(member.path(&["dungeons", "dungeon_types", "catacombs", "experience"]));
    let class_level = |c: &str| cata_level_from_xp(num_default(member.path(&["dungeons", "player_classes", c, "experience"])));
    let bag = member.get("accessory_bag_storage").unwrap_or(&empty);

    let skills = member.path(&["player_data", "experience"]).filter(|e| e.truthy()).map(|exp| {
        let mut m = IndexMap::new();
        for (k, xp) in exp.entries() {
            if let Some(name) = k.strip_prefix("SKILL_") {
                m.insert(name.to_string(), skill_level_from_xp(xp.to_number(), 60));
            }
        }
        m
    });
    let slayers = member.path(&["slayer", "slayer_bosses"]).filter(|b| b.truthy()).map(|bosses| {
        let mut m = IndexMap::new();
        for (boss, table) in SLAYER_XP.iter() {
            let xp = num_default(bosses.path(&[boss, "xp"]));
            m.insert(boss.to_string(), table.iter().filter(|&&need| xp >= need).count() as i32);
        }
        m
    });
    let combat_level = match member.path(&["player_data", "experience", "SKILL_COMBAT"]) {
        None => COMBAT_MAX_LEVEL,
        Some(x) => skill_level_from_xp(x.to_number(), COMBAT_MAX_LEVEL as i64),
    };

    let pets = member
        .path(&["pets_data", "pets"])
        .map(|p| {
            p.values()
                .map(|p| Pet {
                    kind: p.get("type").map(Json::to_js_string).unwrap_or_default(),
                    tier: p.get("tier").map(Json::to_js_string).unwrap_or_default(),
                    held_item: p.get("heldItem").and_then(|h| h.as_str().map(String::from)),
                })
                .collect()
        })
        .unwrap_or_default();

    let perks_src = member.path(&["player_data", "perks"]).or_else(|| member.get("perks"));
    let mut perks = IndexMap::new();
    if let Some(p) = perks_src {
        for (k, v) in p.entries() {
            if let Some(x) = v.as_f64() {
                perks.insert(k.to_string(), x);
            }
        }
    }

    let harp = |k: &str| {
        let key = format!("song_{k}_perfect_completions");
        let v = member.path(&["foraging", "songs", "harp"]).and_then(|h| h.get(&key));
        let v = v.or_else(|| member.path(&["quests", "harp_quest"]).and_then(|h| h.get(&key)));
        num_default(v) > 0.0
    };

    let cakes = member
        .path(&["player_data", "temp_stat_buffs"])
        .map(|b| {
            b.values()
                .filter_map(|b| {
                    let stat = cake_stat(b.get("stat_id")?.as_str()?)?;
                    let expire = b.get("expire_at").map_or(f64::INFINITY, Json::to_number);
                    (expire > now_ms).then(|| Cake { stat: stat.js_name().to_string(), amount: b.get("amount").map_or(0.0, num_or0) })
                })
                .collect()
        })
        .unwrap_or_default();

    let wolf_xp = num_default(member.path(&["slayer", "slayer_bosses", "wolf", "xp"]));

    Ok(Member {
        items,
        cata_level: cata_level_from_xp(cata_xp),
        archer_level: class_level("archer"),
        berserk_level: class_level("berserk"),
        mage_level: class_level("mage"),
        cata_xp,
        secrets: num_default(member.path(&["dungeons", "secrets"])),
        skills,
        slayers,
        combat_level,
        collections: read_collections(coop_collection(profile, uuid, member).as_ref(), collections),
        pets,
        power: bag.get("selected_power").and_then(Json::as_str).map(str::to_lowercase),
        unlocked_powers: bag.get("unlocked_powers").map(|u| u.values().map(|x| x.to_js_string().to_lowercase()).collect()).unwrap_or_default(),
        tuning: read_tuning(bag.path(&["tuning", "slot_0"])),
        shards: read_shards(member.get("attributes")),
        perks,
        rift_prism: match member.path(&["rift", "access", "consumed_prism"]) {
            Some(Json::Bool(b)) => Some(*b),
            _ => None,
        },
        highest_ap: bag.get("highest_magical_power").and_then(Json::as_f64),
        profile_speed: wolf_slayer_speed(wolf_xp),
        profile_stats: ProfileStats {
            sb_level: (num_default(member.path(&["leveling", "experience"])) / SB_LEVEL_XP).floor() as i32,
            bop_bonus: member.path(&["leveling", "bop_bonus"]).and_then(Json::as_str).map(String::from),
            harp_songs: HARP_SONGS.iter().filter(|s| harp(s.key)).map(|s| s.key.to_string()).collect(),
            cakes,
        },
        pet_profile: PetProfile {
            bank: bank_balance(profile, member),
            gold_collection: member.path(&["collection", "GOLD_INGOT"]).and_then(Json::as_f64),
        },
    })
}

/// Maxed and unmaxed collections for the co-op, or `None` when the collections API is off or the resource is not
/// loaded. `collection`: amounts by id (`coop_collection` output).
pub fn read_collections(collection: Option<&FxHashMap<String, f64>>, collections: Option<&CollectionsResource>) -> Option<Collections> {
    let collections = collections.filter(|c| !c.is_empty())?;
    let collection = collection?;
    let mut unmaxed = Vec::new();
    let mut maxed = 0;
    for (id, c) in collections {
        let raw = collection.get(id).copied().unwrap_or(f64::NAN);
        let amount = if raw.is_nan() { 0.0 } else { raw };
        if amount >= c.max {
            maxed += 1;
        } else {
            unmaxed.push(UnmaxedCollection { id: id.clone(), name: c.name.clone(), amount, max: c.max });
        }
    }
    Some(Collections { maxed, unmaxed })
}

/// Collections for The One count for the whole co-op: every member adds their amounts, members who left included.
/// `None` when no member shares collections (collections API off). `member` is `profile.members[uuid]`.
pub fn coop_collection(profile: &Json, uuid: &str, member: &Json) -> Option<FxHashMap<String, f64>> {
    let mut total: FxHashMap<String, f64> = FxHashMap::default();
    let mut any = false;
    let mut add = |m: &Json| {
        if let Some(c) = m.get("collection").filter(|c| c.truthy()) {
            any = true;
            for (id, amount) in c.entries() {
                *total.entry(id.to_string()).or_insert(0.0) += num_or0(amount);
            }
        }
    };
    add(member);
    if let Some(members) = profile.get("members") {
        for (k, m) in members.entries() {
            if k != uuid {
                add(m);
            }
        }
    }
    any.then_some(total)
}

/// Members still on the profile. A member who left keeps an entry with `profile.deletion_notice`, and an invite not
/// yet accepted has `coop_invitation.confirmed` false. Neither counts.
pub fn active_members(profile: &Json) -> usize {
    let not_false = |v: Option<&Json>| !matches!(v, Some(Json::Bool(false)));
    profile
        .get("members")
        .map(|m| {
            m.values()
                .filter(|m| {
                    !m.path(&["profile", "deletion_notice"]).is_some_and(Json::truthy)
                        && not_false(m.path(&["profile", "coop_invitation", "confirmed"]))
                        && not_false(m.path(&["coop_invitation", "confirmed"]))
                })
                .count()
        })
        .unwrap_or(0)
}

/// Coins in the bank for Golden Dragon. `None` = bank API off.
fn bank_balance(profile: &Json, member: &Json) -> Option<f64> {
    let shared = profile.path(&["banking", "balance"]);
    let personal = if active_members(profile) <= 1 { member.path(&["profile", "bank_account"]) } else { None };
    if shared.is_none() && personal.is_none() {
        return None;
    }
    let f = |v: Option<&Json>| v.map_or(0.0, num_or0);
    let (a, b) = (f(shared), f(personal));
    Some(dps_core::jsmath::max(a, b))
}

/// Speed from Wolf Slayer levels.
pub fn wolf_slayer_speed(xp: f64) -> i32 {
    let level = WOLF_SLAYER_XP.iter().filter(|&&need| xp >= need).count() as i32;
    WOLF_SLAYER_SPEED_LEVELS.iter().filter(|&&l| level >= l).count() as i32
}

/// `{ allocation, split }` from the tuning slot, or `None` when it is missing.
pub fn read_tuning(slot: Option<&Json>) -> Option<Tuning> {
    let slot = slot?;
    if !matches!(slot, Json::Obj(_) | Json::Arr(_)) {
        return None;
    }
    let mut allocation: IndexMap<String, f64> = IndexMap::new();
    for (key, value) in slot.entries() {
        if let Some(stat) = tuning_key(key) {
            if value.to_number() > 0.0 {
                *allocation.entry(stat.js_name().to_string()).or_insert(0.0) += value.to_number();
            }
        }
    }
    let total = allocation.values().fold(0.0, |a, b| a + b);
    if total == 0.0 || total.is_nan() {
        return Some(Tuning { allocation: IndexMap::new(), split: None });
    }
    let split = allocation.iter().map(|(k, v)| (k.clone(), v / total)).collect();
    Some(Tuning { allocation, split: Some(split) })
}

/// Attribute levels from `member.attributes.stacks`.
pub fn read_shards(attributes: Option<&Json>) -> IndexMap<String, i32> {
    let stacks = attributes.and_then(|a| a.get("stacks"));
    let mut levels = IndexMap::new();
    for shard in SHARDS {
        let count = num_default(stacks.and_then(|s| s.get(shard.attribute)));
        if count > 0.0 {
            if let Some(table) = shards_cumulative(shard.rarity) {
                levels.insert(shard.id.to_string(), table.iter().filter(|&&need| count >= need).count() as i32);
            }
        }
    }
    levels
}
