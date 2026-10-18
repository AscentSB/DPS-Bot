//! The /dps embed and buttons (port of buildEmbed, buttons, tuningText, statsText, ... in src/commands/dps.js).
//! They produce serde_json values that serialize to the same JSON discord.js sends.

use crate::session::{chest_toggle, page_count, Session, UpgradeMode, View, FREE_STEP_COINS, PAGE_SIZE};
use dps_core::config::constants::class_info;
use dps_core::jsfmt::{format_coins, js_string, math_round, to_fixed, to_locale_string};
use dps_core::model::{Damage, StatLine, TargetStats, Weights};
use dps_core::model::Map;
use dps_core::{Class, FloorMode, Mode};
use serde_json::{json, Value};
use std::fmt::Write;

// ---------------------------------------------------------------------------------------------------------------
// discord.js escapeMarkdown with default options
// ---------------------------------------------------------------------------------------------------------------

/// `(?!:\d+>)` at `pos`: true when an emoji tail `:123>` starts there.
fn emoji_tail(c: &[char], pos: usize) -> bool {
    if c.get(pos) != Some(&':') {
        return false;
    }
    let mut i = pos + 1;
    let start = i;
    while c.get(i).is_some_and(|x| x.is_ascii_digit()) {
        i += 1;
    }
    i > start && c.get(i) == Some(&'>')
}

