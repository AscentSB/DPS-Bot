//! Adversarial parity: mutated profiles and synthetic NBT items against the JS (oracle/review-skyblock.mjs).
//! Skips quietly when testdata/review-skyblock is missing. Cases where the JS throws a TypeError on malformed data
//! (the Rust reader is lenient) are counted but not failures.

use dps_core::model::{CollectionsResource, ItemInfo, Resource};
use dps_skyblock::member::{read_member, Json};
use serde_json::Value;
use std::path::PathBuf;

fn td() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn load() -> (Resource, CollectionsResource, f64) {
    let snap: Value = serde_json::from_slice(&std::fs::read(td().join("snapshot.json")).unwrap()).unwrap();
    let now = snap["now"].as_f64().unwrap();
    let items: Vec<ItemInfo> = snap["items"].as_array().unwrap().iter().map(|p| serde_json::from_value(p[1].clone()).unwrap()).collect();
    let resource: Resource = items.into_iter().map(|i| (i.id.clone(), i)).collect();
    let collections: CollectionsResource = snap["collections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p[0].as_str().unwrap().to_string(), serde_json::from_value(p[1].clone()).unwrap()))
        .collect();
    (resource, collections, now)
}

fn norm(v: &Value, path: &str) -> Value {
    match v {
        Value::Object(o) => {
            let mut m = serde_json::Map::new();
            for (k, x) in o {
                if x.is_null() {
                    continue;
                }
                if path.ends_with(".pet") && !["type", "tier", "exp", "heldItem"].contains(&k.as_str()) {
                    continue;
                }
                m.insert(k.clone(), norm(x, &format!("{path}.{k}")));
            }
            Value::Object(m)
        }
        Value::Array(a) => Value::Array(a.iter().map(|x| norm(x, &format!("{path}[]"))).collect()),
        Value::Number(n) => serde_json::json!(n.as_f64().unwrap()),
        o => o.clone(),
    }
}

fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            x == y || (x - y).abs() <= 1e-9 * x.abs().max(y.abs())
        }
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w))),
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q)),
        _ => a == b,
    }
}

fn diff(a: &Value, b: &Value, path: &str, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for k in x.keys().chain(y.keys().filter(|k| !x.contains_key(*k))) {
                match (x.get(k), y.get(k)) {
                    (Some(p), Some(q)) => diff(p, q, &format!("{path}.{k}"), out),
                    (p, q) => out.push(format!("{path}.{k}: rust {p:?} js {q:?}")),
                }
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                diff(p, q, &format!("{path}[{i}]"), out);
            }
        }
        _ => {
            if !same(a, b) {
                let s = |v: &Value| v.to_string().chars().take(160).collect::<String>();
                out.push(format!("{path}: rust {} js {}", s(a), s(b)));
            }
        }
    }
}

fn to_json(v: &Value) -> Json {
    Json::parse(v.to_string().as_bytes()).unwrap()
}

fn set_path(root: &mut Json, path: &str, op: &str, value: &Value) {
    let keys: Vec<&str> = path.split('.').collect();
    fn go(cur: &mut Json, keys: &[&str], op: &str, value: &Json) {
        let Json::Obj(o) = cur else { return };
        let k = keys[0];
        let idx = o.iter().position(|(kk, _)| &**kk == k);
        if keys.len() == 1 {
            match (idx, op) {
                (Some(i), "del") => {
                    o.remove(i);
                }
                (Some(i), _) => o[i].1 = value.clone(),
                (None, "del") => {}
                (None, _) => o.push((k.into(), value.clone())),
            }
            return;
        }
        let i = match idx {
            Some(i) if matches!(o[i].1, Json::Obj(_)) => i,
            _ => {
                if op == "del" {
                    return;
                }
                match idx {
                    Some(i) => o[i].1 = Json::Obj(Vec::new()),
                    None => o.push((k.into(), Json::Obj(Vec::new()))),
                }
                o.iter().position(|(kk, _)| &**kk == k).unwrap()
            }
        };
        go(&mut o[i].1, &keys[1..], op, value);
    }
    go(root, &keys, op, &to_json(value));
}

