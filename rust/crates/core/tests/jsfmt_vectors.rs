//! Checks dps_core::jsfmt (and the Accessory Power formula) against Node's output in testdata/jsfmt.json
//! (written by oracle/jsfmt.mjs). Skips when the file is missing.

use dps_core::jsfmt;
use serde_json::Value;

fn num(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap(),
        Value::String(s) => match s.as_str() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            "-0" => -0.0,
            other => panic!("bad number {other}"),
        },
        other => panic!("bad number {other}"),
    }
}

fn same(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
}

#[test]
fn jsfmt_matches_node() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/jsfmt.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        eprintln!("skipping: {path} missing (run oracle/jsfmt.mjs)");
        return;
    };
    let vectors: Vec<Value> = serde_json::from_str(&text).unwrap();
    let mut failures = Vec::new();
    let mut counts = std::collections::BTreeMap::<String, (usize, usize)>::new();
    for v in &vectors {
        let f = v["fn"].as_str().unwrap();
        let args = v["args"].as_array().unwrap();
        let x = num(&args[0]);
        let entry = counts.entry(f.to_string()).or_default();
        entry.0 += 1;
        let ok = match f {
            "String" => {
                let got = jsfmt::js_string(x);
                let ok = got == v["out"].as_str().unwrap();
                if !ok { failures.push(format!("String({x:e}) = {got:?}, want {}", v["out"])); }
                ok
            }
            "toLocaleString" => {
                let got = jsfmt::to_locale_string(x);
                let ok = got == v["out"].as_str().unwrap();
                if !ok { failures.push(format!("toLocaleString({x:e}) = {got:?}, want {}", v["out"])); }
                ok
            }
            "toFixed" => {
                let d = args[1].as_u64().unwrap() as u32;
                let got = jsfmt::to_fixed(x, d);
                let ok = got == v["out"].as_str().unwrap();
                if !ok { failures.push(format!("({x:e}).toFixed({d}) = {got:?}, want {}", v["out"])); }
                ok
            }
            "formatCoins" => {
                let got = jsfmt::format_coins(x);
                let ok = got == v["out"].as_str().unwrap();
                if !ok { failures.push(format!("formatCoins({x:e}) = {got:?}, want {}", v["out"])); }
                ok
            }
            "Math.round" => {
                let got = jsfmt::math_round(x);
                let want = num(&v["out"]);
                let ok = same(got, want);
                if !ok { failures.push(format!("Math.round({x:e}) = {got:e}, want {want:e}")); }
                ok
            }
            "powerMultiplier" => {
                let got = dps_core::formulas::power_multiplier(x);
                let want = num(&v["out"]);
                let ok = same(got, want);
                if !ok { failures.push(format!("powerMultiplier({x:e}) = {got:e}, want {want:e}")); }
                ok
            }
            "Math.log" => {
                let got = dps_core::jsmath::log(x);
                let want = num(&v["out"]);
                let ok = same(got, want);
                if !ok { failures.push(format!("Math.log({x:e}) = {got:e}, want {want:e}")); }
                ok
            }
            "pow" => {
                let y = num(&args[1]);
                let got = dps_core::jsmath::pow(x, y);
                let want = num(&v["out"]);
                let ok = same(got, want);
                if !ok { failures.push(format!("{x:e} ** {y:e} = {got:e}, want {want:e}")); }
                ok
            }
            other => panic!("unknown fn {other}"),
        };
        if !ok {
            entry.1 += 1;
        }
    }
    for (f, (n, bad)) in &counts {
        eprintln!("{f}: {n} vectors, {bad} mismatches");
    }
    for f in failures.iter().take(40) {
        eprintln!("{f}");
    }
    assert!(failures.is_empty(), "{} mismatches", failures.len());
}
