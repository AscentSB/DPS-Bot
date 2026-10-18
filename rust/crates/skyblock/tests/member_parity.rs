//! read_member on every testdata profile equals the JS output (members/<name>.json), compared as JSON values.

use dps_core::model::{CollectionsResource, Resource};
use dps_skyblock::member::{read_member, Json};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Instant;

fn td() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

/// Numbers compare numerically (JS ints vs Rust floats), null equals missing, and `pet` is reduced to the four
/// fields the model keeps.
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
        Value::Array(a) => Value::Array(a.iter().enumerate().map(|(_, x)| norm(x, &format!("{path}[]"))).collect()),
        Value::Number(n) => serde_json::json!(n.as_f64().unwrap()),
        o => o.clone(),
    }
}

fn close(a: &Value, b: &Value, path: &str, errs: &mut Vec<String>) {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            if (x - y).abs() > 1e-9 * x.abs().max(y.abs()).max(1e-300) && x != y {
                errs.push(format!("{path}: {x} != {y}"));
            }
        }
        (Value::Object(x), Value::Object(y)) => {
            for k in x.keys().chain(y.keys()) {
                match (x.get(k), y.get(k)) {
                    (Some(p), Some(q)) => close(p, q, &format!("{path}.{k}"), errs),
                    _ => errs.push(format!("{path}.{k}: key missing on one side")),
                }
            }
            let kx: Vec<_> = x.keys().collect();
            let ky: Vec<_> = y.keys().collect();
            if kx.len() == ky.len() && kx != ky && !path.is_empty() && !x.is_empty() {
                // key order only matters where the JS iterates; report it softly
                let _ = (kx, ky);
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                errs.push(format!("{path}: len {} != {}", x.len(), y.len()));
            }
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                close(p, q, &format!("{path}[{i}]"), errs);
            }
        }
        _ => {
            if a != b {
                errs.push(format!("{path}: {a} != {b}"));
            }
        }
    }
}

/// Same key order for the maps the planner iterates.
fn order(a: &Value, b: &Value, path: &str, errs: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let kx: Vec<_> = x.keys().collect();
            let ky: Vec<_> = y.keys().collect();
            if kx != ky {
                errs.push(format!("{path}: key order {kx:?} != {ky:?}"));
            }
            for (k, p) in x {
                if let Some(q) = y.get(k) {
                    order(p, q, &format!("{path}.{k}"), errs);
                }
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                order(p, q, &format!("{path}[{i}]"), errs);
            }
        }
        _ => {}
    }
}

#[test]
fn read_member_matches_js() {
    let td = td();
    let snap: Value = serde_json::from_slice(&std::fs::read(td.join("snapshot.json")).unwrap()).unwrap();
    let now = snap["now"].as_f64().unwrap();
    let resource: Resource = serde_json::from_value(serde_json::json!(snap["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p[1].clone())
        .collect::<Vec<_>>()))
    .map(|v: Vec<dps_core::model::ItemInfo>| v.into_iter().map(|i| (i.id.clone(), i)).collect())
    .unwrap();
    let collections: CollectionsResource = snap["collections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p[0].as_str().unwrap().to_string(), serde_json::from_value(p[1].clone()).unwrap()))
        .collect();

    let mut names: Vec<_> = std::fs::read_dir(td.join("profiles")).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    assert!(!names.is_empty());
    let mut failures = Vec::new();
    let mut total = std::time::Duration::ZERO;
    for n in &names {
        let raw = std::fs::read(td.join("profiles").join(n)).unwrap();
        #[derive(serde::Deserialize)]
        struct P {
            uuid: String,
            profile: Json,
        }
        let p: P = serde_json::from_slice(&raw).unwrap();
        let expect: Value = serde_json::from_slice(&std::fs::read(td.join("members").join(n)).unwrap()).unwrap();
        let t = Instant::now();
        let got = read_member(&p.profile, &p.uuid, &resource, Some(&collections), now);
        total += t.elapsed();
        let mut errs = Vec::new();
        match (got, expect.get("error")) {
            (Ok(m), None) => {
                let got = norm(&serde_json::to_value(&m).unwrap(), "");
                let exp = norm(&expect, "");
                close(&got, &exp, "", &mut errs);
                // key order of the maps the planner iterates
                for k in ["skills", "slayers", "shards", "perks", "tuning", "gems"] {
                    if let (Some(a), Some(b)) = (got.get(k), exp.get(k)) {
                        order(a, b, k, &mut errs);
                    }
                }
                for (i, (a, b)) in got["items"].as_array().unwrap().iter().zip(exp["items"].as_array().unwrap()).enumerate() {
                    order(a, b, &format!("items[{i}]"), &mut errs);
                }
            }
            (Err(e), Some(x)) => assert_eq!(Some(e.as_str()), x.as_str(), "{n}"),
            (g, _) => errs.push(format!("ok/err mismatch: {:?}", g.map(|_| ()))),
        }
        if !errs.is_empty() {
            errs.truncate(8);
            failures.push(format!("{n}: {errs:#?}"));
        }
    }
    eprintln!("read_member: {} profiles in {:?} ({:?} each)", names.len(), total, total / names.len() as u32);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
