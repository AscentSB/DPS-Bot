//! Cost parts and their prices, plus the pricing helpers of src/calc/upgrades.js: `priceParts`, `enchantParts`,
//! `combineParts`, `convertParts`, `copyParts`, `reforgeCost`, `reforgeAllowed` and the label helpers (`roman`,
//! `ordinal`, `titleCase`, `enrichmentName`).

use dps_core::config::constants::*;
use dps_core::config::enchants::{enchant, table_enchant_xp, enchant_upgrade_item, ENCHANT_TABLE_TWO_BOTTLES, GRAND_EXP_BOTTLE_ID};
use dps_core::data::{all_reforges, get_reforge, Reforge};
use dps_core::kinds::Mode;
use dps_core::model::{ItemState, PriceSource, PriceTables};
use std::borrow::Cow;

/// A cost part: `{ id, qty }` (qty defaults to 1) or `{ coins }` (gem slot unlock fees and fixed fees).
#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    Id(Cow<'static, str>, f64),
    Coins(f64),
}

impl Part {
    /// `{ id }` with a static id.
    pub fn id(id: &'static str) -> Part {
        Part::Id(Cow::Borrowed(id), 1.0)
    }
    /// `{ id, qty }` with a static id.
    pub fn qty(id: &'static str, qty: f64) -> Part {
        Part::Id(Cow::Borrowed(id), qty)
    }
    /// `{ id }` with a built id.
    pub fn owned(id: String) -> Part {
        Part::Id(Cow::Owned(id), 1.0)
    }
    /// `{ id, qty }` with a built id.
    pub fn owned_qty(id: String, qty: f64) -> Part {
        Part::Id(Cow::Owned(id), qty)
    }
    /// The part with its quantity (or coins) doubled, as a dual-wielded item's upgrades are bought twice.
    pub fn doubled(&self) -> Part {
        match self {
            Part::Coins(c) => Part::Coins(2.0 * c),
            Part::Id(id, q) => Part::Id(id.clone(), 2.0 * q),
        }
    }
    /// JS `x.coins || prices.get(x.id)` is checked by callers through [`Part::priced`].
    pub fn priced(&self, prices: &PriceTables) -> bool {
        match self {
            Part::Coins(c) => *c != 0.0 && !c.is_nan() || prices.get("undefined").is_some(),
            Part::Id(id, _) => prices.get(id).is_some(),
        }
    }
}

fn source_code(s: PriceSource) -> &'static str {
    s.code()
}

/// JS `priceParts(parts, prices)`: `{ cost, source }` or `None` when a part has no price. The source is the
/// distinct sources in first-seen order joined with `+`.
pub fn price_parts(parts: &[Part], prices: &PriceTables) -> Option<(f64, String)> {
    let mut cost = 0.0;
    // At most a handful of distinct sources: coins, BZ, AH.
    let mut sources: [&'static str; 3] = [""; 3];
    let mut n = 0;
    let add = |s: &'static str, sources: &mut [&'static str; 3], n: &mut usize| {
        if !sources[..*n].contains(&s) {
            sources[*n] = s;
            *n += 1;
        }
    };
    for part in parts {
        match part {
            Part::Coins(c) => {
                if *c != 0.0 && !c.is_nan() {
                    cost += *c;
                    add("coins", &mut sources, &mut n);
                } else {
                    // `part.coins` is falsy: the JS then looks up `prices.get(undefined)`.
                    let (p, s) = prices.get("undefined")?;
                    cost += p;
                    add(source_code(s), &mut sources, &mut n);
                }
            }
            Part::Id(id, qty) => {
                let (p, s) = prices.get(id)?;
                cost += p * qty;
                add(source_code(s), &mut sources, &mut n);
            }
        }
    }
    Some((cost, sources[..n].join("+")))
}

/// The cost only (`priceParts(parts, prices)?.cost`).
pub fn parts_cost(parts: &[Part], prices: &PriceTables) -> Option<f64> {
    let mut cost = 0.0;
    for part in parts {
        match part {
            Part::Coins(c) => {
                if *c != 0.0 && !c.is_nan() {
                    cost += *c;
                } else {
                    cost += prices.get("undefined")?.0;
                }
            }
            Part::Id(id, qty) => cost += prices.get(id)?.0 * qty,
        }
    }
    Some(cost)
}

// ---------------------------------------------------------------------------------------------------------------
// Labels
// ---------------------------------------------------------------------------------------------------------------

