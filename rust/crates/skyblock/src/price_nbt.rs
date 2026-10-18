//! Minimal NBT scanner for auction `item_bytes`: reads only what price keys need and skips everything else
//! without building trees. Port of the parts of `decodeItems` / `toItemState` / `resolveId` (src/lib/nbt.js) and of
//! the key rules in `fetchAuctions` (src/api/priceFetch.js) that matter for pricing.
//!
//! Fields read from the first item of the `i` list, `tag.ExtraAttributes`: `id`, `enchantments` (entry count and
//! first entry), `petInfo` (JSON string or compound: `type`, `tier`, `exp`), `item_tier`, `baseStatBoostPercentage`.

use base64::engine::general_purpose::{GeneralPurpose, STANDARD};
use base64::engine::{DecodePaddingMode, GeneralPurposeConfig};
use base64::{alphabet, Engine};
use dps_core::drops::{floor_drop_key, FULL_STAT_BOOST};
use dps_core::jsfmt::js_string;
use dps_core::pets::{max_pet_level, pet_level};
use dps_core::text::js_number;
use std::io::Read;

/// Lenient base64 like Node's `Buffer.from(s, 'base64')` for the cases that matter (missing padding).
const LENIENT: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent).with_decode_allow_trailing_bits(true),
);

const MAX_DEPTH: u32 = 512;

/// Reusable buffers for [`derive_keys`] (one per thread).
#[derive(Default)]
pub struct Scratch {
    raw: Vec<u8>,
    nbt: Vec<u8>,
}

/// Decode one auction's `item_bytes` (base64 of gzipped NBT) into the price keys it contributes to, or `None` when
/// the blob is undecodable or the first item has no `ExtraAttributes.id` (JS: exception or `!item?.id`).
/// Keys: the resolved id; `<id>_MAX` for max-level pets; `<id>_FLOOR<n>` for n = 1..floor when fully boosted.
pub fn derive_keys(item_bytes: &str, scratch: &mut Scratch) -> Option<Vec<String>> {
    if item_bytes.is_empty() {
        return None;
    }
    scratch.raw.clear();
    if STANDARD.decode_vec(item_bytes.as_bytes(), &mut scratch.raw).is_err() {
        scratch.raw.clear();
        let cleaned: Vec<u8> = item_bytes
            .bytes()
            .filter_map(|b| match b {
                b'-' => Some(b'+'),
                b'_' => Some(b'/'),
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' => Some(b),
                _ => None,
            })
            .collect();
        LENIENT.decode_vec(&cleaned, &mut scratch.raw).ok()?;
    }
    scratch.nbt.clear();
    scratch.nbt.reserve(scratch.raw.len() * 4);
    flate2::read::GzDecoder::new(&scratch.raw[..]).read_to_end(&mut scratch.nbt).ok()?;
    keys_from_nbt(&scratch.nbt)
}

/// Same, from raw uncompressed NBT bytes (the root compound with the `i` list).
pub fn keys_from_nbt(nbt: &[u8]) -> Option<Vec<String>> {
    let extra = scan_extra(nbt)?;
    Some(extra.keys())
}

#[derive(Clone, Debug, PartialEq)]
enum Val<'a> {
    Num(f64),
    Str(&'a [u8]),
    Other,
}

impl Val<'_> {
    /// JS `Number(value)`.
    fn number(&self) -> f64 {
        match self {
            Val::Num(n) => *n,
            Val::Str(s) => js_number(&String::from_utf8_lossy(s)),
            Val::Other => f64::NAN,
        }
    }
    /// JS template-literal text of the value.
    fn text(&self) -> String {
        match self {
            Val::Num(n) => js_string(*n),
            Val::Str(s) => String::from_utf8_lossy(s).into_owned(),
            Val::Other => "[object Object]".into(),
        }
    }
}

#[derive(Default)]
struct PetInfo<'a> {
    kind: Option<&'a [u8]>,
    tier: Option<&'a [u8]>,
    exp: f64,
    /// Owned text when the pet came from a JSON string.
    kind_owned: Option<String>,
    tier_owned: Option<String>,
}

#[derive(Default)]
struct Extra<'a> {
    id: Option<&'a [u8]>,
    ench_count: usize,
    ench_first: Option<(&'a [u8], Val<'a>)>,
    pet_string: Option<&'a [u8]>,
    pet_compound: Option<PetInfo<'a>>,
    item_tier: Option<Val<'a>>,
    stat_boost: Option<Val<'a>>,
}

