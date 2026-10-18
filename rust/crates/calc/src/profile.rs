//! `profileUpgrades` (src/calc/upgrades.js): the next level of each profile stat source, ranked by % damage gained.

use crate::eval::{weighted_gain, EvalResult};
use crate::helpers::{accessory_superseded, new_item};
use crate::parts::{title_case, title_lower};
use crate::pick::Evaluator;
use crate::{Ctx, Loadout, Model};
use dps_core::config::constants::{ACCESSORY_CATEGORY, CATA_STAT_BONUS, COMBAT_MAX_LEVEL, WOLF_SLAYER_SPEED_LEVELS};
use dps_core::config::enchants::enchant;
use dps_core::config::item_stats::item_stats;
use dps_core::config::pets::{GOLDEN_DRAGON_BANK_CAP, GOLD_COLLECTION_DIGITS};
use dps_core::config::profile::{HARP_SONGS, SB_STRENGTH_EVERY, SKILL_BONUSES, SLAYER_BONUSES, SLAYER_MAX_LEVEL};
use dps_core::data::{accessory_chain_position, accessory_family};
use dps_core::jsfmt::{js_string, to_locale_string};
use dps_core::model::{PriceTables, Weights};
use dps_core::requirements::{meets_requirements, Player};
use dps_core::text::clean_name;
use indexmap::IndexMap;

