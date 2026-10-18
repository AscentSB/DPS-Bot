//! Port of src/lib/nbt.js: `decodeItems`, `toItemState`, `normalizeGems`, `petInfo`, `resolveId`.
//!
//! A direct NBT reader. `decode_items` decodes the base64 / gzip blob and, per item, parses only
//! `tag.ExtraAttributes` (everything else is skipped without allocating), because `toItemState` reads nothing else.
//! Values follow what `prismarine-nbt`'s `simplify()` produces: byte / short / int / float / double are numbers, a
//! long is a BigInt (a number here), arrays are lists of numbers, strings are strings, lists are
//! arrays and compounds are objects (JS key order: integer-like keys first, ascending).

use base64::Engine;
use dps_core::drops::drop_base_rarity;
use dps_core::jsfmt::js_string;
use dps_core::model::{Gem, Gems, ItemState, PetInfo, Resource};
use dps_core::text::{clean_name, js_number};
use flate2::read::GzDecoder;
use indexmap::IndexMap;
use rustc_hash::FxBuildHasher;
use std::io::Read;

pub type Compound = IndexMap<String, Nbt, FxBuildHasher>;

/// A simplified NBT value.
#[derive(Clone, Debug, PartialEq)]
pub enum Nbt {
    Num(f64),
    Str(String),
    /// JS array: lists and byte / int / long arrays.
    List(Vec<Nbt>),
    Compound(Compound),
}

impl Nbt {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Nbt::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_compound(&self) -> Option<&Compound> {
        match self {
            Nbt::Compound(c) => Some(c),
            _ => None,
        }
    }
    /// JS `Number(value)`.
    pub fn to_number(&self) -> f64 {
        match self {
            Nbt::Num(x) => *x,
            Nbt::Str(s) => js_number(s),
            Nbt::List(l) => match l.len() {
                0 => 0.0,
                1 => l[0].to_number(),
                _ => f64::NAN,
            },
            Nbt::Compound(_) => f64::NAN,
        }
    }
    /// JS `String(value)` / template literal.
    pub fn to_js_string(&self) -> String {
        match self {
            Nbt::Num(x) => js_string(*x),
            Nbt::Str(s) => s.clone(),
            Nbt::List(l) => l.iter().map(|v| v.to_js_string()).collect::<Vec<_>>().join(","),
            Nbt::Compound(_) => "[object Object]".to_string(),
        }
    }
    /// JS truthiness.
    pub fn truthy(&self) -> bool {
        match self {
            Nbt::Num(x) => *x != 0.0 && !x.is_nan(),
            Nbt::Str(s) => !s.is_empty(),
            _ => true,
        }
    }
}

/// `Number(compound[key] ?? default)`.
fn num_or(c: &Compound, key: &str, default: f64) -> f64 {
    c.get(key).map_or(default, Nbt::to_number)
}

// ---------------------------------------------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------------------------------------------

const MAX_DEPTH: u32 = 512;

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

const EOF: &str = "Unexpected end of NBT data";

