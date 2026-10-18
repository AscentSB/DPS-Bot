//! Planner part B parity (run with `--release`): for every case in testdata/cases.json, `run_dps` equals
//! golden/<caseId>.json (same steps in the same order, numbers within 1e-9 relative). Prints Rust time vs the JS
//! `ms` per case and the total CPU time ratio.
//!
//! `cargo test --release --test plan_parity -- --nocapture`; `CASE=substring` filters.

mod common;
use common::*;
use dps_calc::run_dps;
use dps_core::model::{DpsJob, Member, PriceTables};
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

fn prices() -> Option<Arc<PriceTables>> {
    let text = std::fs::read_to_string(testdata().join("snapshot.json")).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let mut t = PriceTables::default();
    for pair in v["bazaar"].as_array()? {
        t.bazaar.insert(pair[0].as_str()?.to_string(), pair[1].as_f64()?);
    }
    for pair in v["auctions"].as_array()? {
        t.auctions.insert(pair[0].as_str()?.to_string(), pair[1].as_f64()?);
    }
    Some(Arc::new(t))
}

/// Process CPU seconds (user + system) from /proc/self/stat.
fn cpu_seconds() -> f64 {
    let s = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let rest = s.rsplit(')').next().unwrap_or("");
    let f: Vec<&str> = rest.split_whitespace().collect();
    let ticks = |i: usize| f.get(i).and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0);
    (ticks(11) + ticks(12)) / 100.0
}

/// `EXACT=1` compares every number bit for bit instead of within 1e-9 relative.
fn exact() -> bool {
    static E: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *E.get_or_init(|| std::env::var("EXACT").is_ok_and(|v| v == "1"))
}

fn value_diff(path: &str, got: &Value, want: &Value, out: &mut Vec<String>) {
    if out.len() >= 10 {
        return;
    }
    match (got, want) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            if exact() {
                if x.to_bits() != y.to_bits() {
                    out.push(format!("{path}: got {x:e} want {y:e} (bits differ)"));
                }
            } else if rel(x, y) > 1e-9 && !(x.abs() < 1e-12 && y.abs() < 1e-12) {
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
fn run_dps_matches_golden() {
    let Some(resource) = resource() else {
        eprintln!("skipping: testdata/snapshot.json missing");
        return;
    };
    let prices = prices().unwrap();
    let cases: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(testdata().join("cases.json")).unwrap()).unwrap();
    let filter = std::env::var("CASE").ok();
    let cancel = AtomicBool::new(false);
    let (mut pass, mut total) = (0, 0);
    let (mut rust_wall, mut js_ms) = (0.0, 0.0);
    let cpu0 = cpu_seconds();
    let mut failures: Vec<String> = Vec::new();
    for c in &cases {
        let id = c["id"].as_str().unwrap();
        if filter.as_ref().is_some_and(|f| !id.contains(f.as_str())) {
            continue;
        }
        let golden_path = testdata().join("golden").join(format!("{id}.json"));
        let Ok(text) = std::fs::read_to_string(&golden_path) else { continue };
        let golden: Value = serde_json::from_str(&text).unwrap();
        let job: DpsJob = serde_json::from_value(c["job"].clone()).unwrap();
        let profile = c["profile"].as_str().unwrap();
        let member: Member =
            serde_json::from_str(&std::fs::read_to_string(testdata().join("members").join(format!("{profile}.json"))).unwrap()).unwrap();
        let t = Instant::now();
        let result = run_dps(&member, &job, &resource, &prices, &cancel);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        total += 1;
        let js = golden["ms"].as_f64().unwrap_or(0.0);
        rust_wall += ms;
        js_ms += js;
        let mut d = Vec::new();
        match result {
            Err(e) => d.push(format!("error: {e}")),
            Ok(r) => {
                let got = serde_json::to_value(&r).unwrap();
                let mut want = golden.clone();
                want.as_object_mut().unwrap().remove("ms");
                value_diff("", &got, &want, &mut d);
            }
        }
        let status = if d.is_empty() { "ok  " } else { "FAIL" };
        eprintln!("{status} {id}: rust {ms:8.1} ms, js {js:9.1} ms, x{:.1}", js / ms.max(0.001));
        if d.is_empty() {
            pass += 1;
        } else {
            failures.push(format!("{id}: {}", d.join("; ")));
        }
    }
    let cpu = cpu_seconds() - cpu0;
    eprintln!("passed {pass}/{total}; rust wall {:.1} s, rust cpu {:.1} s, js {:.1} s (wall = cpu, one thread); wall ratio {:.1}x, cpu ratio {:.1}x", rust_wall / 1000.0, cpu, js_ms / 1000.0, js_ms / rust_wall.max(1e-9), js_ms / 1000.0 / cpu.max(1e-9));
    for f in failures.iter().take(30) {
        eprintln!("  {f}");
    }
    assert!(failures.is_empty(), "{} of {total} cases differ", failures.len());
}