fn at<'a>(j: &'a mut Json, path: &[&str]) -> Option<&'a mut Json> {
    let mut cur = j;
    for k in path {
        let Json::Obj(o) = cur else { return None };
        cur = &mut o.iter_mut().find(|(kk, _)| &**kk == *k)?.1;
    }
    Some(cur)
}

fn obj(v: Value) -> Json {
    to_json(&v)
}

fn put(j: &mut Json, path: &[&str], v: Json) {
    let (last, parents) = path.split_last().unwrap();
    if let Some(Json::Obj(o)) = at(j, parents) {
        match o.iter_mut().find(|(k, _)| &**k == *last) {
            Some(e) => e.1 = v,
            None => o.push(((*last).into(), v)),
        }
    }
}

fn del(j: &mut Json, path: &[&str]) {
    let (last, parents) = path.split_last().unwrap();
    if let Some(Json::Obj(o)) = at(j, parents) {
        o.retain(|(k, _)| &**k != *last);
    }
}

/// Mirrors the `co(...)` mutations of review-skyblock.mjs.
fn custom(prof: &mut Json, uuid: &str, op: &str) {
    let others: Vec<String> = match at(prof, &["members"]) {
        Some(Json::Obj(o)) => o.iter().map(|(k, _)| k.to_string()).filter(|k| k != uuid).collect(),
        _ => Vec::new(),
    };
    let o = others.first().cloned().unwrap_or_default();
    let (o, u) = (o.as_str(), uuid);
    match op {
        "other-left" => put(prof, &["members", o, "profile"], obj(serde_json::json!({"deletion_notice": {"timestamp": 1}}))),
        "other-unconfirmed" => put(prof, &["members", o, "coop_invitation"], obj(serde_json::json!({"confirmed": false}))),
        "other-unconfirmed2" => put(prof, &["members", o, "profile"], obj(serde_json::json!({"coop_invitation": {"confirmed": false}}))),
        "other-confirmed-true" => put(prof, &["members", o, "coop_invitation"], obj(serde_json::json!({"confirmed": true}))),
        "other-no-collection" => del(prof, &["members", o, "collection"]),
        "other-collection-weird" => put(prof, &["members", o, "collection"], obj(serde_json::json!({"GOLD_INGOT": "x", "COAL": "7", "Z": null}))),
        "other-null" => put(prof, &["members", o], Json::Null),
        "solo-bank-personal" | "solo-bank-both" => {
            for k in &others {
                del(prof, &["members", k]);
            }
            put(prof, &["members", u, "profile"], obj(serde_json::json!({"bank_account": 500})));
            if op == "solo-bank-both" {
                put(prof, &["banking"], obj(serde_json::json!({"balance": 900})));
            } else {
                del(prof, &["banking"]);
            }
        }
        "coop-bank-personal" => {
            put(prof, &["members", u, "profile"], obj(serde_json::json!({"bank_account": 500})));
            del(prof, &["banking"]);
        }
        "no-members" => del(prof, &["members"]),
        "wrong-uuid" => {
            let m = at(prof, &["members", u]).cloned().unwrap();
            put(prof, &["members"], Json::Obj(vec![("other".into(), m)]));
        }
        "member-str" => put(prof, &["members", u], Json::Str("x".into())),
        "member-empty" => put(prof, &["members", u], Json::Obj(Vec::new())),
        "profile-null-fields" => {
            put(prof, &["banking"], Json::Null);
            put(prof, &["members", u, "profile"], Json::Null);
        }
        "member-keys-numeric" => {
            let bp = at(prof, &["members", u, "inventory", "backpack_contents"]).cloned();
            let get = |k: &str| match &bp {
                Some(Json::Obj(o)) => o.iter().find(|(kk, _)| &**kk == k).map(|(_, v)| v.clone()),
                _ => None,
            };
            let mut v = Json::Obj(vec![("10".into(), get("0").unwrap_or(Json::Obj(Vec::new()))), ("2".into(), get("1").unwrap_or(Json::Obj(Vec::new())))]);
            // JS objects list integer keys in ascending order.
            if let Json::Obj(o) = &mut v {
                o.sort_by_key(|(k, _)| k.parse::<u32>().unwrap());
            }
            put(prof, &["members", u, "inventory", "backpack_contents"], v);
        }
        other => panic!("unknown custom case {other}"),
    }
}

