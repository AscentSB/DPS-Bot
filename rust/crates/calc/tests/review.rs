//! Adversarial review vectors (oracle/review-members.mjs): mutated members, jobs and price tables, compared with the
//! JS runDps output. `cargo test --release --test review -- --nocapture`; `CASE=substring` filters.

mod common;
use common::*;
use dps_calc::run_dps;
use dps_core::model::{DpsJob, Member, PriceTables};
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn prices(kind: &str) -> Arc<PriceTables> {
    let text = std::fs::read_to_string(testdata().join("snapshot.json")).unwrap();
    let v: Value = serde_json::from_str(&text).unwrap();
    let mut t = PriceTables::default();
    if kind == "empty" {
        return Arc::new(t);
    }
    for pair in v["bazaar"].as_array().unwrap() {
        t.bazaar.insert(pair[0].as_str().unwrap().to_string(), pair[1].as_f64().unwrap());
    }
    for (i, pair) in v["auctions"].as_array().unwrap().iter().enumerate() {
        if kind == "half" && i % 2 != 0 {
            continue;
        }
        t.auctions.insert(pair[0].as_str().unwrap().to_string(), pair[1].as_f64().unwrap());
    }
    Arc::new(t)
}

fn value_diff(path: &str, got: &Value, want: &Value, out: &mut Vec<String>) {
    if out.len() >= 10 {
        return;
    }
    match (got, want) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            if rel(x, y) > 1e-9 && !(x.abs() < 1e-12 && y.abs() < 1e-12) {
                out.push(format!("{path}: got {x:e} want {y:e}"));
            }
        }
        (Value::Object(x), Value::Object(y)) => {
            for (k, v) in x {
                match y.get(k) {
                    Some(w) => value_diff(&format!("{path}.{k}"), v, w, out),
                    None if v.is_null() => {}
                    None => out.push(format!("{path}.{k}: only in got ({v})")),
                }
            }
            for (k, w) in y {
                if !x.contains_key(k) && !w.is_null() {
                    out.push(format!("{path}.{k}: only in golden"));
                }
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                out.push(format!("{path}: length {} vs {}", x.len(), y.len()));
            }
            for (i, (v, w)) in x.iter().zip(y).enumerate() {
                value_diff(&format!("{path}[{i}]"), v, w, out);
            }
        }
        _ => {
            if got != want {
                out.push(format!("{path}: got {got} want {want}"));
            }
        }
    }
}

#[test]
fn review_vectors() {
    let Some(resource) = resource() else { return };
    let dir = testdata().join("review-plan");
    let Ok(rd) = std::fs::read_dir(&dir) else { return };
    let filter = std::env::var("CASE").ok();
    let mut files: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    files.sort();
    let cancel = AtomicBool::new(false);
    let (mut pass, mut total) = (0, 0);
    let mut failures = Vec::new();
    for f in files {
        let Ok(v) = serde_json::from_str::<Value>(&std::fs::read_to_string(&f).unwrap_or_default()) else { continue };
        let id = v["id"].as_str().unwrap().to_string();
        if filter.as_ref().is_some_and(|x| !id.contains(x.as_str())) {
            continue;
        }
        total += 1;
        let job: DpsJob = serde_json::from_value(v["job"].clone()).unwrap();
        let member: Member = serde_json::from_value(v["member"].clone()).unwrap();
        let pr = prices(v["prices"].as_str().unwrap());
        let got = run_dps(&member, &job, &resource, &pr, &cancel);
        let mut d = Vec::new();
        match (&got, v.get("result").filter(|r| !r.is_null())) {
            (Err(e), None) => eprintln!("both errored {id}: rust {e}, js {}", v["error"]),
            (Err(e), Some(_)) => d.push(format!("rust error {e}, js ok")),
            (Ok(_), None) => d.push(format!("js error {}, rust ok", v["error"])),
            (Ok(r), Some(want)) => value_diff("", &serde_json::to_value(r).unwrap(), want, &mut d),
        }
        if d.is_empty() {
            pass += 1;
        } else {
            eprintln!("FAIL {id}: {}", d.join("; "));
            failures.push(id);
        }
    }
    eprintln!("review passed {pass}/{total}");
    assert!(failures.is_empty(), "{} differ: {failures:?}", failures.len());
}
