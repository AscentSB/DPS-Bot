//! Port of scripts/update-data.mjs: regenerates pets.json, reforges.json, accessories.json, fragged.json and npcShop.json from the
//! NotEnoughUpdates community repo, writing byte-identical files (JSON.stringify formatting, JS key order).
//!
//! JSON values go through a small ordered model ([`J`]) so key order and number formatting follow JavaScript:
//! integer-like keys first in ascending order, then insertion order; numbers print like `String(x)`.

use dps_core::jsfmt::write_js_string;
use dps_core::text::js_number;
use indexmap::IndexMap;
use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::path::Path;
use std::sync::Arc;

const NEU: &str = "https://raw.githubusercontent.com/NotEnoughUpdates/NotEnoughUpdates-REPO/master/constants";
const NEU_ITEMS: &str = "https://raw.githubusercontent.com/NotEnoughUpdates/NotEnoughUpdates-REPO/master/items";
const NEU_ROOT: &str = "https://raw.githubusercontent.com/NotEnoughUpdates/NotEnoughUpdates-REPO/master";
const NEU_TREE: &str = "https://api.github.com/repos/NotEnoughUpdates/NotEnoughUpdates-REPO/git/trees/master?recursive=1";
const HYPIXEL_ITEMS: &str = "https://api.hypixel.net/v2/resources/skyblock/items";
const FETCH_CONCURRENCY: usize = 16;

const STAT_KEYS: [(&str, &str); 10] = [
    ("strength", "strength"),
    ("crit_damage", "critDamage"),
    ("crit_chance", "critChance"),
    ("ability_damage", "abilityDamage"),
    ("intelligence", "intelligence"),
    ("speed", "speed"),
    ("walk_speed", "speed"),
    ("attack_speed", "attackSpeed"),
    ("bonus_attack_speed", "attackSpeed"),
    ("ferocity", "ferocity"),
];

const PET_STATS: [(&str, &str); 8] = [
    ("STRENGTH", "strength"),
    ("CRIT_DAMAGE", "critDamage"),
    ("CRIT_CHANCE", "critChance"),
    ("INTELLIGENCE", "intelligence"),
    ("SPEED", "speed"),
    ("BONUS_ATTACK_SPEED", "attackSpeed"),
    ("ABILITY_DAMAGE", "abilityDamage"),
    ("FEROCITY", "ferocity"),
];

fn lookup(table: &[(&'static str, &'static str)], k: &str) -> Option<&'static str> {
    table.iter().find(|(a, _)| *a == k).map(|(_, b)| *b)
}

// ---------------------------------------------------------------------------------------------------------------
// Ordered JSON value with JS semantics.
// ---------------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(IndexMap<String, J>),
}

impl<'de> Deserialize<'de> for J {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = J;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("any JSON value")
            }
            fn visit_unit<E>(self) -> Result<J, E> {
                Ok(J::Null)
            }
            fn visit_none<E>(self) -> Result<J, E> {
                Ok(J::Null)
            }
            fn visit_bool<E>(self, v: bool) -> Result<J, E> {
                Ok(J::Bool(v))
            }
            fn visit_i64<E>(self, v: i64) -> Result<J, E> {
                Ok(J::Num(v as f64))
            }
            fn visit_u64<E>(self, v: u64) -> Result<J, E> {
                Ok(J::Num(v as f64))
            }
            fn visit_f64<E>(self, v: f64) -> Result<J, E> {
                Ok(J::Num(v))
            }
            fn visit_str<E>(self, v: &str) -> Result<J, E> {
                Ok(J::Str(v.to_owned()))
            }
            fn visit_string<E>(self, v: String) -> Result<J, E> {
                Ok(J::Str(v))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut s: A) -> Result<J, A::Error> {
                let mut v = Vec::with_capacity(s.size_hint().unwrap_or(0));
                while let Some(x) = s.next_element()? {
                    v.push(x);
                }
                Ok(J::Arr(v))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<J, A::Error> {
                let mut o = IndexMap::with_capacity(m.size_hint().unwrap_or(0));
                while let Some((k, v)) = m.next_entry::<String, J>()? {
                    // duplicate keys: first position, last value (JSON.parse)
                    o.insert(k, v);
                }
                Ok(J::Obj(o))
            }
        }
        d.deserialize_any(V)
    }
}