impl Extra<'_> {
    fn pet(&self) -> Option<PetInfo<'_>> {
        if self.id != Some(&b"PET"[..]) {
            return None;
        }
        if let Some(p) = &self.pet_compound {
            return Some(PetInfo { kind: p.kind, tier: p.tier, exp: p.exp, kind_owned: None, tier_owned: None });
        }
        let s = self.pet_string?;
        if s.is_empty() {
            return None;
        }
        let v: serde_json::Value = serde_json::from_slice(s).ok()?;
        match &v {
            serde_json::Value::Object(o) => {
                let get = |k: &str| o.get(k).and_then(|x| x.as_str()).map(str::to_owned);
                Some(PetInfo {
                    kind: None,
                    tier: None,
                    exp: o.get("exp").and_then(|x| x.as_f64()).unwrap_or(0.0),
                    kind_owned: get("type"),
                    tier_owned: get("tier"),
                })
            }
            // A truthy non-object has no type or tier: JS keeps the plain id and never reaches max level.
            serde_json::Value::Null | serde_json::Value::Bool(false) => None,
            serde_json::Value::String(t) if t.is_empty() => None,
            serde_json::Value::Number(n) if n.as_f64() == Some(0.0) => None,
            _ => Some(PetInfo::default()),
        }
    }

    fn keys(&self) -> Vec<String> {
        let raw_id = String::from_utf8_lossy(self.id.unwrap_or_default()).into_owned();
        let mut id = raw_id.clone();
        if raw_id == "ENCHANTED_BOOK" && self.ench_count == 1 {
            if let Some((name, val)) = &self.ench_first {
                id = format!("ENCHANTMENT_{}_{}", String::from_utf8_lossy(name).to_uppercase(), val.text());
            }
        }
        let pet = self.pet();
        let mut kind = String::new();
        let mut tier = String::new();
        if let Some(p) = &pet {
            kind = p.kind_owned.clone().unwrap_or_else(|| p.kind.map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default());
            tier = p.tier_owned.clone().unwrap_or_else(|| p.tier.map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default());
            if !kind.is_empty() && !tier.is_empty() {
                id = format!("PET_{kind}_{tier}");
            }
        }
        let mut keys = vec![id.clone()];
        if let Some(p) = &pet {
            if pet_level(&kind, &tier, p.exp) >= max_pet_level(&kind) {
                keys.push(format!("{id}_MAX"));
            }
        }
        if let Some(tier_val) = &self.item_tier {
            let floor = tier_val.number();
            let boost = self.stat_boost.as_ref().map_or(0.0, Val::number);
            if boost >= FULL_STAT_BOOST {
                let mut f = 1.0;
                while f <= floor && f <= 10_000.0 {
                    keys.push(floor_drop_key(&id, f));
                    f += 1.0;
                }
            }
        }
        keys
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Cursor over big-endian NBT.
// ---------------------------------------------------------------------------------------------------------------

struct Cur<'a> {
    b: &'a [u8],
    p: usize,
}

type R<T> = Option<T>;

impl<'a> Cur<'a> {
    fn take(&mut self, n: usize) -> R<&'a [u8]> {
        let end = self.p.checked_add(n)?;
        let s = self.b.get(self.p..end)?;
        self.p = end;
        Some(s)
    }
    fn u8(&mut self) -> R<u8> {
        Some(self.take(1)?[0])
    }
    fn u16(&mut self) -> R<usize> {
        let s = self.take(2)?;
        Some(u16::from_be_bytes([s[0], s[1]]) as usize)
    }
    fn i32(&mut self) -> R<i32> {
        let s = self.take(4)?;
        Some(i32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn name(&mut self) -> R<&'a [u8]> {
        let n = self.u16()?;
        self.take(n)
    }
    fn len(&mut self) -> R<usize> {
        let n = self.i32()?;
        Some(n.max(0) as usize)
    }

    /// Read a scalar of tag type `t` as a value (compounds, lists and arrays are skipped and give `Other`).
    fn value(&mut self, t: u8, depth: u32) -> R<Val<'a>> {
        Some(match t {
            1 => Val::Num(self.take(1)?[0] as i8 as f64),
            2 => {
                let s = self.take(2)?;
                Val::Num(i16::from_be_bytes([s[0], s[1]]) as f64)
            }
            3 => Val::Num(self.i32()? as f64),
            // prismarine's simplify turns a long into a BigInt: Number(x) is the integer
            4 => Val::Num(i64::from_be_bytes(self.take(8)?.try_into().ok()?) as f64),
            5 => {
                let s = self.take(4)?;
                Val::Num(f32::from_be_bytes([s[0], s[1], s[2], s[3]]) as f64)
            }
            6 => {
                let s = self.take(8)?;
                Val::Num(f64::from_be_bytes(s.try_into().ok()?))
            }
            8 => {
                let n = self.u16()?;
                Val::Str(self.take(n)?)
            }
            _ => {
                self.skip(t, depth)?;
                Val::Other
            }
        })
    }

    fn skip(&mut self, t: u8, depth: u32) -> R<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        match t {
            1 => self.take(1).map(|_| ()),
            2 => self.take(2).map(|_| ()),
            3 | 5 => self.take(4).map(|_| ()),
            4 | 6 => self.take(8).map(|_| ()),
            7 => {
                let n = self.len()?;
                self.take(n).map(|_| ())
            }
            8 => {
                let n = self.u16()?;
                self.take(n).map(|_| ())
            }
            9 => {
                let et = self.u8()?;
                let n = self.len()?;
                match et {
                    0 => Some(()),
                    1 => self.take(n).map(|_| ()),
                    2 => self.take(n.checked_mul(2)?).map(|_| ()),
                    3 | 5 => self.take(n.checked_mul(4)?).map(|_| ()),
                    4 | 6 => self.take(n.checked_mul(8)?).map(|_| ()),
                    _ => {
                        for _ in 0..n {
                            self.skip(et, depth + 1)?;
                        }
                        Some(())
                    }
                }
            }
            10 => {
                loop {
                    let ft = self.u8()?;
                    if ft == 0 {
                        return Some(());
                    }
                    self.name()?;
                    self.skip(ft, depth + 1)?;
                }
            }
            11 => {
                let n = self.len()?;
                self.take(n.checked_mul(4)?).map(|_| ())
            }
            12 => {
                let n = self.len()?;
                self.take(n.checked_mul(8)?).map(|_| ())
            }
            _ => None,
        }
    }
}

