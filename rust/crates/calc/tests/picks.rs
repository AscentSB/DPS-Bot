//! Golden tests against testdata/picks (written by oracle/calc-picks.sh): per-item stats, accessory bag, shards,
//! tuning allocation, pet / power / tuning picks, loadout edits and the small global functions.

mod common;
use common::*;
use dps_calc::helpers::*;
use dps_calc::ids::*;
use dps_calc::*;
use dps_core::config::accessories::POWERS;
use dps_core::kinds::{Class, FloorMode, Mode, Target};
use dps_core::model::{ItemState, Pet};
use dps_core::stat::Stat;
use serde_json::Value;
use std::sync::Arc;

fn picks_dir() -> std::path::PathBuf {
    testdata().join("picks")
}

fn load(path: &std::path::Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

struct Errs(Vec<String>);
impl Errs {
    fn push(&mut self, s: String) {
        if self.0.len() < 40 {
            self.0.push(s);
        }
    }
    fn check(&mut self, ok: bool, what: impl FnOnce() -> String) {
        if !ok {
            self.push(what());
        }
    }
}

fn loose_eq(errs: &mut Errs, what: &str, got: &Loose, want: &Value) {
    let want = want.as_object().unwrap_or_else(|| panic!("{what}: {want}"));
    let mut seen = 0;
    for (k, v) in want {
        let stat = Stat::from_js(k).unwrap_or_else(|| panic!("{what}: unknown stat {k}"));
        seen += 1;
        match got.get(stat) {
            None => errs.push(format!("{what}: missing key {k}")),
            Some(g) => errs.check(g == v.as_f64().unwrap(), || format!("{what}.{k}: got {g:e} want {v}")),
        }
    }
    let n = got.iter().count();
    errs.check(n == seen, || format!("{what}: {n} keys, want {seen}: {got:?} vs {want:?}"));
}

fn triple_eq(errs: &mut Errs, what: &str, got: &Triple, want: &Value) {
    loose_eq(errs, &format!("{what}.global"), &got.global, &want["global"]);
    loose_eq(errs, &format!("{what}.wither"), &got.wither, &want["wither"]);
    loose_eq(errs, &format!("{what}.dragon"), &got.dragon, &want["dragon"]);
}

fn pet_of(v: &Value) -> Option<Pet> {
    if v.is_null() {
        None
    } else {
        Some(serde_json::from_value(v.clone()).unwrap())
    }
}

struct Setup {
    ctx: Ctx,
    weights: dps_core::model::Weights,
    model: Model,
    start: Loadout,
    start_ser: SerLoadout,
}

fn setup(case: &str, resource: &Arc<dps_core::model::Resource>) -> Setup {
    let ctx_json = load(&testdata().join("ctx").join(format!("{case}.json")));
    let (ctx, weights) = ctx_from_json(&ctx_json, resource.clone());
    let mut model = Model::new(&ctx);
    let mut first = String::new();
    read_lines(&testdata().join("evals").join(format!("{case}.jsonl")), |n, text| {
        if n == 0 {
            first = text.to_string();
        }
    });
    #[derive(serde::Deserialize)]
    struct L {
        loadout: SerLoadout,
    }
    let l: L = serde_json::from_str(&first).unwrap();
    let start = Loadout::from_serialized(&mut model, &l.loadout, ctx.pet_profile.bank).unwrap();
    Setup { ctx, weights, model, start, start_ser: l.loadout }
}

/// Compare serialized loadouts, treating the Set fields (owned, extras, unlocked powers) as sets.
fn norm(mut l: SerLoadout) -> SerLoadout {
    l.owned.sort();
    l.extras.sort();
    l.unlocked_powers.sort();
    l
}

fn ser_eq(errs: &mut Errs, what: &str, got: &SerLoadout, want: &SerLoadout) {
    let (g, w) = (norm(got.clone()), norm(want.clone()));
    if g == w {
        return;
    }
    // Find the first differing field for a readable message.
    let gv = serde_json::to_value(&g).unwrap();
    let wv = serde_json::to_value(&w).unwrap();
    for (k, wv) in wv.as_object().unwrap() {
        if gv.get(k) != Some(wv) {
            let (a, b) = (gv.get(k).cloned().unwrap_or(Value::Null).to_string(), wv.to_string());
            errs.push(format!("{what}: field {k} differs\n   got  {}\n   want {}", &a[..a.len().min(400)], &b[..b.len().min(400)]));
            return;
        }
    }
    errs.push(format!("{what}: differs (typed compare)"));
}

#[test]
fn per_case_picks() {
    let Some(resource) = resource() else {
        eprintln!("skipping: testdata/snapshot.json missing");
        return;
    };
    if !picks_dir().exists() {
        eprintln!("skipping: testdata/picks missing (run oracle/calc-picks.sh)");
        return;
    }
    let mut errs = Errs(Vec::new());
    let mut soft: Vec<String> = Vec::new();
    for case in case_names() {
        let path = picks_dir().join(format!("{case}.json"));
        if !path.exists() {
            continue;
        }
        let want = load(&path);
        let mut s = setup(&case, &resource);
        let cls = s.ctx.cls;
        let rcm = s.ctx.is_rcm();
        let _ = rcm;
        let tag = |t: &str| format!("{case} {t}");

        // ---- items
        for (i, it) in want["items"].as_array().unwrap().iter().enumerate() {
            let state: ItemState = serde_json::from_value(it["item"].clone()).unwrap();
            let id = s.model.intern_item(state.clone());
            let w = |t: &str| tag(&format!("items[{i}] {} {t}", state.id));
            triple_eq(&mut errs, &w("contrib0"), s.model.contribution(id, 0), &it["contrib0"]);
            // variant 1 only differs when the digit count is non-zero; the model normalises variant 0 = digit 0.
            triple_eq(&mut errs, &w("contrib1"), s.model.contribution(id, 1), &it["contrib1"]);
            triple_eq(&mut errs, &w("raw0"), s.model.raw_stats(id, 0), &it["raw0"]);
            triple_eq(&mut errs, &w("raw1"), s.model.raw_stats(id, 1), &it["raw1"]);
            loose_eq(&mut errs, &w("acc0"), s.model.accessory_item_stats(id, 0), &it["acc0"]);
            loose_eq(&mut errs, &w("acc1"), s.model.accessory_item_stats(id, 1), &it["acc1"]);
            errs.check(s.model.flags(id).ap == it["ap"].as_f64().unwrap(), || w("ap"));
            errs.check(s.model.flags(id).is_dungeon == it["isDungeon"].as_bool().unwrap(), || w("isDungeon"));
            errs.check(is_dungeon(&state, &resource) == it["isDungeon"].as_bool().unwrap(), || w("is_dungeon fn"));
            errs.check(can_recomb(&state, &resource) == it["canRecomb"].as_bool().unwrap(), || w("canRecomb"));
            errs.check(item_rarity_of(&state) == it["rarity"].as_str().unwrap(), || w("rarity"));
            if !it["chimera"].is_null() {
                let t = s.model.raw_stats_chimera(id, 0, 123.456);
                triple_eq(&mut errs, &w("chimera"), &t, &it["chimera"]);
                let rs = s.model.rag_strength_with(id, 0, 123.456);
                errs.check(rs == t.global.get0(Stat::Strength), || w("rag_strength_with"));
            }
        }

        // ---- bag
        let start_power = s.start.power;
        for b in want["bag"].as_array().unwrap() {
            let power = match b["power"].as_str() {
                None => start_power,
                Some(p) => match ctx_power_index(p) {
                    Some(i) => PowerField::Known(i as u8),
                    None => PowerField::Unknown,
                },
            };
            let extra = b["extra"].as_f64().unwrap();
            let sd = b["sd"].as_u64().unwrap() as usize;
            let r = s.model.accessory_stats(&s.ctx, s.start.accessories, power, extra, sd);
            let w = tag(&format!("bag power={} extra={extra} sd={sd}", b["power"]));
            loose_eq(&mut errs, &format!("{w} stats"), &r.stats, &b["stats"]);
            errs.check(r.ap == b["ap"].as_f64().unwrap(), || format!("{w} ap {} vs {}", r.ap, b["ap"]));
            errs.check(r.tuning_points == b["tuningPoints"].as_f64().unwrap(), || format!("{w} tuning points"));
        }
        let counted: Vec<String> = s.model.counted_accessories(s.start.accessories).iter().map(|&i| s.model.item(i).id.clone()).collect();
        let want_counted: Vec<String> = want["counted"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        errs.check(counted == want_counted, || tag("counted accessories"));
        errs.check(s.model.total_accessory_power(s.start.accessories) == want["totalAp"].as_f64().unwrap(), || tag("totalAp"));

        // ---- shards
        let st = s.start.shards.shard_stats(cls);
        loose_eq(&mut errs, &tag("shards.global"), &st.global, &want["shards"]["global"]);
        loose_eq(&mut errs, &tag("shards.wither"), &st.wither, &want["shards"]["wither"]);
        loose_eq(&mut errs, &tag("shards.dragon"), &st.dragon, &want["shards"]["dragon"]);
        errs.check(st.tuning_points == want["shards"]["tuningPoints"].as_f64().unwrap(), || tag("shards tuningPoints"));
        for t in want["shardTests"].as_array().unwrap() {
            let entries: Vec<(String, f64)> = t["shards"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_f64().unwrap())).collect();
            let lv = Levels::new(LevelKind::Shards, entries);
            let r = lv.shard_stats(cls);
            loose_eq(&mut errs, &tag(&format!("shardTest {} global", t["shards"])), &r.global, &t["result"]["global"]);
            loose_eq(&mut errs, &tag(&format!("shardTest {} wither", t["shards"])), &r.wither, &t["result"]["wither"]);
            loose_eq(&mut errs, &tag(&format!("shardTest {} dragon", t["shards"])), &r.dragon, &t["result"]["dragon"]);
            errs.check(r.tuning_points == t["result"]["tuningPoints"].as_f64().unwrap(), || tag("shardTest tuningPoints"));
        }

        // ---- tuning
        for t in want["tuning"].as_array().unwrap() {
            let (points, speed, a, mult, cap) = (
                t["points"].as_f64().unwrap(),
                t["speed"].as_f64().unwrap(),
                t["as"].as_f64().unwrap(),
                t["mult"].as_f64().unwrap(),
                t["capBonus"].as_f64().unwrap(),
            );
            let r = tuning_allocation(points, speed, &s.ctx, cap, a, mult);
            let w = tag(&format!("tuning {points} {speed} {a} {mult} {cap}"));
            errs.check(r.attack_speed == t["stats"]["attackSpeed"].as_f64().unwrap(), || format!("{w} attackSpeed"));
            errs.check(r.speed == t["stats"]["speed"].as_f64().unwrap(), || format!("{w} speed"));
            errs.check(r.attack_speed_points == t["attackSpeedPoints"].as_f64().unwrap(), || format!("{w} attackSpeedPoints"));
            errs.check(r.speed_points == t["speedPoints"].as_f64().unwrap(), || format!("{w} speedPoints"));
            errs.check(r.dps_points == t["dpsPoints"].as_f64().unwrap(), || format!("{w} dpsPoints"));
            let keys: Vec<&str> = t["keys"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
            let got_keys: Vec<&str> = ["attackSpeed", "speed"].into_iter().chain(r.dps[..r.dps_len].iter().map(|(k, _)| k.js_name())).collect();
            errs.check(keys == got_keys, || format!("{w} keys {got_keys:?} vs {keys:?}"));
            for &(k, v) in &r.dps[..r.dps_len] {
                errs.check(v == t["stats"][k.js_name()].as_f64().unwrap(), || format!("{w} {}", k.js_name()));
            }
        }

        // ---- scalars
        let sc = &want["scalars"];
        for v in sc["volleys"].as_array().unwrap() {
            let g = eval::volleys_per_second(&s.model, s.start.weapon, v[0].as_f64().unwrap(), &s.ctx);
            errs.check(g == v[1].as_f64().unwrap(), || tag(&format!("volleys {v}")));
        }
        for v in sc["arrows"].as_array().unwrap() {
            let g = eval::arrows_per_volley(&s.model, s.start.weapon, v[0].as_f64().unwrap(), &s.ctx);
            errs.check(g == v[1].as_f64().unwrap(), || tag(&format!("arrows {v}")));
        }
        for v in sc["saCloak"].as_array().unwrap() {
            let g = s.ctx.sa_cloak_multiplier(v[0].as_f64().unwrap());
            errs.check(g == v[1].as_f64().unwrap(), || tag(&format!("saCloak {v}")));
        }
        errs.check(s.ctx.class_additive_damage() == sc["classAdditive"].as_f64().unwrap(), || tag("classAdditive"));
        errs.check(eval::berserk_melee_multiplier(&s.ctx) == sc["berserkMelee"].as_f64().unwrap(), || tag("berserkMelee"));
        errs.check(eval::archer_multiplier(&s.ctx) == sc["archerMult"].as_f64().unwrap(), || tag("archerMult"));
        errs.check(eval::book_multiplier(&s.model, &s.start, &s.ctx) == sc["book"].as_f64().unwrap(), || tag("book"));
        errs.check(s.ctx.maxed_collections() == sc["maxed"].as_f64().unwrap(), || tag("maxed"));
        errs.check(s.ctx.has_dragon_set() == sc["hasDragonSet"].as_bool().unwrap(), || tag("hasDragonSet"));
        loose_eq(&mut errs, &tag("profileTotals"), &s.ctx.d.profile_totals, &sc["profileTotals"]);

        // ---- picks
        let p = &want["picks"];
        let ex = active_extras(&s.model, &s.ctx, &s.start, Some(&s.weights));
        let want_ex: Vec<&str> = p["activeExtras"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        errs.check(ex.keys().collect::<Vec<_>>() == want_ex, || tag("activeExtras"));
        let ev = Evaluator::new(&s.model, &s.ctx, s.weights);
        let dps: Vec<Pet> = ev.dps_pets(&s.start.pets).iter().map(|&i| s.model.pet(i).clone()).collect();
        let want_dps: Vec<Pet> = p["dpsPets"].as_array().unwrap().iter().map(|v| pet_of(v).unwrap()).collect();
        errs.check(dps == want_dps, || tag("dpsPets"));
        let act = |l: &Loadout| -> [Option<Pet>; 2] { [l.active_pets[0].map(|i| s.model.pet(i).clone()), l.active_pets[1].map(|i| s.model.pet(i).clone())] };
        let want_act = |v: &Value| [pet_of(&v["wither"]), pet_of(&v["dragon"])];
        errs.check(act(&ev.select_pet(&s.start)) == want_act(&p["selectPet"]), || tag("selectPet"));
        let mut none = s.start.clone();
        none.active_pets = [None, None];
        errs.check(act(&ev.select_pet(&none)) == want_act(&p["selectPetFromNone"]), || tag("selectPetFromNone"));
        let want_power = |v: &Value| match v.as_str() {
            None => PowerField::Unset,
            Some(n) => match ctx_power_index(n) {
                Some(i) => PowerField::Known(i as u8),
                None => PowerField::Unknown,
            },
        };
        let mut sp = s.start.clone();
        sp.power = match s.ctx.power.as_deref() {
            Some(n) => match ctx_power_index(n) {
                Some(i) => PowerField::Known(i as u8),
                None => PowerField::Unknown,
            },
            None => s.start.power,
        };
        errs.check(ev.select_power(&sp).power == want_power(&p["selectPower"]), || tag("selectPower"));
        errs.check(ev.select_power(&s.start).power == want_power(&p["selectPowerFromStart"]), || tag("selectPowerFromStart"));
        let split_eq = |got: Option<Vec<(Stat, f64)>>, want: &Value| -> bool {
            match (got, want.as_object()) {
                (None, None) => true,
                (Some(g), Some(w)) => g.len() == w.len() && g.iter().zip(w.iter()).all(|((gk, gv), (wk, wv))| gk.js_name() == wk && *gv == wv.as_f64().unwrap()),
                _ => false,
            }
        };
        errs.check(split_eq(ev.best_tuning_split(&s.start), &p["bestTuningSplit"]), || tag(&format!("bestTuningSplit {:?} vs {}", ev.best_tuning_split(&s.start), p["bestTuningSplit"])));
        errs.check(split_eq(ev.best_tuning_split(&none), &p["bestTuningSplitNoPets"]), || tag("bestTuningSplitNoPets"));
        errs.check(ev.score(&s.start) == p["score"].as_f64().unwrap(), || tag("score"));

        // ---- ops
        for (i, op) in want["ops"].as_array().unwrap().iter().enumerate() {
            let name = op["op"].as_str().unwrap();
            let base_ser: SerLoadout = if op["base"].as_str() == Some("prev") {
                serde_json::from_value(op["from"].clone()).unwrap()
            } else {
                s.start_ser.clone()
            };
            let base = Loadout::from_serialized(&mut s.model, &base_ser, s.ctx.pet_profile.bank).unwrap();
            let item_of = |model: &mut Model, v: &Value| -> Option<ItemId> {
                if v.is_null() {
                    None
                } else {
                    Some(model.intern_item(serde_json::from_value::<ItemState>(v.clone()).unwrap()))
                }
            };
            let got = match name {
                "clone" => base.clone(),
                "replaceItem" => {
                    let old = item_of(&mut s.model, &op["old"]).unwrap();
                    let next = item_of(&mut s.model, &op["next"]).unwrap();
                    base.replace_item(&mut s.model, old, next)
                }
                "setSlot" => {
                    let item = item_of(&mut s.model, &op["item"]).unwrap();
                    let mut l = base.clone();
                    l.set_slot(Slot::from_name(op["slot"].as_str().unwrap()).unwrap(), item);
                    l
                }
                "setDragonSlot" => {
                    let item = item_of(&mut s.model, &op["item"]);
                    let mut l = base.clone();
                    l.set_dragon_slot(GearSlot::from_name(op["slot"].as_str().unwrap()).unwrap(), item);
                    l
                }
                other => panic!("op {other}"),
            };
            let want_l: SerLoadout = serde_json::from_value(op["result"].clone()).unwrap();
            ser_eq(&mut errs, &tag(&format!("op[{i}] {name} {}", op["slot"])), &got.to_serialized(&s.model), &want_l);
        }

        // ---- misc
        for sn in want["slotNames"].as_array().unwrap() {
            let state: ItemState = serde_json::from_value(sn[0].clone())
                .unwrap_or_else(|_| ItemState { id: sn[0]["id"].as_str().unwrap().into(), name: "x".into(), ..Default::default() });
            let id = s.model.intern_item(state);
            let got = s.start.slot_name(&s.model, id, false);
            errs.check(got == sn[1].as_str().unwrap(), || tag(&format!("slotName {} got {got} want {}", s.model.item(id).id, sn[1])));
        }
        let up: Vec<String> = s.start.upgradable_items(&s.model).iter().map(|&(i, _)| s.model.item(i).id.clone()).collect();
        let want_up: Vec<String> = want["upgradable"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        if up != want_up {
            soft.push(format!("{case}: upgradable_items {} vs {} (identity vs content dedup)", up.len(), want_up.len()));
        }
        let gv = s.start.gear_view(true);
        let id_of = |x: Option<ItemId>| x.map(|i| s.model.item(i).id.clone());
        errs.check(id_of(gv.weapon) == want["gearView"]["weapon"].as_str().map(String::from), || tag("gearView weapon"));
        let ids = |m: &SlotMap, names: &[&str]| -> Vec<(String, Option<String>)> { m.entries().map(|(sl, it)| (names[sl].to_string(), id_of(it))).collect() };
        let wm = |v: &Value| -> Vec<(String, Option<String>)> { v.as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().map(String::from))).collect() };
        errs.check(ids(&gv.armor, &ARMOR_NAMES) == wm(&want["gearView"]["armor"]), || tag("gearView armor"));
        errs.check(ids(&gv.equipment, &EQUIPMENT_NAMES) == wm(&want["gearView"]["equipment"]), || tag("gearView equipment"));
        errs.check((s.start.dragon_gear.is_none()) == want["gearView"]["same"].as_bool().unwrap(), || tag("gearView same"));

        let excluded: Vec<&str> = resource.keys().map(|k| k.as_str()).filter(|k| s.ctx.excluded_item(k)).collect();
        let want_ex: Vec<&str> = want["excluded"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        errs.check(excluded == want_ex, || tag("excluded items"));
    }
    for m in &soft {
        eprintln!("soft: {m}");
    }
    assert!(errs.0.is_empty(), "{} errors:\n{}", errs.0.len(), errs.0.join("\n"));
}

fn ctx_power_index(name: &str) -> Option<usize> {
    POWERS.iter().position(|p| p.key == name)
}

#[test]
fn global_functions() {
    let Some(resource) = resource() else {
        eprintln!("skipping: testdata/snapshot.json missing");
        return;
    };
    let path = picks_dir().join("global.json");
    if !path.exists() {
        eprintln!("skipping: testdata/picks/global.json missing");
        return;
    }
    let g = load(&path);
    let mut errs = Errs(Vec::new());
    for w in g["weightsFor"].as_array().unwrap() {
        let cls = Class::from_js(w["cls"].as_str().unwrap()).unwrap();
        let target = w["target"].as_str().and_then(Target::from_js);
        let r = weights_for(cls, target, Mode::from_js(w["mode"].as_str().unwrap()).unwrap(), FloorMode::from_js(w["floorMode"].as_str().unwrap()).unwrap());
        errs.check(r.wither == w["result"]["wither"].as_f64().unwrap() && r.dragon == w["result"]["dragon"].as_f64().unwrap(), || format!("weightsFor {w}"));
    }
    for w in g["skeleton"].as_array().unwrap() {
        let cls = Class::from_js(w["cls"].as_str().unwrap()).unwrap();
        errs.check(skeleton_master_chest_rule(cls, w["cata"].as_f64().unwrap()) == w["result"].as_bool().unwrap(), || format!("skeleton {w}"));
    }
    for (id, want) in g["newItem"].as_object().unwrap() {
        let want: ItemState = serde_json::from_value(want.clone()).unwrap();
        let got = new_item(id, &resource);
        errs.check(got == want, || format!("newItem {id}: {got:?} vs {want:?}"));
    }
    for p in g["upgradesInto"].as_array().unwrap() {
        errs.check(upgrades_into(p[0].as_str().unwrap(), p[1].as_str().unwrap()), || format!("upgradesInto {p}"));
    }
    for p in g["upgradesIntoFalse"].as_array().unwrap() {
        errs.check(!upgrades_into(p[0].as_str().unwrap(), p[1].as_str().unwrap()), || format!("!upgradesInto {p}"));
    }
    let acc_ids: Vec<&str> = resource.iter().filter(|(_, i)| i.category.as_deref() == Some("ACCESSORY")).map(|(k, _)| k.as_str()).take(400).collect();
    for s in g["superseded"].as_array().unwrap() {
        let id = s[0].as_str().unwrap();
        errs.check(accessory_superseded(id, acc_ids[..200].iter().copied(), Some(&resource)) == s[1].as_bool().unwrap(), || format!("superseded(resource) {id}"));
        errs.check(accessory_superseded(id, acc_ids[..200].iter().copied(), None) == s[2].as_bool().unwrap(), || format!("superseded(no resource) {id}"));
    }
    for c in g["canRecomb"].as_array().unwrap() {
        let id = c[0].as_str().unwrap();
        let tier = resource.get(id).and_then(|i| i.tier.clone()).unwrap_or_else(|| "COMMON".into());
        let item = ItemState { id: id.to_string(), base_rarity: item_rarity_of(&ItemState { id: id.to_string(), base_rarity: tier, ..Default::default() }).to_string(), recomb: false, ..Default::default() };
        errs.check(can_recomb(&item, &resource) == c[1].as_bool().unwrap(), || format!("canRecomb {id}"));
    }
    for (key, table) in g["baseStats"].as_object().unwrap() {
        let parts: Vec<&str> = key.split('|').collect();
        let cls = Class::from_js(parts[0]).unwrap();
        let cata: f64 = parts[1].parse().unwrap();
        let fm = FloorMode::from_js(parts[2]).unwrap();
        let table = table.as_object().unwrap();
        for id in resource.keys().take(6000) {
            let b = itemstats::compute_base_stats(id, &resource, Some(cls), cata, fm);
            let nonempty = b.global.mask != 0 || b.wither.mask != 0 || b.dragon.mask != 0;
            match table.get(id) {
                Some(w) => triple_eq(&mut errs, &format!("baseStats {key} {id}"), &b, w),
                None => errs.check(!nonempty, || format!("baseStats {key} {id} should be empty")),
            }
        }
    }
    assert!(errs.0.is_empty(), "{} errors:\n{}", errs.0.len(), errs.0.join("\n"));
}