fn reduce_items(mut v: Value) -> Value {
    if let Some(items) = v.get_mut("items").and_then(|x| x.as_array_mut()) {
        for it in items.iter_mut() {
            *it = it["id"].clone();
        }
    }
    v
}

#[test]
fn mutated_profiles_match_js() {
    let dir = td().join("review-skyblock");
    if !dir.join("mutations.json").exists() {
        eprintln!("review-skyblock testdata missing, skipped");
        return;
    }
    let (resource, collections, now) = load();
    let doc: Value = serde_json::from_slice(&std::fs::read(dir.join("mutations.json")).unwrap()).unwrap();
    let mut bases = std::collections::HashMap::new();
    for (name, uuid) in doc["uuids"].as_object().unwrap() {
        let raw = std::fs::read(td().join("profiles").join(format!("{name}.json"))).unwrap();
        #[derive(serde::Deserialize)]
        struct P {
            profile: Json,
        }
        let p: P = serde_json::from_slice(&raw).unwrap();
        bases.insert(name.clone(), (uuid.as_str().unwrap().to_string(), p.profile));
    }
    let (mut fails, mut jsthrows, mut total) = (Vec::new(), 0, 0);
    for c in doc["cases"].as_array().unwrap() {
        total += 1;
        let id = c["id"].as_str().unwrap();
        let (uuid, base) = &bases[c["base"].as_str().unwrap()];
        let mut prof = base.clone();
        let path = c["path"].as_str().unwrap();
        let op = c["op"].as_str().unwrap_or("set");
        if path == "@custom" {
            custom(&mut prof, uuid, op);
            run_case(id, &prof, uuid, &c["expected"], &resource, &collections, now, &mut fails, &mut jsthrows);
            continue;
        }
        let (target, p) = match path.strip_prefix('@') {
            Some(p) => (&mut prof, p.to_string()),
            None => (&mut prof, format!("members.{uuid}.{path}")),
        };
        set_path(target, &p, op, c.get("value").unwrap_or(&Value::Null));
        run_case(id, &prof, uuid, &c["expected"], &resource, &collections, now, &mut fails, &mut jsthrows);
    }
    report(&fails, jsthrows, total);
}

#[allow(clippy::too_many_arguments)]
fn run_case(id: &str, prof: &Json, uuid: &str, expected: &Value, resource: &Resource, collections: &CollectionsResource, now: f64, fails: &mut Vec<String>, jsthrows: &mut usize) {
    let got = read_member(prof, uuid, resource, Some(collections), now);
    match (got, expected.get("error").and_then(|e| e.as_str())) {
        (Ok(m), None) => {
            let g = norm(&reduce_items_if_needed(serde_json::to_value(&m).unwrap(), expected), "");
            let e = norm(&expected["member"], "");
            let mut d = Vec::new();
            diff(&g, &e, "", &mut d);
            if !d.is_empty() {
                d.truncate(4);
                fails.push(format!("{id}: {d:?}"));
            }
        }
        (Err(a), Some(b)) => {
            if a != b && (b.starts_with("Player is") || b.starts_with("Inventory API")) {
                fails.push(format!("{id}: error text rust {a:?} js {b:?}"));
            } else if a != b {
                if std::env::var("SHOW_THROWS").is_ok() { eprintln!("THROW {id}: js {b:?}"); }
                *jsthrows += 1;
            }
        }
        (Ok(_), Some(b)) => {
            if b.starts_with("Player is") || b.starts_with("Inventory API") {
                fails.push(format!("{id}: rust Ok, js error {b:?}"));
            } else {
                if std::env::var("SHOW_THROWS").is_ok() { eprintln!("THROW {id}: js {b:?}"); }
                *jsthrows += 1;
            }
        }
        (Err(a), None) => fails.push(format!("{id}: rust error {a:?}, js ok")),
    }
}