/// escapeItalic for one marker (`*`, or `_` with the emoji-tail exclusion).
fn escape_italic_one(text: &str, m: char) -> String {
    let c: Vec<char> = text.chars().collect();
    let n = c.len();
    let mut out = String::with_capacity(text.len() + 4);
    let mut idx = 0u32;
    let mut i = 0;
    while i < n {
        if c[i] == m && (i == 0 || c[i - 1] != m) && !(m == '_' && emoji_tail(&c, i + 1)) {
            if i + 1 < n && c[i + 1] != m {
                out.push('\\');
                out.push(m);
                out.push(c[i + 1]);
                i += 2;
                continue;
            }
            if i + 2 < n && c[i + 1] == m && c[i + 2] == m {
                idx += 1;
                if idx % 2 == 1 {
                    out.push('\\');
                    out.push(m);
                    out.push(m);
                    out.push(m);
                } else {
                    out.push(m);
                    out.push(m);
                    out.push('\\');
                    out.push(m);
                }
                i += 3;
                continue;
            }
            if i + 1 == n {
                out.push('\\');
                out.push(m);
                i += 1;
                continue;
            }
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

fn escape_bold(text: &str) -> String {
    let c: Vec<char> = text.chars().collect();
    let n = c.len();
    let mut out = String::with_capacity(text.len() + 4);
    let mut idx = 0u32;
    let mut i = 0;
    while i < n {
        if c[i] == '*' && c.get(i + 1) == Some(&'*') {
            if c.get(i + 2) == Some(&'*') {
                idx += 1;
                out.push_str(if idx % 2 == 1 { "*\\*\\*" } else { "\\*\\**" });
                i += 3;
            } else {
                out.push_str("\\*\\*");
                i += 2;
            }
            continue;
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

fn escape_underline(text: &str) -> String {
    let c: Vec<char> = text.chars().collect();
    let n = c.len();
    let mut out = String::with_capacity(text.len() + 4);
    let mut idx = 0u32;
    let mut i = 0;
    while i < n {
        if c[i] == '_' && c.get(i + 1) == Some(&'_') {
            if c.get(i + 2) == Some(&'_') && !emoji_tail(&c, i + 3) {
                idx += 1;
                out.push_str(if idx % 2 == 1 { "_\\_\\_" } else { "\\_\\__" });
                i += 3;
                continue;
            }
            if !emoji_tail(&c, i + 2) {
                out.push_str("\\_\\_");
                i += 2;
                continue;
            }
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

/// escapeInlineCode: a run of one or two backticks (not part of a longer run) gets each backtick escaped.
fn escape_inline_code(text: &str) -> String {
    let c: Vec<char> = text.chars().collect();
    let n = c.len();
    let mut out = String::with_capacity(text.len() + 4);
    let mut i = 0;
    while i < n {
        if c[i] == '`' && (i == 0 || c[i - 1] != '`') {
            let two = c.get(i + 1) == Some(&'`');
            let len = if two { 2 } else { 1 };
            // greedy ``? then (?=[^`]|$)
            if c.get(i + len).is_none_or(|x| *x != '`') {
                out.push_str(if two { "\\`\\`" } else { "\\`" });
                i += len;
                continue;
            }
            if two {
                // backtrack to a single backtick: followed by '`', so the lookahead fails too
            }
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

/// discord.js `escapeMarkdown(text)` with default options. The URL / custom-emoji exclusions inside the italic and
/// underline patterns are not reproduced (the callers only pass Minecraft names).
pub fn escape_markdown(text: &str) -> String {
    let mut s = text.replace('\\', "\\\\");
    s = escape_inline_code(&s);
    s = s.replace("```", "\\`\\`\\`");
    s = escape_italic_one(&s, '*');
    s = escape_italic_one(&s, '_');
    s = escape_bold(&s);
    s = escape_underline(&s);
    s = s.replace("~~", "\\~\\~");
    s.replace("||", "\\|\\|")
}

// ---------------------------------------------------------------------------------------------------------------
// text pieces
// ---------------------------------------------------------------------------------------------------------------

fn tuning_label(stat: &str) -> &str {
    match stat {
        "strength" => "strength",
        "critDamage" => "crit damage",
        "critChance" => "crit chance",
        "intelligence" => "intelligence",
        other => other,
    }
}

/// JS `tuningText(tuning, split)`.
pub fn tuning_text(tuning: Option<&dps_core::model::TuningPoints>, split: Option<&Map<f64>>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(t) = tuning {
        if t.attack_speed != 0.0 && !t.attack_speed.is_nan() {
            parts.push(format!("{} attack speed", js_string(t.attack_speed)));
        }
        if t.speed != 0.0 && !t.speed.is_nan() {
            parts.push(format!("{} speed", js_string(t.speed)));
        }
    }
    let dps = tuning.map_or(0.0, |t| t.dps);
    if let Some(split) = split {
        for (stat, share) in split {
            let points = math_round(dps * share);
            if points != 0.0 && !points.is_nan() {
                parts.push(format!("{} {}", js_string(points), tuning_label(stat)));
            }
        }
    }
    if parts.is_empty() {
        "none".into()
    } else {
        parts.join(" · ")
    }
}

/// JS `damageText(d, rcm)`.
pub fn damage_text(d: &Damage, rcm: bool) -> String {
    if rcm {
        format!("{} per Wither Impact", format_coins(d.wither))
    } else {
        format!("Withers {}\nDragons {}", format_coins(d.wither), format_coins(d.dragon))
    }
}

#[derive(Clone, Copy)]
enum K {
    CritChance,
    AttackSpeed,
    Speed,
    Strength,
    CritDamage,
    Intelligence,
    AbilityDamage,
}

impl K {
    fn label(self) -> &'static str {
        match self {
            K::CritChance => "Crit chance",
            K::AttackSpeed => "Attack speed",
            K::Speed => "Speed",
            K::Strength => "Strength",
            K::CritDamage => "Crit damage",
            K::Intelligence => "Intelligence",
            K::AbilityDamage => "Ability damage",
        }
    }
    fn get(self, l: &StatLine) -> f64 {
        match self {
            K::CritChance => l.crit_chance,
            K::AttackSpeed => l.attack_speed,
            K::Speed => l.speed,
            K::Strength => l.strength,
            K::CritDamage => l.crit_damage,
            K::Intelligence => l.intelligence,
            K::AbilityDamage => l.ability_damage,
        }
    }
}

fn shown_stats(cls: Class, mode: Mode) -> &'static [K] {
    use K::*;
    if cls == Class::Mage && mode == Mode::Rcm {
        &[Speed, Intelligence, AbilityDamage]
    } else if cls == Class::Mage {
        &[CritChance, AttackSpeed, Speed, Strength, CritDamage, Intelligence]
    } else {
        &[CritChance, AttackSpeed, Speed, Strength, CritDamage]
    }
}

fn stat_number(n: f64) -> String {
    to_locale_string(math_round(n))
}

/// JS `statsText(s, before, after, weights)`.
pub fn stats_text(cls: Class, mode: Mode, before: &TargetStats, after: &TargetStats, weights: &Weights) -> String {
    let targets: Vec<bool> = [weights.wither > 0.0, weights.dragon > 0.0].to_vec();
    // targets = ['wither','dragon'] filtered; main is the first, other the second; none -> main wither.
    let (main_dragon, other): (bool, Option<bool>) = match (targets[0], targets[1]) {
        (true, true) => (false, Some(true)),
        (true, false) => (false, None),
        (false, true) => (true, None),
        (false, false) => (false, None),
    };
    let pick = |t: &TargetStats, dragon: bool| if dragon { t.dragon } else { t.wither };
    let mut out = String::new();
    for (n, k) in shown_stats(cls, mode).iter().enumerate() {
        if n > 0 {
            out.push('\n');
        }
        let pair = |dragon: bool| {
            format!("{} → {}", stat_number(k.get(&pick(before, dragon))), stat_number(k.get(&pick(after, dragon))))
        };
        let _ = write!(out, "{} {}", k.label(), pair(main_dragon));
        if let Some(o) = other {
            let differs = k.get(&pick(before, o)) != k.get(&pick(before, main_dragon))
                || k.get(&pick(after, o)) != k.get(&pick(after, main_dragon));
            if differs {
                let _ = write!(out, " (dragons {})", pair(o));
            }
        }
    }
    out
}

/// JS `blessingText(s)`.
pub fn blessing_text(s: &Session) -> Option<String> {
    s.blessings.map(|b| format!("Blessings: Power {} · Wisdom {} · Time {}", b.power, b.wisdom, b.time))
}

/// JS `title(s)`.
pub fn title(s: &Session) -> String {
    let set = if s.cls == Class::Mage {
        format!(
            "{}{}",
            if s.mode == Mode::Rcm { " · Clear set" } else { " · Boss set" },
            if s.floor_mode == FloorMode::Normal { " (F7)" } else { "" }
        )
    } else {
        String::new()
    };
    format!("{} · {}{}", escape_markdown(&s.name), class_info(s.cls).label, set)
}

// ---------------------------------------------------------------------------------------------------------------
// buildEmbed / buttons
// ---------------------------------------------------------------------------------------------------------------

/// JS `buildEmbed(s)`: the embed object. Panics never; a missing plan renders as an empty one.
pub fn build_embed(s: &Session) -> Value {
    let rcm = s.cls == Class::Mage && s.mode == Mode::Rcm;
    let first = s.page * PAGE_SIZE;
    let pages = page_count(s);
    if s.view == View::Profile {
        let empty = dps_core::model::ProfileMenu::default();
        let menu = s.profile.as_ref().unwrap_or(&empty);
        let mut desc = String::new();
        for (i, r) in menu.rows.iter().skip(first).take(PAGE_SIZE).enumerate() {
            if i > 0 {
                desc.push_str("\n\n");
            }
            let _ = write!(desc, "**{}. {}**\n+{}% DPS", first + i + 1, r.label, to_fixed(r.gain, 2));
        }
        if desc.is_empty() {
            desc = "No profile upgrade raises your damage.".into();
        }
        let footer = [
            Some(format!(
                "Page {}/{} · Skills, slayers, class and Catacombs levels, SkyBlock level and Harp songs. No coins.",
                s.page + 1,
                pages
            )),
            blessing_text(s),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");
        return json!({
            "title": format!("{} · Profile upgrades", title(s)),
            "description": desc,
            "fields": [{"name": "Damage now", "value": damage_text(&menu.damage, rcm), "inline": true}],
            "footer": {"text": footer},
            "color": 0x2e86c1,
        });
    }
    let result = s.current();
    let default_plan = dps_core::model::Plan::default();
    let plan = result.and_then(|r| r.plan.as_ref()).unwrap_or(&default_plan);
    let mut desc = String::new();
    for (i, st) in plan.steps.iter().skip(first).take(PAGE_SIZE).enumerate() {
        if i > 0 {
            desc.push_str("\n\n");
        }
        let price = if st.cost < FREE_STEP_COINS {
            "free".to_string()
        } else {
            let pm = if st.per_million >= 1.0 { to_fixed(st.per_million, 5) } else { to_fixed(st.per_million, 6) };
            format!("{} coins · {}% per 1M", format_coins(st.cost), pm)
        };
        let _ = write!(desc, "**{}. {}**\n+{}% DPS · {}", first + i + 1, st.label, to_fixed(st.gain, 2), price);
    }
    if desc.is_empty() {
        desc = "No priced upgrade raises your damage.".into();
    }
    let unverified = plan.steps.iter().any(|st| st.unverified == Some(true));
    let n = plan.steps.len();
    let weights = result.map(|r| r.weights).unwrap_or_default();
    let split = result.and_then(|r| r.tuning_split.as_ref());
    let footer = [
        Some(format!(
            "Page {}/{} · {} · Each step assumes the ones above it. Prices: bazaar and auction lowest BIN.",
            s.page + 1,
            pages,
            if s.upgrade_mode == UpgradeMode::Max { "Max levels" } else { "Every level" }
        )),
        blessing_text(s),
        (chest_toggle(s) && s.any_chest).then(|| "Any chestplate allowed (Skeleton Master Chestplate not forced).".to_string()),
        unverified.then(|| "(?) = not in the public API; skip it if you already have it.".to_string()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n");
    json!({
        "title": format!("{} · Coin upgrades", title(s)),
        "description": desc,
        "fields": [
            {"name": "Damage now", "value": damage_text(&plan.start_damage, rcm), "inline": true},
            {"name": format!("After all {n}"), "value": damage_text(&plan.end_damage, rcm), "inline": true},
            {"name": format!("Stats (now → after all {n})"),
             "value": stats_text(s.cls, s.mode, &plan.start_damage.stats, &plan.end_damage.stats, &weights)},
            {"name": "Tuning", "value": tuning_text(Some(&plan.start_damage.tuning), split)},
        ],
        "footer": {"text": footer},
        "color": 0x8e44ad,
    })
}

const PRIMARY: u8 = 1;
const SECONDARY: u8 = 2;

fn button(id: &str, action: &str, label: &str, style: u8, disabled: bool) -> Value {
    json!({"type": 2, "custom_id": format!("dps:{id}:{action}"), "label": label, "style": style, "disabled": disabled})
}

/// JS `buttons(id, s)`: the action rows (the second row only where the chest rule applies).
pub fn buttons(id: &str, s: &Session) -> Value {
    let pages = page_count(s);
    let coins = s.view == View::Coins;
    let profile = s.view == View::Profile;
    let row = json!({"type": 1, "components": [
        button(id, "prev", "◀ Back", SECONDARY, s.page == 0),
        button(id, "next", "Next ▶", SECONDARY, s.page + 1 >= pages),
        button(id, "coins", "Coin upgrades", if coins { PRIMARY } else { SECONDARY }, coins),
        button(id, "profile", "Profile upgrades", if profile { PRIMARY } else { SECONDARY }, profile),
        button(id, "levels", if s.upgrade_mode == UpgradeMode::All { "Show max levels" } else { "Show every level" }, SECONDARY, !coins),
    ]});
    if !chest_toggle(s) {
        return json!([row]);
    }
    json!([row, {"type": 1, "components": [button(
        id,
        "chest",
        if s.any_chest { "Skeleton Master chest: off" } else { "Skeleton Master chest: forced" },
        if s.any_chest { SECONDARY } else { PRIMARY },
        !coins,
    )]}])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::*;
    use dps_core::model::{Blessings, DpsResult, ProfileMenu};
    use std::sync::Arc;

    /// testdata path (env DPS_TESTDATA overrides the crate-relative default).
    fn td(name: &str) -> std::path::PathBuf {
        let base = std::env::var("DPS_TESTDATA").unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata").into());
        std::path::Path::new(&base).join(name)
    }

    fn parse_session(v: &Value) -> Session {
        let g = |k: &str| v[k].as_str();
        let cls = Class::from_js(g("cls").unwrap()).unwrap();
        let mut s = Session::new(cls, g("name").unwrap().to_string(), 0);
        s.target = g("target").and_then(dps_core::Target::from_js);
        s.mode = Mode::from_js(g("mode").unwrap()).unwrap();
        s.floor_mode = FloorMode::from_js(g("floorMode").unwrap()).unwrap();
        s.blessings = serde_json::from_value::<Option<Blessings>>(v["blessings"].clone()).unwrap();
        s.any_chest = v["anyChest"].as_bool().unwrap_or(false);
        s.upgrade_mode = UpgradeMode::from_js(g("upgradeMode").unwrap()).unwrap();
        s.cata_level = v["member"]["cataLevel"].as_f64();
        if let Some(c) = v["coins"].as_object() {
            for (k, r) in c {
                s.coins.insert(k.clone(), Arc::new(serde_json::from_value::<DpsResult>(r.clone()).unwrap()));
            }
        }
        if !v["profile"].is_null() {
            s.profile = Some(serde_json::from_value::<ProfileMenu>(v["profile"].clone()).unwrap());
        }
        s.view = View::from_js(g("view").unwrap()).unwrap();
        s.page = v["page"].as_u64().unwrap() as usize;
        s
    }

    #[test]
    fn escape_vectors() {
        let text = std::fs::read_to_string(td("escape-markdown.json")).expect("testdata/escape-markdown.json");
        let v: Vec<Value> = serde_json::from_str(&text).unwrap();
        for e in &v {
            assert_eq!(escape_markdown(e["in"].as_str().unwrap()), e["out"].as_str().unwrap(), "{e}");
        }
        // Expected values taken from discord.js.
        for (i, o) in [
            ("a*b**c***d`e``f", r"a\*b\*\*c\*\*\*d\`e\`\`f"),
            ("x__y___z____", r"x\_\_y\_\_\_z\_\_\\_\_"),
            ("_a_b__c", r"\_a\_b\_\_c"),
            ("**_*_**", r"\*\*\_\*\_\*\*"),
            ("```", r"\`\`\`"),
            ("a~~b||c\\d", r"a\~\~b\|\|c\\d"),
            ("___", r"\_\_\_"),
        ] {
            assert_eq!(escape_markdown(i), o, "{i}");
        }
    }

    #[test]
    fn embed_goldens() {
        let rd = std::fs::read_dir(td("embeds")).expect("testdata/embeds");
        let mut n = 0;
        for f in rd {
            let p = f.unwrap().path();
            let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
            for e in v["entries"].as_array().unwrap() {
                let s = parse_session(&e["session"]);
                let id = e["id"].as_str().unwrap();
                if !e["embed"].is_null() {
                    assert_eq!(build_embed(&s), e["embed"], "{} embed {id}", p.display());
                }
                if !e["components"].is_null() {
                    assert_eq!(buttons(id, &s), e["components"], "{} components {id}", p.display());
                }
                n += 1;
            }
        }
        assert!(n > 0);
        eprintln!("checked {n} embed entries");
    }

    /// Edge sessions (oracle/review-embed.mjs): number formatting, paging, weights, tuning, names, chest rule.
    #[test]
    fn embed_edge_cases() {
        let path = td("review/embed-edge.json");
        let Ok(text) = std::fs::read_to_string(&path) else { return eprintln!("skipped: {} missing", path.display()) };
        let v: Value = serde_json::from_str(&text).unwrap();
        let mut bad = Vec::new();
        for e in v["entries"].as_array().unwrap() {
            let s = parse_session(&e["session"]);
            let id = e["id"].as_str().unwrap();
            if !e["embed"].is_null() && build_embed(&s) != e["embed"] {
                bad.push(format!("embed {id}: {}\n  rust {}\n  js   {}", s.page, build_embed(&s), e["embed"]));
            }
            if !e["components"].is_null() && buttons(id, &s) != e["components"] {
                bad.push(format!("components {id}"));
            }
        }
        assert!(bad.is_empty(), "{} differ:\n{}", bad.len(), bad.iter().take(6).cloned().collect::<Vec<_>>().join("\n"));
    }
}