fn truthy(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::Number(n) => n.as_f64().is_some_and(|x| x != 0.0 && !x.is_nan()),
        serde_json::Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// JS `profileUpgrades(loadout, ctx, weights, prices)`: `(rows sorted by gain, the damage now)`. `prices` (optional)
/// lets it list accessories that have no price and cannot be traded (quest and free talismans).
///
/// `model` is the model `loadout` was built with. Changing a context field that items depend on (Catacombs level,
/// gold collection, maxed collections) evaluates the loadout in a model of its own.
pub fn profile_upgrades(
    model: &mut Model,
    ctx: &Ctx,
    weights: Weights,
    loadout: &Loadout,
    prices: Option<&PriceTables>,
) -> (Vec<(String, f64)>, EvalResult) {
    let base = Evaluator::new(model, ctx, weights).evaluate(loadout);
    let mut rows: Vec<(String, f64)> = Vec::new();

    let try_ctx = |model: &Model, rows: &mut Vec<(String, f64)>, label: String, change: &dyn Fn(&mut Ctx)| {
        let mut c2 = ctx.clone();
        change(&mut c2);
        c2.rederive();
        let after = if model.accepts(&c2) {
            Evaluator::new(model, &c2, weights).evaluate(loadout)
        } else {
            let mut m2 = Model::new(&c2);
            let ser = loadout.to_serialized(model);
            let l2 = Loadout::from_serialized(&mut m2, &ser, c2.pet_profile.bank).expect("loadout round trip");
            Evaluator::new(&m2, &c2, weights).evaluate(&l2)
        };
        let gain = weighted_gain(&base, &after, &weights);
        if gain > 1e-9 {
            rows.push((label, gain));
        }
    };

    if ctx.cata_level < (CATA_STAT_BONUS.len() - 1) as f64 {
        let n = ctx.cata_level;
        try_ctx(model, &mut rows, format!("Catacombs {} → {}", js_string(n), js_string(n + 1.0)), &|c| c.cata_level = n + 1.0);
    }
    let (class_name, level) = match ctx.cls {
        dps_core::kinds::Class::Arch => ("Archer", ctx.archer_level),
        dps_core::kinds::Class::Bers => ("Berserk", ctx.berserk_level),
        dps_core::kinds::Class::Mage => ("Mage", ctx.mage_level),
    };
    if level < 50.0 {
        let cls = ctx.cls;
        try_ctx(model, &mut rows, format!("{class_name} {} → {}", js_string(level), js_string(level + 1.0)), &move |c| match cls {
            dps_core::kinds::Class::Arch => c.archer_level = level + 1.0,
            dps_core::kinds::Class::Bers => c.berserk_level = level + 1.0,
            dps_core::kinds::Class::Mage => c.mage_level = level + 1.0,
        });
    }
    if ctx.combat_level < COMBAT_MAX_LEVEL as f64 {
        let n = ctx.combat_level;
        try_ctx(model, &mut rows, format!("Combat {} → {}", js_string(n), js_string(n + 1.0)), &|c| c.combat_level = n + 1.0);
    }
    if let Some(skills) = &ctx.skills {
        for sb in SKILL_BONUSES {
            let level = skills.get(sb.skill).copied().unwrap_or(0);
            if (level as f64) < sb.max {
                let skill = sb.skill;
                try_ctx(model, &mut rows, format!("{} {} → {}", title_lower(skill), level, level + 1), &move |c| {
                    if let Some(s) = &mut c.skills {
                        s.insert(skill.to_string(), level + 1);
                    }
                });
            }
        }
    }
    if let Some(slayers) = &ctx.slayers {
        let mut bosses: Vec<&str> = SLAYER_BONUSES.iter().map(|s| s.boss).collect();
        if !bosses.contains(&"wolf") {
            bosses.push("wolf");
        }
        for boss in bosses {
            let level = slayers.get(boss).copied().unwrap_or(0);
            if (level as f64) >= SLAYER_MAX_LEVEL {
                continue;
            }
            let speed = boss == "wolf" && WOLF_SLAYER_SPEED_LEVELS.contains(&(level + 1));
            try_ctx(model, &mut rows, format!("{} Slayer {} → {}", title_case(boss), level, level + 1), &move |c| {
                if let Some(s) = &mut c.slayers {
                    s.insert(boss.to_string(), level + 1);
                }
                if speed {
                    c.profile_speed += 1.0;
                }
            });
        }
    }
    if let Some(p) = &ctx.profile_stats {
        let target = ((p.sb_level as f64 / SB_STRENGTH_EVERY).floor() + 1.0) * SB_STRENGTH_EVERY;
        try_ctx(model, &mut rows, format!("SkyBlock level {} → {}", p.sb_level, js_string(target)), &|c| {
            if let Some(p) = &mut c.profile_stats {
                p.sb_level = target as i32;
            }
        });
        for song in HARP_SONGS {
            if !p.harp_songs.iter().any(|k| k == song.key) {
                let key = song.key;
                try_ctx(model, &mut rows, format!("Melody's Harp: {} (perfect)", song.name), &move |c| {
                    if let Some(p) = &mut c.profile_stats {
                        p.harp_songs.push(key.to_string());
                    }
                });
            }
        }
    }
    // Golden Dragon's Shining Scales: +11.1 strength per digit of Gold collection (max 9 digits).
    if let Some(gold) = ctx.pet_profile.gold_collection {
        if loadout.pets.ids.iter().any(|&p| model.pet(p).kind == "GOLDEN_DRAGON") {
            let digits = js_string(gold.floor().max(0.0)).len();
            if (digits as f64) < GOLD_COLLECTION_DIGITS {
                let target = 10f64.powi(digits as i32);
                try_ctx(
                    model,
                    &mut rows,
                    format!("Gold collection to {} (Golden Dragon: +11.1 strength)", to_locale_string(target)),
                    &|c| c.pet_profile.gold_collection = Some(target),
                );
            }
        }
    }
    // The One on a worn necklace: each collection maxed adds its strength.
    {
        let necklaces = [loadout.equipment.get(crate::ids::NECKLACE), loadout.dragon_item(crate::ids::GearSlot::Equipment(crate::ids::NECKLACE as u8))];
        let mut the_one = 0i32;
        for n in necklaces.into_iter().flatten() {
            the_one = the_one.max(model.item(n).enchants.get("ultimate_the_one").copied().unwrap_or(0));
        }
        let e = enchant("ultimate_the_one");
        let min_level = e.and_then(|e| e.min_level).unwrap_or(1);
        if the_one >= min_level {
            if let Some(colls) = &ctx.collections {
                let per = e
                    .and_then(|e| e.the_one)
                    .and_then(|t| t.get((the_one - 1) as usize).copied())
                    .map(js_string)
                    .unwrap_or_else(|| "undefined".to_string());
                for c in &colls.unmaxed {
                    try_ctx(
                        model,
                        &mut rows,
                        format!(
                            "Max {} collection ({} / {}; The One: +{} strength)",
                            c.name,
                            to_locale_string(c.amount),
                            to_locale_string(c.max),
                            per
                        ),
                        &|cx| {
                            if let Some(co) = &mut cx.collections {
                                co.maxed += 1;
                            }
                        },
                    );
                }
            }
        }
    }
    // Free quest accessories (soulbound, no price): only the ones that raise accessory (magical) power. In an upgrade
    // chain, only the next tier that does (a Campfire badge that stays the same rarity is skipped).
    if let Some(prices) = prices {
        let resource = ctx.resource.clone();
        let acc_ids: Vec<String> = {
            let b = model.bag(loadout.accessories);
            b.items.iter().map(|&i| model.item(i).id.clone()).collect()
        };
        let ap_now = model.total_accessory_power(loadout.accessories);
        let player = Player { cata_level: ctx.cata_level, skills: ctx.skills.clone(), slayers: ctx.slayers.clone() };
        // family -> (id, ap, name, bag), in first-insertion order.
        let mut best: IndexMap<String, (String, f64, crate::BagId)> = IndexMap::new();
        for (id, info) in resource.iter() {
            if info.category.as_deref() != Some(ACCESSORY_CATEGORY)
                || !info.soulbound.as_ref().is_some_and(truthy)
                || acc_ids.iter().any(|x| x == id)
                || prices.get(id).is_some()
            {
                continue;
            }
            if accessory_superseded(id, acc_ids.iter().map(|s| s.as_str()), Some(&resource)) || !meets_requirements(Some(info), Some(&player)) {
                continue;
            }
            let item = model.intern_item(new_item(id, &resource));
            let bag = model.bag_with(loadout.accessories, item);
            let ap = model.total_accessory_power(bag) - ap_now;
            if !(ap > 0.0) {
                continue;
            }
            let family = accessory_family(id).unwrap_or(id).to_string();
            match best.get_mut(&family) {
                Some(prev) if accessory_chain_position(id) >= accessory_chain_position(&prev.0) => {}
                Some(prev) => *prev = (id.clone(), ap, bag),
                None => {
                    best.insert(family, (id.clone(), ap, bag));
                }
            }
        }
        for (id, ap, bag) in best.into_values() {
            let info = &resource[&id];
            let note = item_stats(&id).and_then(|d| d.note).filter(|n| !n.is_empty());
            let mut next = loadout.clone();
            next.accessories = bag;
            let ev = Evaluator::new(model, ctx, weights);
            let picked = ev.select_pet(&next);
            let gain = weighted_gain(&base, &ev.evaluate(&picked), &weights);
            if gain > 1e-9 {
                let name = clean_name(info.name.as_deref().unwrap_or(&id));
                let label = format!("{} (free quest accessory: +{} magical power{})", name, js_string(ap), note.map(|n| format!(", {n}")).unwrap_or_default());
                rows.push((label, gain));
            }
        }
    }
    let _ = GOLDEN_DRAGON_BANK_CAP;
    rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    (rows, base)
}