impl<'a> Reader<'a> {
    #[inline]
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.p.checked_add(n).ok_or(EOF)?;
        if end > self.b.len() {
            return Err(EOF.into());
        }
        let s = &self.b[self.p..end];
        self.p = end;
        Ok(s)
    }
    #[inline]
    fn u8(&mut self) -> Result<u8, String> {
        let v = *self.b.get(self.p).ok_or(EOF)?;
        self.p += 1;
        Ok(v)
    }
    #[inline]
    fn u16(&mut self) -> Result<usize, String> {
        let s = self.take(2)?;
        Ok(u16::from_be_bytes([s[0], s[1]]) as usize)
    }
    #[inline]
    fn i32(&mut self) -> Result<i32, String> {
        let s = self.take(4)?;
        Ok(i32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn name(&mut self) -> Result<&'a [u8], String> {
        let n = self.u16()?;
        self.take(n)
    }
    fn count(&mut self) -> Result<usize, String> {
        Ok(self.i32()?.max(0) as usize)
    }

    fn skip(&mut self, ty: u8, depth: u32) -> Result<(), String> {
        if depth > MAX_DEPTH {
            return Err("NBT nested too deeply".into());
        }
        match ty {
            1 => self.take(1).map(|_| ()),
            2 => self.take(2).map(|_| ()),
            3 | 5 => self.take(4).map(|_| ()),
            4 | 6 => self.take(8).map(|_| ()),
            7 => {
                let n = self.count()?;
                self.take(n).map(|_| ())
            }
            8 => {
                let n = self.u16()?;
                self.take(n).map(|_| ())
            }
            9 => {
                let et = self.u8()?;
                let n = self.count()?;
                match et {
                    1 => self.take(n).map(|_| ()),
                    2 => self.take(n.checked_mul(2).ok_or(EOF)?).map(|_| ()),
                    3 | 5 => self.take(n.checked_mul(4).ok_or(EOF)?).map(|_| ()),
                    4 | 6 => self.take(n.checked_mul(8).ok_or(EOF)?).map(|_| ()),
                    _ => {
                        for _ in 0..n {
                            self.skip(et, depth + 1)?;
                        }
                        Ok(())
                    }
                }
            }
            10 => {
                loop {
                    let t = self.u8()?;
                    if t == 0 {
                        return Ok(());
                    }
                    let _ = self.name()?;
                    self.skip(t, depth + 1)?;
                }
            }
            11 => {
                let n = self.count()?;
                self.take(n.checked_mul(4).ok_or(EOF)?).map(|_| ())
            }
            12 => {
                let n = self.count()?;
                self.take(n.checked_mul(8).ok_or(EOF)?).map(|_| ())
            }
            _ => Err(format!("Unknown NBT tag type {ty}")),
        }
    }

    /// prismarine-nbt's `simplify` turns a long into a BigInt: `Number(x)` and `${x}` give the integer.
    fn read_long(&mut self) -> Result<Nbt, String> {
        let s = self.take(8)?;
        Ok(Nbt::Num(i64::from_be_bytes(s.try_into().unwrap()) as f64))
    }

    fn read_value(&mut self, ty: u8, depth: u32) -> Result<Nbt, String> {
        if depth > MAX_DEPTH {
            return Err("NBT nested too deeply".into());
        }
        Ok(match ty {
            1 => Nbt::Num(self.u8()? as i8 as f64),
            2 => {
                let s = self.take(2)?;
                Nbt::Num(i16::from_be_bytes([s[0], s[1]]) as f64)
            }
            3 => Nbt::Num(self.i32()? as f64),
            4 => self.read_long()?,
            5 => {
                let s = self.take(4)?;
                Nbt::Num(f32::from_be_bytes([s[0], s[1], s[2], s[3]]) as f64)
            }
            6 => {
                let s = self.take(8)?;
                Nbt::Num(f64::from_be_bytes(s.try_into().unwrap()))
            }
            7 => {
                let n = self.count()?;
                let s = self.take(n)?;
                Nbt::List(s.iter().map(|&b| Nbt::Num(b as i8 as f64)).collect())
            }
            8 => {
                let n = self.u16()?;
                Nbt::Str(String::from_utf8_lossy(self.take(n)?).into_owned())
            }
            9 => {
                let et = self.u8()?;
                let n = self.count()?;
                if et == 0 && n > 0 {
                    return Err("Unknown NBT list element type 0".into());
                }
                let mut v = Vec::with_capacity(n.min(4096));
                for _ in 0..n {
                    v.push(self.read_value(et, depth + 1)?);
                }
                Nbt::List(v)
            }
            10 => Nbt::Compound(self.read_compound(depth)?),
            11 => {
                let n = self.count()?;
                let s = self.take(n.checked_mul(4).ok_or(EOF)?)?;
                Nbt::List(
                    s.chunks_exact(4).map(|c| Nbt::Num(i32::from_be_bytes([c[0], c[1], c[2], c[3]]) as f64)).collect(),
                )
            }
            12 => {
                let n = self.count()?;
                let mut v = Vec::with_capacity(n.min(4096));
                for _ in 0..n {
                    v.push(self.read_long()?);
                }
                Nbt::List(v)
            }
            _ => return Err(format!("Unknown NBT tag type {ty}")),
        })
    }

    fn read_compound(&mut self, depth: u32) -> Result<Compound, String> {
        let mut c = Compound::default();
        loop {
            let t = self.u8()?;
            if t == 0 {
                break;
            }
            let name = String::from_utf8_lossy(self.name()?).into_owned();
            let v = self.read_value(t, depth + 1)?;
            c.insert(name, v);
        }
        js_key_order(&mut c);
        Ok(c)
    }
}