/// Walk the root compound to `i[0].tag.ExtraAttributes` and read its fields. `None` when the NBT is malformed or
/// there is no `ExtraAttributes.id`.
fn scan_extra(nbt: &[u8]) -> Option<Extra<'_>> {
    let mut c = Cur { b: nbt, p: 0 };
    if c.u8()? != 10 {
        return None;
    }
    c.name()?;
    let mut found: Option<Extra> = None;
    loop {
        let t = c.u8()?;
        if t == 0 {
            break;
        }
        let name = c.name()?;
        if name == b"i" && t == 9 {
            let et = c.u8()?;
            let n = c.len()?;
            if et == 10 && n > 0 {
                found = scan_item(&mut c)?;
                // Only the first item matters.
                break;
            }
            // not a list of compounds (or empty): nothing to price
            return None;
        }
        c.skip(t, 0)?;
    }
    let e = found?;
    e.id.filter(|id| !id.is_empty())?;
    Some(e)
}

/// Inside the first item compound: find `tag.ExtraAttributes`. Returns Some(None) when the item has none.
fn scan_item<'a>(c: &mut Cur<'a>) -> R<Option<Extra<'a>>> {
    loop {
        let t = c.u8()?;
        if t == 0 {
            return Some(None);
        }
        let name = c.name()?;
        if name == b"tag" && t == 10 {
            loop {
                let t2 = c.u8()?;
                if t2 == 0 {
                    return Some(None);
                }
                let n2 = c.name()?;
                if n2 == b"ExtraAttributes" && t2 == 10 {
                    return Some(Some(scan_extra_fields(c)?));
                }
                c.skip(t2, 1)?;
            }
        }
        c.skip(t, 0)?;
    }
}

fn scan_extra_fields<'a>(c: &mut Cur<'a>) -> R<Extra<'a>> {
    let mut e = Extra::default();
    loop {
        let t = c.u8()?;
        if t == 0 {
            return Some(e);
        }
        let name = c.name()?;
        match (name, t) {
            (b"id", 8) => {
                let n = c.u16()?;
                e.id = Some(c.take(n)?);
            }
            (b"enchantments", 10) => {
                e.ench_count = 0;
                e.ench_first = None;
                loop {
                    let ft = c.u8()?;
                    if ft == 0 {
                        break;
                    }
                    let fname = c.name()?;
                    let v = c.value(ft, 3)?;
                    e.ench_count += 1;
                    if e.ench_first.is_none() {
                        e.ench_first = Some((fname, v));
                    }
                }
            }
            (b"petInfo", 8) => {
                let n = c.u16()?;
                e.pet_string = Some(c.take(n)?);
                e.pet_compound = None;
            }
            (b"petInfo", 10) => {
                let mut p = PetInfo::default();
                loop {
                    let ft = c.u8()?;
                    if ft == 0 {
                        break;
                    }
                    let fname = c.name()?;
                    let v = c.value(ft, 3)?;
                    match (fname, v) {
                        (b"type", Val::Str(s)) => p.kind = Some(s),
                        (b"tier", Val::Str(s)) => p.tier = Some(s),
                        (b"exp", Val::Num(x)) => p.exp = x,
                        _ => {}
                    }
                }
                e.pet_compound = Some(p);
                e.pet_string = None;
            }
            (b"item_tier", _) => e.item_tier = Some(c.value(t, 3)?),
            (b"baseStatBoostPercentage", _) => e.stat_boost = Some(c.value(t, 3)?),
            _ => c.skip(t, 3)?,
        }
    }
}