/// JS `ordinal(n)`.
pub fn ordinal(n: i32) -> String {
    match n {
        1 => "1st".into(),
        2 => "2nd".into(),
        3 => "3rd".into(),
        4 => "4th".into(),
        5 => "5th".into(),
        _ => format!("{n}th"),
    }
}

/// JS `roman(n)`.
pub fn roman(n: i32) -> String {
    const R: [&str; 10] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
    if (1..=10).contains(&n) {
        R[(n - 1) as usize].to_string()
    } else {
        n.to_string()
    }
}

/// JS `titleCase(s)`: split on `_`, capitalise each word, join with spaces.
pub fn title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for (i, w) in s.split('_').enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let mut chars = w.chars();
        if let Some(c) = chars.next() {
            for u in c.to_uppercase() {
                out.push(u);
            }
            out.push_str(chars.as_str());
        }
    }
    out
}

/// `titleCase(s.toLowerCase())` for ASCII ids.
pub fn title_lower(s: &str) -> String {
    title_case(&s.to_lowercase())
}

/// JS `enrichmentName(kind)`.
pub fn enrichment_name(kind: &str) -> String {
    if kind == "walk_speed" {
        "Speed".to_string()
    } else {
        title_case(kind)
    }
}

/// `${qty} ${titleCase(id.toLowerCase())}${qty > 1 ? 's' : ''}` for a `{ id, qty }` part list, joined with `, `.
pub fn parts_text(parts: &[dps_core::config::pets::Part]) -> String {
    let mut out = String::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        dps_core::jsfmt::write_js_string(&mut out, p.qty);
        out.push(' ');
        out.push_str(&title_lower(p.id));
        if p.qty > 1.0 {
            out.push('s');
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Enchant parts
// ---------------------------------------------------------------------------------------------------------------

/// The way chosen by [`enchant_parts`]: its parts, the note shown in the label and whether it stacks on the level
/// below.
#[derive(Clone, Debug, PartialEq)]
pub struct Way {
    pub parts: Vec<Part>,
    pub note: Option<String>,
    pub stacks: bool,
}

fn book_id(upper: &str, lv: i32) -> String {
    format!("ENCHANTMENT_{upper}_{lv}")
}

/// JS `enchantParts(key, level, current, prices)`: cheapest way to get an enchant to `level` from `current`.
pub fn enchant_parts(key: &str, level: i32, current: i32, prices: &PriceTables) -> Option<Way> {
    let upper = key.to_ascii_uppercase();
    let mut ways: Vec<Way> = vec![Way { parts: vec![Part::owned(book_id(&upper, level))], note: None, stacks: false }];
    let xp = table_enchant_xp(key).and_then(|t| if level >= 1 { t.get((level - 1) as usize).copied() } else { None });
    if let Some(xp) = xp {
        ways.push(Way {
            parts: vec![Part::qty(GRAND_EXP_BOTTLE_ID, if xp >= ENCHANT_TABLE_TWO_BOTTLES { 2.0 } else { 1.0 })],
            note: Some("enchanting table".to_string()),
            stacks: false,
        });
    }
    if let Some(upgrade) = enchant_upgrade_item(key) {
        if upgrade.level == level {
            let below = if current >= level - 1 {
                Some(Way { parts: vec![], note: None, stacks: false })
            } else {
                enchant_parts(key, level - 1, current, prices)
            };
            if let Some(below) = below {
                let mut parts = below.parts;
                parts.push(Part::id(upgrade.id));
                let article = if upgrade.name.starts_with(['A', 'E', 'I', 'O', 'U']) { "an" } else { "a" };
                ways.push(Way { parts, note: Some(format!("add {article} {}", upgrade.name)), stacks: current >= level - 1 });
            }
        }
    }
    if key.starts_with("ultimate_") && current > 0 {
        if let Some((parts, text)) = combine_parts(&upper, current, level, prices) {
            ways.push(Way { parts, note: Some(text), stacks: true });
        }
    }
    let mut best: Option<usize> = None;
    let mut best_cost = f64::INFINITY;
    for (i, way) in ways.iter().enumerate() {
        if let Some(cost) = parts_cost(&way.parts, prices) {
            if cost < best_cost {
                best = Some(i);
                best_cost = cost;
            }
        }
    }
    best.map(|i| ways.swap_remove(i))
}

/// JS `combineParts(book, from, to, prices)`: at each level add the book of that level (combine) or buy the next
/// level's book. Returns the parts and the label text, or `None`.
pub fn combine_parts(upper: &str, from: i32, to: i32, prices: &PriceTables) -> Option<(Vec<Part>, String)> {
    let mut parts = Vec::new();
    let mut combined: Vec<String> = Vec::new();
    let mut lv = from;
    while lv < to {
        let same = prices.get(&book_id(upper, lv)).map(|p| p.0);
        let next = prices.get(&book_id(upper, lv + 1)).map(|p| p.0);
        if same.is_none() && next.is_none() {
            return None;
        }
        // `!(next <= same)` with a missing `next` compares 0 <= same.
        let combine = match same {
            Some(s) => !(next.unwrap_or(0.0) <= s),
            None => false,
        };
        parts.push(Part::owned(book_id(upper, if combine { lv } else { lv + 1 })));
        if combine {
            combined.push(roman(lv));
        }
        lv += 1;
    }
    if combined.is_empty() {
        return None;
    }
    let text = format!(
        "combine: add {}",
        combined.iter().map(|r| format!("a {r} book")).collect::<Vec<_>>().join(", ")
    );
    Some((parts, text))
}

// ---------------------------------------------------------------------------------------------------------------
// Reforges
// ---------------------------------------------------------------------------------------------------------------

/// JS `reforgeAllowed(key, reforge, item, ctx)`: armor only takes Ancient (`ARMOR_REFORGE`) outside the Mage RCM set.
pub fn reforge_allowed(reforge: &Reforge, id: &str, category: Option<&str>, mode: Mode) -> bool {
    if category.is_some_and(|c| ARMOR_SLOTS.contains(&c)) && reforge.key != ARMOR_REFORGE && mode != Mode::Rcm {
        return false;
    }
    reforge.applies_to(id, category)
}

/// The number of blacksmith reforges that apply to an item (the expected number of rolls).
pub fn blacksmith_rolls(id: &str, category: Option<&str>) -> usize {
    all_reforges().iter().filter(|r| r.stone.is_none() && r.applies_to(id, category)).count()
}

/// JS `reforgeCost(reforge, item)`.
pub fn reforge_cost(reforge: &Reforge, id: &str, category: Option<&str>) -> Vec<Part> {
    match reforge.stone {
        Some(stone) => vec![Part::id(stone)],
        None => vec![Part::Coins(BLACKSMITH_REFORGE_COST * (blacksmith_rolls(id, category).max(1)) as f64)],
    }
}

/// [`reforge_cost`] with the roll count remembered in `rolls`.
pub fn reforge_cost_cached(reforge: &Reforge, id: &str, category: Option<&str>, rolls: &mut Option<usize>) -> Vec<Part> {
    match reforge.stone {
        Some(stone) => vec![Part::id(stone)],
        None => {
            let n = *rolls.get_or_insert_with(|| blacksmith_rolls(id, category));
            vec![Part::Coins(BLACKSMITH_REFORGE_COST * (n.max(1)) as f64)]
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Copies and conversions
// ---------------------------------------------------------------------------------------------------------------

/// JS `rcmBladeScore(b)`.
pub fn rcm_blade_score(b: &ItemState) -> f64 {
    let e = |k: &str| b.enchants.get(k).copied().unwrap_or(0) != 0;
    3.0 * e("ultimate_wise") as i32 as f64 + 2.0 * e("giant_killer") as i32 as f64 + e("ultimate_swarm") as i32 as f64
        - 2.0 * e("titan_killer") as i32 as f64
        - e("ender_slayer") as i32 as f64
}

/// JS `convertParts(spare, src, prices, swap)`: a spare blade raised to match `src`. Returns the new item, its parts
/// and the names of what still has to be bought (for the label). `swap` puts another enchant in place of one of src's
/// (a dual-wield copy has Execute where src has Prosecute): the spare's level of that one counts, priced as src's book.
pub fn convert_parts(spare: &ItemState, src: &ItemState, prices: &PriceTables, swap: &[(&str, &str)]) -> (ItemState, Vec<Part>, Vec<String>) {
    let mut item = spare.clone();
    let mut parts: Vec<Part> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for (src_key, &level) in &src.enchants {
        let key: &str = swap.iter().find(|(from, _)| *from == src_key.as_str()).map_or(src_key.as_str(), |(_, to)| *to);
        let cur = item.enchants.get(key).copied().unwrap_or(0);
        if cur >= level {
            continue;
        }
        let Some(way) = enchant_parts(src_key, level, cur, prices) else { continue };
        parts.extend(way.parts);
        let ench_name = match enchant(key).and_then(|e| e.name) {
            Some(n) => n.to_string(),
            None => title_case(key.strip_prefix("ultimate_").unwrap_or(key)),
        };
        names.push(match &way.note {
            Some(note) => format!("{ench_name} {} ({note})", roman(level)),
            None => format!("{ench_name} {}", roman(level)),
        });
        // One ultimate enchant per item: a new one replaces the old (Ultimate Wise -> Chimera).
        if key.starts_with("ultimate_") {
            let drop: Vec<String> = item.enchants.keys().filter(|k| k.starts_with("ultimate_")).cloned().collect();
            for k in drop {
                item.enchants.shift_remove(&k);
            }
        }
        if let Some(e) = enchant(key) {
            for c in e.conflicts {
                item.enchants.shift_remove(*c);
            }
        }
        item.enchants.insert(key.to_string(), level);
    }
    let hpb = spare.hpb.max(src.hpb);
    let hot_max = HOT_POTATO.max as i32;
    let hot = 0.max(hpb.min(hot_max) - spare.hpb.min(hot_max));
    let fuming = 0.max(0.max(hpb - hot_max) - 0.max(spare.hpb - hot_max));
    if hot != 0 {
        parts.push(Part::qty(HOT_POTATO.id, hot as f64));
        names.push(format!("{hot} Hot Potato Book{}", if hot > 1 { "s" } else { "" }));
    }
    if fuming != 0 {
        parts.push(Part::qty(FUMING_POTATO.id, fuming as f64));
        names.push(format!("{fuming} Fuming Potato Book{}", if fuming > 1 { "s" } else { "" }));
    }
    item.hpb = hpb;
    if src.art_of_war && !spare.art_of_war {
        parts.push(Part::id(ART_OF_WAR_ID));
        names.push("The Art of War".to_string());
        item.art_of_war = true;
    }
    if src.master_stars > spare.master_stars {
        let from = spare.master_stars.max(0) as usize;
        let to = (src.master_stars.max(0) as usize).min(MASTER_STAR_IDS.len());
        for id in MASTER_STAR_IDS.iter().take(to).skip(from) {
            parts.push(Part::id(id));
        }
        // JS uses `spare.masterStars ?? 0` here (not clamped).
        let from = spare.master_stars;
        names.push(if src.master_stars - from > 1 {
            format!("{}–{} Master Stars", ordinal(from + 1), ordinal(src.master_stars))
        } else {
            format!("{} Master Star", ordinal(src.master_stars))
        });
        item.master_stars = src.master_stars;
    }
    if src.recomb && !spare.recomb {
        parts.push(Part::id(RECOMBOBULATOR_ID));
        names.push("Recombobulator".to_string());
        item.recomb = true;
    }
    if let Some(r) = src.reforge.as_deref() {
        if !r.is_empty() && src.reforge != spare.reforge {
            if let Some(rf) = get_reforge(r) {
                parts.extend(reforge_cost(rf, &spare.id, spare.category.as_deref()));
                names.push(format!("{} reforge", rf.name));
                item.reforge = src.reforge.clone();
            }
        }
    }
    for x in &src.scrolls {
        if !item.scrolls.contains(x) {
            parts.push(Part::owned(x.clone()));
            names.push(title_lower(x));
            item.scrolls.push(x.clone());
        }
    }
    (item, parts, names)
}

/// JS `copyParts(item, prices)`: everything a full copy of an item costs (priced parts only).
pub fn copy_parts(item: &ItemState, prices: &PriceTables) -> Vec<Part> {
    let mut parts: Vec<Part> = Vec::new();
    for (k, &level) in &item.enchants {
        if let Some(way) = enchant_parts(k, level, 0, prices) {
            parts.extend(way.parts);
        }
    }
    let hot_max = HOT_POTATO.max as i32;
    if item.hpb != 0 {
        parts.push(Part::qty(HOT_POTATO.id, item.hpb.min(hot_max) as f64));
    }
    if item.hpb > hot_max {
        parts.push(Part::qty(FUMING_POTATO.id, (item.hpb - hot_max) as f64));
    }
    if item.art_of_war {
        parts.push(Part::id(ART_OF_WAR_ID));
    }
    for id in MASTER_STAR_IDS.iter().take(item.master_stars.max(0) as usize) {
        parts.push(Part::id(id));
    }
    if item.recomb {
        parts.push(Part::id(RECOMBOBULATOR_ID));
    }
    if let Some(rf) = item.reforge.as_deref().and_then(get_reforge) {
        parts.extend(reforge_cost(rf, &item.id, item.category.as_deref()));
    }
    for g in item.gems.slots.values() {
        parts.push(Part::owned(format!("{}_{}_GEM", g.quality, g.gem)));
    }
    for s in &item.scrolls {
        parts.push(Part::owned(s.clone()));
    }
    parts.retain(|x| x.priced(prices));
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prices(bz: &[(&str, f64)], ah: &[(&str, f64)]) -> PriceTables {
        let mut t = PriceTables::default();
        for &(k, v) in bz {
            t.bazaar.insert(k.to_string(), v);
        }
        for &(k, v) in ah {
            t.auctions.insert(k.to_string(), v);
        }
        t
    }

    #[test]
    fn labels() {
        assert_eq!(ordinal(1), "1st");
        assert_eq!(ordinal(3), "3rd");
        assert_eq!(ordinal(5), "5th");
        assert_eq!(ordinal(6), "6th");
        assert_eq!(ordinal(0), "0th");
        assert_eq!(roman(1), "I");
        assert_eq!(roman(7), "VII");
        assert_eq!(roman(10), "X");
        assert_eq!(roman(11), "11");
        assert_eq!(roman(0), "0");
        assert_eq!(title_case("critical_damage"), "Critical Damage");
        assert_eq!(title_case("golden_dragon"), "Golden Dragon");
        assert_eq!(title_case("a__b"), "A  B");
        assert_eq!(title_lower("GIANT_FRAGMENT_LASER"), "Giant Fragment Laser");
        assert_eq!(enrichment_name("walk_speed"), "Speed");
        assert_eq!(enrichment_name("critical_chance"), "Critical Chance");
    }

    #[test]
    fn pricing() {
        let t = prices(&[("A", 10.0)], &[("B", 5.0), ("A", 99.0)]);
        // Bazaar first; sources in first-seen order; quantities multiply.
        let (c, s) = price_parts(&[Part::id("B"), Part::qty("A", 3.0), Part::Coins(7.0)], &t).unwrap();
        assert_eq!(c, 5.0 + 30.0 + 7.0);
        assert_eq!(s, "AH+BZ+coins");
        assert!(price_parts(&[Part::id("MISSING")], &t).is_none());
        assert_eq!(price_parts(&[], &t), Some((0.0, String::new())));
        assert_eq!(Part::qty("A", 2.0).doubled(), Part::qty("A", 4.0));
        assert_eq!(Part::Coins(3.0).doubled(), Part::Coins(6.0));
        assert!(Part::Coins(3.0).priced(&t));
        assert!(!Part::id("MISSING").priced(&t));
    }

    #[test]
    fn enchant_ways() {
        // Book price, enchanting table (Grand Exp Bottle) and the End Stone Idol way for Ender Slayer VII.
        let t = prices(&[("ENCHANTMENT_SHARPNESS_5", 100.0), ("GRAND_EXP_BOTTLE", 10.0)], &[]);
        let w = enchant_parts("sharpness", 5, 0, &t).unwrap();
        assert_eq!(w.note.as_deref(), Some("enchanting table"));
        assert_eq!(w.parts, vec![Part::qty(GRAND_EXP_BOTTLE_ID, 1.0)]);
        // Nothing priced: no way.
        assert!(enchant_parts("sharpness", 6, 0, &t).is_none());
        // Ultimate combine: level 3 from 2 by combining a II book when it is cheaper than a III book.
        let t = prices(&[], &[("ENCHANTMENT_ULTIMATE_WISE_2", 10.0), ("ENCHANTMENT_ULTIMATE_WISE_3", 100.0)]);
        let w = enchant_parts("ultimate_wise", 3, 2, &t).unwrap();
        assert_eq!(w.note.as_deref(), Some("combine: add a II book"));
        assert!(w.stacks);
        assert_eq!(w.parts, vec![Part::owned("ENCHANTMENT_ULTIMATE_WISE_2".to_string())]);
        // A missing next-level price makes `!(next <= same)` true for the JS (null <= n), but the missing book then
        // leaves the way unpriced.
        let t = prices(&[], &[("ENCHANTMENT_ULTIMATE_WISE_2", 10.0)]);
        assert!(enchant_parts("ultimate_wise", 3, 2, &t).is_none());
    }
}