/// JS array-index key: canonical decimal below 2^32 - 1.
pub fn array_index(k: &str) -> Option<u32> {
    let b = k.as_bytes();
    if b.is_empty() || b.len() > 10 || !b.iter().all(u8::is_ascii_digit) || (b[0] == b'0' && b.len() > 1) {
        return None;
    }
    let v: u64 = k.parse().ok()?;
    (v < u32::MAX as u64).then_some(v as u32)
}

/// Reorder like a JS object: integer-like keys first in ascending order, the rest in insertion order.
fn js_key_order(c: &mut Compound) {
    if !c.keys().any(|k| array_index(k).is_some()) {
        return;
    }
    let old = std::mem::take(c);
    let mut ints: Vec<(u32, String, Nbt)> = Vec::new();
    let mut rest: Vec<(String, Nbt)> = Vec::new();
    for (k, v) in old {
        match array_index(&k) {
            Some(i) => ints.push((i, k, v)),
            None => rest.push((k, v)),
        }
    }
    ints.sort_by_key(|(i, _, _)| *i);
    for (_, k, v) in ints {
        c.insert(k, v);
    }
    for (k, v) in rest {
        c.insert(k, v);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// decodeItems
// ---------------------------------------------------------------------------------------------------------------

/// `Buffer.from(s, 'base64')`: lenient (ignores unknown characters, accepts the URL-safe alphabet, optional padding).
pub fn base64_decode_lenient(s: &str) -> Vec<u8> {
    use base64::engine::general_purpose::STANDARD;
    if let Ok(v) = STANDARD.decode(s) {
        return v;
    }
    let mut clean: Vec<u8> = Vec::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' => clean.push(b),
            b'-' => clean.push(b'+'),
            b'_' => clean.push(b'/'),
            b'=' => break,
            _ => {}
        }
    }
    if clean.len() % 4 == 1 {
        clean.pop();
    }
    let engine = base64::engine::GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        base64::engine::GeneralPurposeConfig::new()
            .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent)
            .with_decode_allow_trailing_bits(true),
    );
    engine.decode(&clean).unwrap_or_default()
}