#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) struct W(pub Vec<u8>);
    impl W {
        fn name(&mut self, n: &str) {
            self.0.extend((n.len() as u16).to_be_bytes());
            self.0.extend(n.as_bytes());
        }
        pub fn string(&mut self, n: &str, v: &str) -> &mut Self {
            self.0.push(8);
            self.name(n);
            self.name(v);
            self
        }
        pub fn int(&mut self, n: &str, v: i32) -> &mut Self {
            self.0.push(3);
            self.name(n);
            self.0.extend(v.to_be_bytes());
            self
        }
        pub fn open(&mut self, n: &str) -> &mut Self {
            self.0.push(10);
            self.name(n);
            self
        }
        pub fn end(&mut self) -> &mut Self {
            self.0.push(0);
            self
        }
    }

    /// `{ i: [ { id: 1s, Count: 1b, tag: { display:{Name}, ExtraAttributes: { ... } } ] }`
    pub(crate) fn item(fill: impl FnOnce(&mut W)) -> Vec<u8> {
        let mut w = W(vec![]);
        w.open("");
        w.0.push(9);
        w.name("i");
        w.0.push(10);
        w.0.extend(1i32.to_be_bytes());
        w.0.push(2);
        w.name("id");
        w.0.extend(1i16.to_be_bytes());
        w.open("tag");
        w.open("display").string("Name", "x").end();
        w.open("ExtraAttributes");
        fill(&mut w);
        w.end(); // ExtraAttributes
        w.end(); // tag
        w.end(); // item
        w.end(); // root
        w.0
    }

    #[test]
    fn plain_and_floor() {
        let nbt = item(|w| {
            w.string("id", "SKELETON_MASTER_CHESTPLATE").int("item_tier", 3).int("baseStatBoostPercentage", 50);
        });
        assert_eq!(keys_from_nbt(&nbt).unwrap(), ["SKELETON_MASTER_CHESTPLATE", "SKELETON_MASTER_CHESTPLATE_FLOOR1", "SKELETON_MASTER_CHESTPLATE_FLOOR2", "SKELETON_MASTER_CHESTPLATE_FLOOR3"]);
        let nbt = item(|w| {
            w.string("id", "X").int("item_tier", 3).int("baseStatBoostPercentage", 49);
        });
        assert_eq!(keys_from_nbt(&nbt).unwrap(), ["X"]);
    }

    #[test]
    fn book_and_missing_id() {
        let nbt = item(|w| {
            w.string("id", "ENCHANTED_BOOK").open("enchantments").int("sharpness", 7).end();
        });
        assert_eq!(keys_from_nbt(&nbt).unwrap(), ["ENCHANTMENT_SHARPNESS_7"]);
        let nbt = item(|w| {
            w.string("id", "ENCHANTED_BOOK").open("enchantments").int("a", 1).int("b", 2).end();
        });
        assert_eq!(keys_from_nbt(&nbt).unwrap(), ["ENCHANTED_BOOK"]);
        assert!(keys_from_nbt(&item(|w| {
            w.int("x", 1);
        }))
        .is_none());
    }

    #[test]
    fn pets() {
        let max = r#"{"type":"ENDER_DRAGON","tier":"LEGENDARY","exp":25353230,"heldItem":null}"#;
        let nbt = item(|w| {
            w.string("id", "PET").string("petInfo", max);
        });
        assert_eq!(keys_from_nbt(&nbt).unwrap(), ["PET_ENDER_DRAGON_LEGENDARY", "PET_ENDER_DRAGON_LEGENDARY_MAX"]);
        let low = r#"{"type":"ENDER_DRAGON","tier":"LEGENDARY","exp":10}"#;
        let nbt = item(|w| {
            w.string("id", "PET").string("petInfo", low);
        });
        assert_eq!(keys_from_nbt(&nbt).unwrap(), ["PET_ENDER_DRAGON_LEGENDARY"]);
        let nbt = item(|w| {
            w.string("id", "PET").string("petInfo", "{not json");
        });
        assert_eq!(keys_from_nbt(&nbt).unwrap(), ["PET"]);
    }

    #[test]
    fn truncated_is_none() {
        let nbt = item(|w| {
            w.string("id", "X");
        });
        assert!(keys_from_nbt(&nbt[..nbt.len() - 12]).is_none());
    }
}