/// Canonical array index ("0", "17", not "01", below 2^32 - 1): the keys JS objects iterate first.
fn index_key(k: &str) -> Option<u32> {
    if k.is_empty() || k.len() > 10 || (k.len() > 1 && k.starts_with('0')) || !k.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    k.parse::<u64>().ok().filter(|&n| n < u32::MAX as u64).map(|n| n as u32)
}

/// `Object.entries(obj)`: integer-like keys ascending, then the rest in insertion order.
pub fn ordered<'a>(o: &'a IndexMap<String, J>) -> Vec<(&'a String, &'a J)> {
    let mut ints: Vec<(u32, &String, &J)> = Vec::new();
    let mut rest = Vec::new();
    for (k, v) in o {
        match index_key(k) {
            Some(n) => ints.push((n, k, v)),
            None => rest.push((k, v)),
        }
    }
    if ints.is_empty() {
        return rest;
    }
    ints.sort_by_key(|x| x.0);
    ints.into_iter().map(|(_, k, v)| (k, v)).chain(rest).collect()
}

impl J {
    pub fn get(&self, k: &str) -> Option<&J> {
        match self {
            J::Obj(o) => o.get(k),
            _ => None,
        }
    }
    /// JS truthiness.
    pub fn truthy(&self) -> bool {
        match self {
            J::Null => false,
            J::Bool(b) => *b,
            J::Num(n) => *n != 0.0 && !n.is_nan(),
            J::Str(s) => !s.is_empty(),
            J::Arr(_) | J::Obj(_) => true,
        }
    }
    fn str(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
            _ => None,
        }
    }
    /// `x ?? fallback`
    fn or(self, fallback: J) -> J {
        if self == J::Null {
            fallback
        } else {
            self
        }
    }
}

/// `obj.key ?? fallback` over an optional field.
fn field_or(v: &J, k: &str, fallback: J) -> J {
    v.get(k).cloned().unwrap_or(J::Null).or(fallback)
}

fn write_json(out: &mut String, v: &J, indent: Option<usize>, depth: usize) {
    let nl = |out: &mut String, d: usize| {
        if let Some(n) = indent {
            out.push('\n');
            for _ in 0..n * d {
                out.push(' ');
            }
        }
    };
    match v {
        J::Null => out.push_str("null"),
        J::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        J::Num(n) => {
            if n.is_finite() {
                write_js_string(out, *n)
            } else {
                out.push_str("null")
            }
        }
        J::Str(s) => out.push_str(&serde_json::to_string(s).unwrap()),
        J::Arr(a) => {
            if a.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                nl(out, depth + 1);
                write_json(out, x, indent, depth + 1);
            }
            nl(out, depth);
            out.push(']');
        }
        J::Obj(o) => {
            if o.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (k, x)) in ordered(o).into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                nl(out, depth + 1);
                out.push_str(&serde_json::to_string(k).unwrap());
                out.push(':');
                if indent.is_some() {
                    out.push(' ');
                }
                write_json(out, x, indent, depth + 1);
            }
            nl(out, depth);
            out.push('}');
        }
    }
}

/// `JSON.stringify(v)` (`indent` None) or `JSON.stringify(v, null, indent)`.
pub fn stringify(v: &J, indent: Option<usize>) -> String {
    let mut s = String::new();
    write_json(&mut s, v, indent, 0);
    s
}