/// Gunzip a buffer (JS `zlib.gunzipSync`).
pub fn gunzip(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(data.len().saturating_mul(6));
    GzDecoder::new(data).read_to_end(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// JS `decodeItems(base64)`: the `ExtraAttributes` compound of every item in the blob that has one (items without it
/// give no item state, so they are dropped here), in order.
pub fn decode_items(base64: &str) -> Result<Vec<Compound>, String> {
    if base64.is_empty() {
        return Ok(Vec::new());
    }
    let raw = gunzip(&base64_decode_lenient(base64))?;
    decode_items_raw(&raw)
}

/// `decode_items` on already gunzipped NBT bytes.
pub fn decode_items_raw(raw: &[u8]) -> Result<Vec<Compound>, String> {
    let mut r = Reader { b: raw, p: 0 };
    let root_ty = r.u8()?;
    let _ = r.name()?;
    if root_ty != 10 {
        r.skip(root_ty, 0)?;
        return Ok(Vec::new());
    }
    let mut items: Vec<Compound> = Vec::new();
    loop {
        let t = r.u8()?;
        if t == 0 {
            break;
        }
        let name = r.name()?;
        if name == b"i" {
            // A later `i` replaces an earlier one, like a JS object key.
            items.clear();
            if t == 9 {
                let et = r.u8()?;
                let n = r.count()?;
                if et == 10 {
                    for _ in 0..n {
                        if let Some(e) = read_item(&mut r)? {
                            items.push(e);
                        }
                    }
                } else {
                    for _ in 0..n {
                        r.skip(et, 1)?;
                    }
                }
            } else {
                r.skip(t, 1)?;
            }
        } else {
            r.skip(t, 1)?;
        }
    }
    Ok(items)
}

/// One item compound: returns `tag.ExtraAttributes`.
fn read_item(r: &mut Reader) -> Result<Option<Compound>, String> {
    let mut extra: Option<Compound> = None;
    loop {
        let t = r.u8()?;
        if t == 0 {
            return Ok(extra);
        }
        let name = r.name()?;
        if name == b"tag" {
            extra = None;
            if t == 10 {
                loop {
                    let t2 = r.u8()?;
                    if t2 == 0 {
                        break;
                    }
                    let n2 = r.name()?;
                    if n2 == b"ExtraAttributes" && t2 == 10 {
                        extra = Some(r.read_compound(2)?);
                    } else {
                        if n2 == b"ExtraAttributes" {
                            extra = None;
                        }
                        r.skip(t2, 2)?;
                    }
                }
            } else {
                r.skip(t, 1)?;
            }
        } else {
            r.skip(t, 1)?;
        }
    }
}

/// Parse a whole (gunzipped) NBT document into the simplified value, for tests and tools.
pub fn parse_simplified(raw: &[u8]) -> Result<Nbt, String> {
    let mut r = Reader { b: raw, p: 0 };
    let ty = r.u8()?;
    let _ = r.name()?;
    r.read_value(ty, 0)
}

// ---------------------------------------------------------------------------------------------------------------
// toItemState
// ---------------------------------------------------------------------------------------------------------------

/// JS `petInfo(extra)`: the parsed `petInfo` of a PET (JSON string or compound), `None` otherwise.
pub fn pet_info(extra: &Compound) -> Option<PetInfo> {
    if extra.get("id").and_then(Nbt::as_str) != Some("PET") {
        return None;
    }
    match extra.get("petInfo")? {
        Nbt::Str(s) if !s.is_empty() => {
            let v: serde_json::Value = serde_json::from_str(s).ok()?;
            let o = v.as_object()?;
            Some(PetInfo {
                kind: o.get("type").and_then(|x| x.as_str()).map(String::from),
                tier: o.get("tier").and_then(|x| x.as_str()).map(String::from),
                exp: o.get("exp").and_then(|x| x.as_f64()),
                held_item: o.get("heldItem").and_then(|x| x.as_str()).map(String::from),
            })
        }
        Nbt::Compound(o) => Some(PetInfo {
            kind: o.get("type").and_then(Nbt::as_str).map(String::from),
            tier: o.get("tier").and_then(Nbt::as_str).map(String::from),
            exp: match o.get("exp") {
                Some(Nbt::Num(x)) => Some(*x),
                _ => None,
            },
            held_item: o.get("heldItem").and_then(Nbt::as_str).map(String::from),
        }),
        _ => None,
    }
}

/// JS `resolveId(extra)`: ENCHANTED_BOOK with one enchant -> `ENCHANTMENT_<NAME>_<LEVEL>`; PET -> `PET_<TYPE>_<TIER>`.
pub fn resolve_id(extra: &Compound) -> Option<String> {
    let id = extra.get("id").and_then(Nbt::as_str).filter(|s| !s.is_empty())?;
    if id == "ENCHANTED_BOOK" {
        if let Some(Nbt::Compound(e)) = extra.get("enchantments") {
            if e.len() == 1 {
                let (k, v) = e.iter().next().unwrap();
                return Some(format!("ENCHANTMENT_{}_{}", k.to_uppercase(), v.to_js_string()));
            }
        }
    }
    if let Some(p) = pet_info(extra) {
        if let (Some(t), Some(tier)) = (p.kind.as_deref().filter(|s| !s.is_empty()), p.tier.as_deref().filter(|s| !s.is_empty())) {
            return Some(format!("PET_{t}_{tier}"));
        }
    }
    Some(id.to_string())
}

/// JS `toItemState(nbtItem, resource)` for an item's `ExtraAttributes`. `None` for items with no SkyBlock ITEM_ID.
pub fn to_item_state(extra: &Compound, resource: &Resource) -> Option<ItemState> {
    let raw_id = extra.get("id").and_then(Nbt::as_str).filter(|s| !s.is_empty())?;
    let id = resolve_id(extra)?;
    let is_book = raw_id == "ENCHANTED_BOOK";
    let stars = match extra.get("upgrade_level").or_else(|| extra.get("dungeon_item_level")) {
        Some(v) => v.to_number(),
        None => 0.0,
    };
    let info = resource.get(&id);
    let name = clean_name(info.and_then(|i| i.name.as_deref()).unwrap_or(&id));
    let base_rarity = match info {
        Some(_) => drop_base_rarity(&id, resource).to_string(),
        None => "COMMON".to_string(),
    };
    let dungeon = num_or(extra, "dungeon_item", 0.0) > 0.0 || info.is_some_and(|i| i.is_dungeonizable());
    let mut enchants = IndexMapI32::default();
    if !is_book {
        if let Some(Nbt::Compound(e)) = extra.get("enchantments") {
            for (k, v) in e {
                enchants.insert(k.to_lowercase(), v.to_number() as i32);
            }
        }
    }
    let (floor, stat_boost) = match extra.get("item_tier") {
        Some(t) => (Some(t.to_number()), Some(num_or(extra, "baseStatBoostPercentage", 0.0))),
        None => (None, None),
    };
    Some(ItemState {
        category: info.and_then(|i| i.category.clone()),
        name,
        base_rarity,
        recomb: num_or(extra, "rarity_upgrades", 0.0) > 0.0,
        stars: stars.min(5.0) as i32,
        master_stars: (stars - 5.0).min(5.0).max(0.0) as i32,
        dungeon: dungeon.then_some(true),
        enchants,
        hpb: num_or(extra, "hot_potato_count", 0.0) as i32,
        art_of_war: num_or(extra, "art_of_war_count", 0.0) > 0.0,
        reforge: extra.get("modifier").and_then(Nbt::as_str).map(String::from),
        gems: normalize_gems(extra.get("gems")),
        enrichment: extra.get("talisman_enrichment").and_then(Nbt::as_str).map(String::from),
        scrolls: match extra.get("ability_scroll") {
            Some(Nbt::List(l)) => l.iter().map(Nbt::to_js_string).collect(),
            _ => Vec::new(),
        },
        pet: pet_info(extra),
        floor,
        stat_boost,
        dual_wield: false,
        id,
    })
}

type IndexMapI32 = indexmap::IndexMap<String, i32>;

/// JS `normalizeGems(raw)`: `{ slots: { COMBAT_0: { gem, quality } }, unlocked }`.
pub fn normalize_gems(raw: Option<&Nbt>) -> Gems {
    let mut out = Gems::default();
    let Some(Nbt::Compound(c)) = raw else { return out };
    if let Some(Nbt::List(l)) = c.get("unlocked_slots") {
        out.unlocked = l.iter().map(Nbt::to_js_string).collect();
    }
    for (key, value) in c {
        if key == "unlocked_slots" || key.ends_with("_gem") {
            continue;
        }
        let quality = match value {
            Nbt::Str(s) => s.as_str(),
            Nbt::Compound(q) => match q.get("quality") {
                Some(Nbt::Str(s)) => s.as_str(),
                _ => continue,
            },
            _ => continue,
        };
        if quality.is_empty() {
            continue;
        }
        let ty = match key.rfind('_') {
            Some(i) if i + 1 < key.len() && key[i + 1..].bytes().all(|b| b.is_ascii_digit()) => &key[..i],
            _ => key.as_str(),
        };
        let gem = match c.get(&format!("{key}_gem")) {
            Some(g) => g.to_js_string(),
            None => ty.to_string(),
        };
        out.slots.insert(key.clone(), Gem { gem, quality: quality.to_string() });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nbt_str(out: &mut Vec<u8>, s: &str) {
        out.extend_from_slice(&(s.len() as u16).to_be_bytes());
        out.extend_from_slice(s.as_bytes());
    }

    /// {i: [ {id: Byte 1, tag: {display: {Name: "x"}, ExtraAttributes: {id: "ABC", upgrade_level: 3 (int), "1": int 7, "0": int 6, big: long}}} ]}
    fn sample() -> Vec<u8> {
        let mut o = vec![10];
        nbt_str(&mut o, "");
        o.push(9);
        nbt_str(&mut o, "i");
        o.push(10);
        o.extend_from_slice(&1i32.to_be_bytes());
        // item compound
        o.push(2);
        nbt_str(&mut o, "id");
        o.extend_from_slice(&5i16.to_be_bytes());
        o.push(10);
        nbt_str(&mut o, "tag");
        o.push(10);
        nbt_str(&mut o, "display");
        o.push(8);
        nbt_str(&mut o, "Name");
        nbt_str(&mut o, "x");
        o.push(0);
        o.push(10);
        nbt_str(&mut o, "ExtraAttributes");
        o.push(8);
        nbt_str(&mut o, "id");
        nbt_str(&mut o, "ABC");
        o.push(3);
        nbt_str(&mut o, "upgrade_level");
        o.extend_from_slice(&3i32.to_be_bytes());
        o.push(3);
        nbt_str(&mut o, "1");
        o.extend_from_slice(&7i32.to_be_bytes());
        o.push(3);
        nbt_str(&mut o, "0");
        o.extend_from_slice(&6i32.to_be_bytes());
        o.push(4);
        nbt_str(&mut o, "big");
        o.extend_from_slice(&0x0000000100000002u64.to_be_bytes());
        o.push(0); // end ExtraAttributes
        o.push(0); // end tag
        o.push(0); // end item
        o.push(0); // end root
        o
    }

    #[test]
    fn reads_extra_attributes() {
        let items = decode_items_raw(&sample()).unwrap();
        assert_eq!(items.len(), 1);
        let e = &items[0];
        let keys: Vec<&str> = e.keys().map(|s| s.as_str()).collect();
        assert_eq!(keys, ["0", "1", "id", "upgrade_level", "big"]);
        assert_eq!(e["big"], Nbt::Num(4294967298.0));
        let st = to_item_state(e, &Resource::default()).unwrap();
        assert_eq!(st.id, "ABC");
        assert_eq!(st.stars, 3);
        assert_eq!(st.base_rarity, "COMMON");
        assert!(decode_items_raw(&sample()[..40]).is_err());
    }

    #[test]
    fn lenient_base64() {
        assert_eq!(base64_decode_lenient("aGk="), b"hi");
        assert_eq!(base64_decode_lenient("aGk"), b"hi");
        assert_eq!(base64_decode_lenient("aG k\n="), b"hi");
    }

    #[test]
    fn gems() {
        let mut c = Compound::default();
        c.insert("JASPER_0".into(), Nbt::Str("PERFECT".into()));
        let mut q = Compound::default();
        q.insert("quality".into(), Nbt::Str("FLAWLESS".into()));
        c.insert("COMBAT_0".into(), Nbt::Compound(q));
        c.insert("COMBAT_0_gem".into(), Nbt::Str("SAPPHIRE".into()));
        c.insert("unlocked_slots".into(), Nbt::List(vec![Nbt::Str("COMBAT_0".into())]));
        let g = normalize_gems(Some(&Nbt::Compound(c)));
        assert_eq!(g.slots["JASPER_0"], Gem { gem: "JASPER".into(), quality: "PERFECT".into() });
        assert_eq!(g.slots["COMBAT_0"], Gem { gem: "SAPPHIRE".into(), quality: "FLAWLESS".into() });
        assert_eq!(g.unlocked, ["COMBAT_0"]);
    }
}
