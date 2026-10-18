//! Differential test of the pure helpers over every item in the resource (oracle/review-helpers.mjs).
mod common;
use common::*;
use dps_calc::helpers::*;
use dps_calc::itemstats::compute_base_stats;
use dps_core::kinds::{Class, FloorMode};
use serde_json::Value;

fn cmp_loose(tag: &str, got: &dps_calc::Loose, want: &Value, bad: &mut Vec<String>) {
    let want_map = want.as_array().unwrap();
    let mut got_n = 0;
    for (s, v) in got.iter() {
        got_n += 1;
        let w = want_map.iter().find(|p| p[0].as_str() == Some(s.js_name()));
        match w {
            None => bad.push(format!("{tag}: Rust has {} = {v}, JS lacks it", s.js_name())),
            Some(p) => {
                let wv = match &p[1] {
                    Value::String(s) => match s.as_str() {
                        "NaN" => f64::NAN,
                        "Infinity" => f64::INFINITY,
                        "-Infinity" => f64::NEG_INFINITY,
                        _ => panic!(),
                    },
                    x => x.as_f64().unwrap(),
                };
                if !(v == wv || (v.is_nan() && wv.is_nan())) {
                    bad.push(format!("{tag}: {} got {v:e} want {wv:e}", s.js_name()));
                }
            }
        }
    }
    if got_n != want_map.len() {
        bad.push(format!("{tag}: key count got {got_n} want {}: {want}", want_map.len()));
    }
}

#[test]
fn helpers_match_js() {
    let Some(resource) = resource() else { return };
    let path = testdata().join("review-helpers.jsonl");
    if !path.exists() {
        eprintln!("skipping");
        return;
    }
    let mut bad: Vec<String> = Vec::new();
    let mut n = 0;
    read_lines(&path, |_, text| {
        let v: Value = serde_json::from_str(text).unwrap();
        n += 1;
        let id = v["id"].as_str().unwrap();
        match v["k"].as_str().unwrap() {
            "new" => {
                let got = serde_json::to_value(new_item(id, &resource)).unwrap();
                if got != v["item"] {
                    bad.push(format!("new {id}: got {got} want {}", v["item"]));
                }
            }
            "base" => {
                let cls = Class::from_js(v["cls"].as_str().unwrap()).unwrap();
                let cata = match &v["cata"] {
                    Value::String(_) => f64::INFINITY,
                    x => x.as_f64().unwrap(),
                };
                let floor = FloorMode::from_js(v["floor"].as_str().unwrap()).unwrap();
                let t = compute_base_stats(id, &resource, Some(cls), cata, floor);
                let tag = format!("base {id} {} {} {}", v["cls"], v["cata"], v["floor"]);
                cmp_loose(&format!("{tag} global"), &t.global, &v["out"]["global"], &mut bad);
                cmp_loose(&format!("{tag} wither"), &t.wither, &v["out"]["wither"], &mut bad);
                cmp_loose(&format!("{tag} dragon"), &t.dragon, &v["out"]["dragon"], &mut bad);
            }
            "misc" => {
                let mut it = new_item(id, &resource);
                it.recomb = v["recomb"].as_bool().unwrap();
                let mut a = it.clone();
                a.scrolls = vec!["ULTIMATE_WITHER_SCROLL".into()];
                let mut b = it.clone();
                b.scrolls = vec!["IMPLOSION_SCROLL".into(), "SHADOW_WARP_SCROLL".into(), "WITHER_SHIELD_SCROLL".into()];
                let got = (item_rarity_of(&it).to_string(), can_recomb(&it, &resource), is_dungeon(&it, &resource), has_wither_impact(&a), has_wither_impact(&b));
                let want = (v["rarity"].as_str().unwrap().to_string(), v["canRecomb"].as_bool().unwrap(), v["dungeon"].as_bool().unwrap(), v["wi"].as_bool().unwrap(), v["wi3"].as_bool().unwrap());
                if got != want {
                    bad.push(format!("misc {id} {}: got {got:?} want {want:?}", v["recomb"]));
                }
            }
            "rar" => {
                let base = v["base"].as_str().unwrap();
                let mut it = new_item(id, &resource);
                it.base_rarity = base.to_string();
                it.recomb = true;
                let a = item_rarity_of(&it).to_string();
                it.recomb = false;
                let b = item_rarity_of(&it).to_string();
                let c = stat_rarity(base).to_string();
                let w: Vec<String> = v["out"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
                if vec![a.clone(), b.clone(), c.clone()] != w {
                    bad.push(format!("rar {id} {base}: got {a} {b} {c} want {w:?}"));
                }
            }
            k => panic!("{k}"),
        }
    });
    eprintln!("helpers: {n} lines, {} problems", bad.len());
    for m in bad.iter().take(25) {
        eprintln!("  {m}");
    }
    assert!(bad.is_empty());
}