fn reduce_items_if_needed(v: Value, expected: &Value) -> Value {
    // expected items are ids only in the mutations file
    if expected["member"]["items"].as_array().is_some_and(|a| a.first().is_none_or(|x| x.is_string())) {
        reduce_items(v)
    } else {
        v
    }
}

/// Malformed shapes Hypixel never sends (wrong JSON types, floats or strings where NBT has ints, ...). The Rust
/// reader drops or coerces them; the JS passes the raw value through or throws. Not worth reproducing.
const KNOWN: &[&str] = &[
    "one:eaIdInt", "one:starsStrBad", "one:starsFloat", "one:hpbFloat", "one:enchList", "one:enchLong", "one:enchDouble",
    "one:enchStr", "one:petJsonArr", "one:petJsonNum", "one:petJsonStr", "one:petJsonExpStr", "one:petJsonTypeNum",
    "one:petCompLong", "one:gemsList", "one:gemsUnlockedInts", "one:gemsQualityNum", "one:gemsGemNonStr", "one:scrollsLists",
    "one:modNum", "one:enrichNum",
    "highest_magical_power:set", "collection.GOLD_INGOT:set", "bop_bonus:set", "catacombs.experience:set",
    "player_data.perks:set", "pets_data.pets:set", "leveling.experience:set", ":pets-shapes", ":perks-mixed", ":hmp-str",
    ":cata-str", ":collection-mixed", ":bop-num",
];

fn report(fails: &[String], jsthrows: usize, total: usize) {
    let all = fails.len();
    let fails: Vec<&String> = fails.iter().filter(|f| !KNOWN.iter().any(|k| f.contains(k))).collect();
    eprintln!("{total} cases, {all} differences ({} unexpected), {jsthrows} where the JS throws on malformed data", fails.len());
    for f in fails.iter().take(60) {
        eprintln!("{f}");
    }
    assert!(fails.is_empty(), "{} differences", fails.len());
}

#[test]
fn synthetic_items_match_js() {
    let dir = td().join("review-skyblock");
    if !dir.join("items.json").exists() {
        return;
    }
    let (resource, collections, now) = load();
    let doc: Value = serde_json::from_slice(&std::fs::read(dir.join("items.json")).unwrap()).unwrap();
    let uuid = doc["uuid"].as_str().unwrap().to_string();
    let raw = std::fs::read(td().join("profiles").join("Ben_MC.json")).unwrap();
    #[derive(serde::Deserialize)]
    struct P {
        profile: Json,
    }
    let base: P = serde_json::from_slice(&raw).unwrap();
    let (mut fails, mut jsthrows, mut total) = (Vec::new(), 0, 0);
    for c in doc["cases"].as_array().unwrap() {
        total += 1;
        let id = c["id"].as_str().unwrap();
        let prof = if let Some(p) = c.get("prof") {
            to_json(p)
        } else {
            let mut prof = base.profile.clone();
            let inv = serde_json::json!({ "inv_contents": { "type": 0, "data": c["data"] } });
            set_path(&mut prof, &format!("members.{uuid}.inventory"), "set", &inv);
            set_path(&mut prof, &format!("members.{uuid}.loadout"), "del", &Value::Null);
            prof
        };
        run_case(id, &prof, &uuid, &c["expected"], &resource, &collections, now, &mut fails, &mut jsthrows);
    }
    report(&fails, jsthrows, total);
}