fn obj(entries: Vec<(&str, J)>) -> J {
    J::Obj(entries.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

// ---------------------------------------------------------------------------------------------------------------
// Conversion (pure)
// ---------------------------------------------------------------------------------------------------------------

/// JS `reforgeKey(name)`: lower-case letters only.
pub fn reforge_key(name: &str) -> String {
    name.to_lowercase().chars().filter(|c| c.is_ascii_lowercase()).collect()
}

/// `{ types, items? }` entries of `itemTypes(raw)`.
fn item_types(raw: &J) -> Vec<(&'static str, J)> {
    match raw {
        J::Str(s) => {
            let types = s.split([' ', ',', '/']).filter(|t| !t.is_empty()).map(|t| J::Str(t.to_owned())).collect();
            vec![("types", J::Arr(types))]
        }
        J::Obj(_) => match raw.get("internalName") {
            Some(a @ J::Arr(_)) => vec![("types", J::Arr(vec![])), ("items", a.clone())],
            _ => vec![("types", J::Arr(vec![]))],
        },
        _ => vec![("types", J::Arr(vec![]))],
    }
}

fn convert_stats(by_rarity: Option<&J>) -> J {
    let mut out = IndexMap::new();
    if let Some(J::Obj(o)) = by_rarity {
        for (rarity, stats) in ordered(o) {
            let mut s = IndexMap::new();
            if let J::Obj(st) = stats {
                for (k, v) in ordered(st) {
                    if let Some(mapped) = lookup(&STAT_KEYS, k) {
                        s.insert(mapped.to_owned(), v.clone());
                    }
                }
            }
            if !s.is_empty() {
                out.insert(rarity.clone(), J::Obj(s));
            }
        }
    }
    J::Obj(out)
}

/// What the JS script wrote, as file contents.
pub struct Outputs {
    pub pets: String,
    pub reforges: String,
    pub accessories: String,
    pub fragged: String,
    pub reforge_count: usize,
    pub accessory_families: usize,
    pub fragged_count: usize,
}

/// Starred item recipe -> `[base, { id, parts }]` (None when the recipe does not apply).
pub fn fragged_entry(id: &str, recipe: &J) -> Option<(String, J)> {
    let J::Obj(cells) = recipe else {
        return None;
    };
    if !recipe.truthy() {
        return None;
    }
    let mut counts: IndexMap<String, f64> = IndexMap::new();
    for (_, cell) in ordered(cells) {
        let Some(cell) = cell.str() else { continue };
        if cell.is_empty() {
            continue;
        }
        let mut it = cell.split(':');
        let item = it.next().unwrap_or("");
        let qty = it.next().filter(|q| !q.is_empty()).map_or(1.0, js_number);
        // `Number(qty || 1)`: a NaN quantity stays NaN (written as null)
        let cur = counts.get(item).copied().unwrap_or(0.0);
        counts.insert(item.to_owned(), cur + qty);
    }
    let base = id.strip_prefix("STARRED_").unwrap_or(id).to_owned();
    let have = counts.get(&base).copied().unwrap_or(0.0);
    if have == 0.0 || have.is_nan() {
        return None;
    }
    counts.shift_remove(&base);
    let counts_obj: IndexMap<String, J> = counts.into_iter().map(|(k, v)| (k, J::Num(v))).collect();
    let parts = ordered(&counts_obj).into_iter().map(|(k, v)| obj(vec![("id", J::Str(k.clone())), ("qty", v.clone())])).collect();
    Some((base, obj(vec![("id", J::Str(id.to_owned())), ("parts", J::Arr(parts))])))
}

/// `items/*_NPC.json` paths in the NEU file list (`/^items\/[^/]+_NPC\.json$/`), sorted.
pub fn npc_files(tree: &J) -> Result<Vec<String>, String> {
    let Some(J::Arr(list)) = tree.get("tree") else { return Err("NEU repo file list: incomplete".into()) };
    if tree.get("truncated").is_some_and(J::truthy) {
        return Err("NEU repo file list: incomplete".into());
    }
    let is_npc = |p: &str| {
        p.strip_prefix("items/").and_then(|r| r.strip_suffix("_NPC.json")).is_some_and(|name| !name.is_empty() && !name.contains('/'))
    };
    let mut paths: Vec<String> = list.iter().filter_map(|x| x.get("path").and_then(J::str)).filter(|p| is_npc(p)).map(str::to_owned).collect();
    paths.sort();
    Ok(paths)
}

/// JS `String(x)` for the strings and numbers NEU uses in shop recipes.
fn js_text(v: &J) -> Option<String> {
    match v {
        J::Str(s) => Some(s.clone()),
        J::Num(n) => Some(dps_core::jsfmt::js_string(*n)),
        _ => None,
    }
}

/// `shopPart`: "ID:qty" (qty defaults to 1).
fn shop_part(s: &str) -> (String, f64) {
    match s.rfind(':') {
        None => (s.to_owned(), 1.0),
        Some(i) => (s[..i].to_owned(), js_number(&s[i + 1..])),
    }
}

/// "§9Combat Merchant (NPC)" -> "Combat Merchant", "Alchemist (Rift NPC)" -> "Alchemist (Rift)" (JS `/§./g`, trim).
fn npc_name(display: &str) -> String {
    let mut out = String::with_capacity(display.len());
    let mut chars = display.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '§' && chars.peek().is_some_and(|n| !matches!(n, '\n' | '\r' | '\u{2028}' | '\u{2029}')) {
            chars.next();
            continue;
        }
        out.push(c);
    }
    let mut name = dps_core::text::js_trim(&out).to_owned();
    if let Some(s) = name.strip_suffix(" (NPC)") {
        name = s.to_owned();
    }
    if let Some(s) = name.strip_suffix(" (Rift NPC)") {
        name = format!("{s} (Rift)");
    }
    name
}

/// npcShop.json from the downloaded NPC files (None for a failed download), in path order: `{ ITEM_ID: [{ npc,
/// coins, items? }] }` sorted by id. Returns the file text and the item count.
pub fn build_npc_shop(npcs: &[Option<J>]) -> (String, usize) {
    let mut shop: IndexMap<String, Vec<J>> = IndexMap::new();
    for npc in npcs.iter().flatten() {
        let name = npc_name(&npc.get("displayname").and_then(js_text).unwrap_or_default());
        if name.is_empty() {
            continue;
        }
        let Some(J::Arr(recipes)) = npc.get("recipes") else { continue };
        for r in recipes {
            if r.get("type").and_then(J::str) != Some("npc_shop") {
                continue;
            }
            let Some(result) = r.get("result").filter(|x| x.truthy()).and_then(js_text) else { continue };
            let (id, n) = shop_part(&result);
            if id.contains(';') || !(n > 0.0) {
                continue;
            }
            let mut coins = 0.0;
            let mut items = Vec::new();
            if let Some(J::Arr(cost)) = r.get("cost") {
                for c in cost.iter().filter_map(js_text) {
                    let (cid, q) = shop_part(&c);
                    if cid == "SKYBLOCK_COIN" {
                        coins += q;
                    } else {
                        items.push(obj(vec![("id", J::Str(cid)), ("qty", J::Num(q / n))]));
                    }
                }
            }
            if !(coins >= 1.0) && items.is_empty() {
                continue;
            }
            let mut fields = vec![("npc", J::Str(name.clone())), ("coins", J::Num(coins / n))];
            if !items.is_empty() {
                fields.push(("items", J::Arr(items)));
            }
            let offer = obj(fields);
            let list = shop.entry(id).or_default();
            let text = stringify(&offer, None);
            if !list.iter().any(|o| stringify(o, None) == text) {
                list.push(offer);
            }
        }
    }
    let mut keys: Vec<String> = shop.keys().cloned().collect();
    keys.sort();
    let count = keys.len();
    let sorted: IndexMap<String, J> = keys.into_iter().map(|k| {
        let v = shop.shift_remove(&k).unwrap_or_default();
        (k, J::Arr(v))
    }).collect();
    (stringify(&J::Obj(sorted), Some(1)) + "\n", count)
}

/// Build the four files from the downloaded sources. `fragged` is `[(base, entry)]` in the order to write.
pub fn build_outputs(basic: &J, stones: &J, misc: &J, pet_meta: &J, pet_nums: &J, fragged: Vec<(String, J)>) -> Result<Outputs, String> {
    let mut reforges: IndexMap<String, J> = IndexMap::new();
    if let J::Obj(o) = basic {
        for (_, r) in ordered(o) {
            let name = r.get("reforgeName");
            let key = reforge_key(&match name {
                Some(J::Str(s)) => s.clone(),
                Some(J::Num(n)) => dps_core::jsfmt::js_string(*n),
                _ => "undefined".to_owned(),
            });
            let mut e = IndexMap::new();
            if let Some(n) = name {
                e.insert("name".to_owned(), n.clone());
            }
            e.insert("stone".to_owned(), J::Null);
            for (k, v) in item_types(r.get("itemTypes").unwrap_or(&J::Null)) {
                e.insert(k.to_owned(), v);
            }
            e.insert("stats".to_owned(), convert_stats(r.get("reforgeStats")));
            reforges.insert(key, J::Obj(e));
        }
    }
    if let J::Obj(o) = stones {
        for (stone_id, r) in ordered(o) {
            let Some(name) = r.get("reforgeName").filter(|n| n.truthy()) else { continue };
            let key = reforge_key(name.str().unwrap_or(""));
            let mut e = IndexMap::new();
            e.insert("name".to_owned(), name.clone());
            e.insert("stone".to_owned(), J::Str(stone_id.clone()));
            for (k, v) in item_types(r.get("itemTypes").unwrap_or(&J::Null)) {
                e.insert(k.to_owned(), v);
            }
            e.insert("stats".to_owned(), convert_stats(r.get("reforgeStats")));
            match key.as_str() {
                "ancient" => e.insert("cataStat".to_owned(), J::Str("critDamage".into())),
                "withered" => e.insert("cataStat".to_owned(), J::Str("strength".into())),
                _ => None,
            };
            reforges.insert(key, J::Obj(e));
        }
    }

    let mut acc = IndexMap::new();
    let upgrades = misc.get("talisman_upgrades");
    if let Some(u) = upgrades {
        acc.insert("upgrades".to_owned(), u.clone());
    }
    if let Some(i) = misc.get("ignored_talisman") {
        acc.insert("ignored".to_owned(), i.clone());
    }
    let accessory_families = match upgrades {
        Some(J::Obj(o)) => o.len(),
        _ => return Err("misc.json has no talisman_upgrades".into()),
    };

    let mut fr = IndexMap::new();
    for (base, e) in fragged {
        fr.insert(base, e);
    }

    // Pets: leveling tables and stat numbers at level 1 and max for every pet with a DPS stat.
    let mut pet_stats: IndexMap<String, J> = IndexMap::new();
    if let J::Obj(types) = pet_nums {
        for (kind, rarities) in ordered(types) {
            let J::Obj(rar) = rarities else { continue };
            for (rarity, data) in ordered(rar) {
                let (Some(d1), Some(d100)) = (data.get("1").filter(|x| x.truthy()), data.get("100").filter(|x| x.truthy())) else { continue };
                let pick = |nums: Option<&J>| {
                    let mut m = IndexMap::new();
                    if let Some(J::Obj(o)) = nums {
                        for (k, v) in ordered(o) {
                            if let Some(mapped) = lookup(&PET_STATS, k) {
                                m.insert(mapped.to_owned(), v.clone());
                            }
                        }
                    }
                    J::Obj(m)
                };
                let mut e = IndexMap::new();
                e.insert("min".to_owned(), pick(d1.get("statNums")));
                e.insert("max".to_owned(), pick(d100.get("statNums")));
                e.insert("otherMin".to_owned(), field_or(d1, "otherNums", J::Arr(vec![])));
                e.insert("otherMax".to_owned(), field_or(d100, "otherNums", J::Arr(vec![])));
                if let Some(c) = data.get("stats_levelling_curve").filter(|c| c.truthy()) {
                    e.insert("curve".to_owned(), c.clone());
                }
                let entry = pet_stats.entry(kind.clone()).or_insert_with(|| J::Obj(IndexMap::new()));
                if let J::Obj(m) = entry {
                    m.insert(rarity.clone(), J::Obj(e));
                }
            }
        }
    }
    let mut custom = IndexMap::new();
    if let Some(J::Obj(c)) = pet_meta.get("custom_pet_leveling") {
        for (k, v) in ordered(c) {
            custom.insert(
                k.clone(),
                obj(vec![
                    ("levels", field_or(v, "pet_levels", J::Arr(vec![]))),
                    ("maxLevel", field_or(v, "max_level", J::Num(100.0))),
                    ("type", field_or(v, "type", J::Num(0.0))),
                ]),
            );
        }
    }
    let mut pets = IndexMap::new();
    if let Some(l) = pet_meta.get("pet_levels") {
        pets.insert("levels".to_owned(), l.clone());
    }
    if let Some(o) = pet_meta.get("pet_rarity_offset") {
        pets.insert("rarityOffset".to_owned(), o.clone());
    }
    pets.insert("custom".to_owned(), J::Obj(custom));
    pets.insert("stats".to_owned(), J::Obj(pet_stats));

    Ok(Outputs {
        pets: stringify(&J::Obj(pets), None) + "\n",
        reforge_count: reforges.len(),
        reforges: stringify(&J::Obj(reforges), Some(1)) + "\n",
        accessories: stringify(&J::Obj(acc), Some(1)) + "\n",
        fragged_count: fr.len(),
        fragged: stringify(&J::Obj(fr), Some(1)) + "\n",
        accessory_families,
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Download + write
// ---------------------------------------------------------------------------------------------------------------

async fn get_json(client: &reqwest::Client, url: &str, name: &str) -> Result<J, String> {
    let res = client.get(url).send().await.map_err(|e| format!("{name}: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("{name}: HTTP {}", res.status().as_u16()));
    }
    let bytes = res.bytes().await.map_err(|e| format!("{name}: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("{name}: {e}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateSummary {
    pub reforges: usize,
    pub accessory_families: usize,
    pub fragged: usize,
    pub npc_shop: usize,
}

impl fmt::Display for UpdateSummary {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Wrote {} reforges, {} accessory families, {} fragged items, {} NPC shop items.", self.reforges, self.accessory_families, self.fragged, self.npc_shop)
    }
}

/// Download the NEU data and write `pets.json`, `reforges.json`, `accessories.json`, `fragged.json`, `npcShop.json` into `out_dir`
/// (created if needed). Errors on any failed constants download; a starred item whose recipe cannot be fetched is
/// skipped, like the script.
pub async fn update_data(out_dir: impl AsRef<Path>) -> Result<UpdateSummary, String> {
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(60)).build().map_err(|e| e.to_string())?;
    let c = |name: &str| {
        let url = format!("{NEU}/{name}");
        let client = client.clone();
        let name = name.to_owned();
        async move { get_json(&client, &url, &name).await }
    };
    let (basic, stones, misc, pet_meta, pet_nums) = tokio::try_join!(c("reforges.json"), c("reforgestones.json"), c("misc.json"), c("pets.json"), c("petnums.json"))?;

    // Fragged (STARRED_) items: base item + dungeon boss fragments, from NEU recipes.
    let hypixel = get_json(&client, HYPIXEL_ITEMS, "items").await?;
    let starred: Vec<String> = match hypixel.get("items") {
        Some(J::Arr(items)) => items.iter().filter_map(|i| i.get("id").and_then(J::str)).filter(|id| id.starts_with("STARRED_")).map(str::to_owned).collect(),
        _ => return Err("items: no items list".into()),
    };
    let sem = Arc::new(tokio::sync::Semaphore::new(FETCH_CONCURRENCY));
    let mut handles = Vec::new();
    for id in starred {
        let (client, sem) = (client.clone(), sem.clone());
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.ok()?;
            let res = client.get(format!("{NEU_ITEMS}/{id}.json")).send().await.ok()?;
            if !res.status().is_success() {
                return None;
            }
            let v: J = serde_json::from_slice(&res.bytes().await.ok()?).ok()?;
            fragged_entry(&id, v.get("recipe")?)
        }));
    }
    let mut fragged = Vec::new();
    for h in handles {
        if let Some(e) = h.await.map_err(|e| e.to_string())? {
            fragged.push(e);
        }
    }

    // NPC shops: every items/*_NPC.json in the repo (the GitHub API wants a User-Agent).
    let res = client.get(NEU_TREE).header("User-Agent", "dps-bot").send().await.map_err(|e| format!("NEU repo file list: {e}"))?;
    let tree: J = serde_json::from_slice(&res.bytes().await.map_err(|e| format!("NEU repo file list: {e}"))?).map_err(|e| format!("NEU repo file list: {e}"))?;
    let mut handles = Vec::new();
    for path in npc_files(&tree)? {
        let (client, sem) = (client.clone(), sem.clone());
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.ok()?;
            let res = client.get(format!("{NEU_ROOT}/{path}")).send().await.ok()?;
            if !res.status().is_success() {
                return None;
            }
            serde_json::from_slice::<J>(&res.bytes().await.ok()?).ok()
        }));
    }
    let mut npcs = Vec::new();
    for h in handles {
        npcs.push(h.await.map_err(|e| e.to_string())?);
    }
    let (npc_shop, npc_count) = build_npc_shop(&npcs);

    let out = build_outputs(&basic, &stones, &misc, &pet_meta, &pet_nums, fragged)?;
    let dir = out_dir.as_ref();
    tokio::fs::create_dir_all(dir).await.map_err(|e| e.to_string())?;
    for (name, text) in [("pets.json", &out.pets), ("reforges.json", &out.reforges), ("accessories.json", &out.accessories), ("fragged.json", &out.fragged), ("npcShop.json", &npc_shop)] {
        tokio::fs::write(dir.join(name), text).await.map_err(|e| format!("{name}: {e}"))?;
    }
    Ok(UpdateSummary { reforges: out.reforge_count, accessory_families: out.accessory_families, fragged: out.fragged_count, npc_shop: npc_count })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn j(s: &str) -> J {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn key_order_and_formatting() {
        let v = j(r#"{"b":1,"10":2,"2":3,"a":[],"o":{},"x":[1.5,1e21,-0,"q\"\n"]}"#);
        assert_eq!(stringify(&v, None), r#"{"2":3,"10":2,"b":1,"a":[],"o":{},"x":[1.5,1e+21,0,"q\"\n"]}"#);
        assert_eq!(stringify(&j(r#"{"a":[1,{"b":2}],"c":{}}"#), Some(1)), "{\n \"a\": [\n  1,\n  {\n   \"b\": 2\n  }\n ],\n \"c\": {}\n}");
        assert_eq!(stringify(&j(r#"{"a":1,"a":2,"b":0}"#), None), r#"{"a":2,"b":0}"#);
    }

    #[test]
    fn reforges_and_pets() {
        let basic = j(r#"{"x":{"reforgeName":"Sharp!","itemTypes":"SWORD, BOW/LONG_SWORD","reforgeStats":{"COMMON":{"strength":3,"speed":1,"walk_speed":2,"junk":9},"RARE":{"junk":1}}}}"#);
        let stones = j(r#"{"STONE_A":{"reforgeName":"Ancient","itemTypes":{"internalName":["A","B"]},"reforgeStats":{"EPIC":{"crit_damage":5}}},"NOPE":{}}"#);
        let misc = j(r#"{"talisman_upgrades":{"A":[["B",1]]},"ignored_talisman":["Z"]}"#);
        let meta = j(r#"{"pet_levels":[1,2],"pet_rarity_offset":{"COMMON":0},"custom_pet_leveling":{"P":{"pet_levels":[5],"max_level":200,"type":1},"Q":{}}}"#);
        let nums = j(r#"{"BEE":{"COMMON":{"1":{"statNums":{"STRENGTH":1,"X":2},"otherNums":[1]},"100":{"statNums":{"STRENGTH":9}},"stats_levelling_curve":"101:200:1"},"EPIC":{"1":{}}}}"#);
        let frag = vec![("B".to_owned(), obj(vec![("id", J::Str("STARRED_B".into())), ("parts", J::Arr(vec![]))]))];
        let o = build_outputs(&basic, &stones, &misc, &meta, &nums, frag).unwrap();
        assert_eq!(o.reforge_count, 2);
        assert!(o.reforges.contains("\"sharp\": {\n  \"name\": \"Sharp!\",\n  \"stone\": null,\n  \"types\": [\n   \"SWORD\","));
        assert!(o.reforges.contains("\"speed\": 2"));
        assert!(!o.reforges.contains("RARE"));
        assert!(o.reforges.contains("\"cataStat\": \"critDamage\""));
        assert!(o.reforges.contains("\"items\": ["));
        assert_eq!(
            o.pets,
            r#"{"levels":[1,2],"rarityOffset":{"COMMON":0},"custom":{"P":{"levels":[5],"maxLevel":200,"type":1},"Q":{"levels":[],"maxLevel":100,"type":0}},"stats":{"BEE":{"COMMON":{"min":{"strength":1},"max":{"strength":9},"otherMin":[1],"otherMax":[],"curve":"101:200:1"}}}}"#.to_owned() + "\n"
        );
    }

    #[test]
    fn npc_shop() {
        let tree = j(r#"{"tree":[{"path":"items/B_NPC.json"},{"path":"items/A_NPC.json"},{"path":"items/x/C_NPC.json"},{"path":"items/_NPC.json"},{"path":"items/ZOMBIE_TALISMAN.json"}]}"#);
        assert_eq!(npc_files(&tree).unwrap(), vec!["items/A_NPC.json", "items/B_NPC.json"]);
        assert!(npc_files(&j(r#"{"tree":[],"truncated":true}"#)).is_err());
        let a = j(r#"{"displayname":"§9Combat Merchant (NPC)","recipes":[
            {"type":"npc_shop","cost":["SKYBLOCK_COIN:500.0"],"result":"ZOMBIE_TALISMAN:1"},
            {"type":"npc_shop","cost":["SKYBLOCK_COIN:500.0"],"result":"ZOMBIE_TALISMAN:1"},
            {"type":"npc_shop","cost":["SKYBLOCK_COIN:8.0"],"result":"BONE:4"},
            {"type":"npc_shop","cost":["SKYBLOCK_COIN:0"],"result":"FREE"},
            {"type":"npc_shop","cost":["SKYBLOCK_COIN:5"],"result":"BEE;4"},
            {"type":"trade","cost":"EMERALD:10","result":"ZOMBIE_TALISMAN"}]}"#);
        let b = j(r#"{"displayname":"§aFear Mongerer (Rift NPC)","recipes":[{"type":"npc_shop","cost":["INTIMIDATION_TALISMAN:1","GREEN_CANDY:100"],"result":"INTIMIDATION_RING"}]}"#);
        let (text, n) = build_npc_shop(&[Some(b), None, Some(a)]);
        assert_eq!(n, 3);
        assert_eq!(
            stringify(&j(&text), None),
            r#"{"BONE":[{"npc":"Combat Merchant","coins":2}],"INTIMIDATION_RING":[{"npc":"Fear Mongerer (Rift)","coins":0,"items":[{"id":"INTIMIDATION_TALISMAN","qty":1},{"id":"GREEN_CANDY","qty":100}]}],"ZOMBIE_TALISMAN":[{"npc":"Combat Merchant","coins":500}]}"#
        );
    }

    #[test]
    fn fragged_recipe() {
        let r = j(r#"{"A1":"SHADOW_ASSASSIN_HELMET:1","A2":"LIVID_FRAGMENT:8","B1":"LIVID_FRAGMENT:4","B2":"","C1":"X"}"#);
        let (base, e) = fragged_entry("STARRED_SHADOW_ASSASSIN_HELMET", &r).unwrap();
        assert_eq!(base, "SHADOW_ASSASSIN_HELMET");
        assert_eq!(stringify(&e, None), r#"{"id":"STARRED_SHADOW_ASSASSIN_HELMET","parts":[{"id":"LIVID_FRAGMENT","qty":12},{"id":"X","qty":1}]}"#);
        assert!(fragged_entry("STARRED_Q", &r).is_none());
    }
}

#[cfg(test)]
mod live {
    /// `UPDATE_OUT=/some/dir cargo test --release -p dps-skyblock update_data_live -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn update_data_live() {
        let dir = std::env::var("UPDATE_OUT").expect("set UPDATE_OUT");
        println!("{}", super::update_data(dir).await.unwrap());
    }
}
