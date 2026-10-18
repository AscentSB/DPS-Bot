//! Adversarial differential test (oracle/review-edge.mjs): degenerate contexts and loadouts against the JS.
mod common;
use common::*;
use dps_calc::{evaluate, Evaluator, Loadout, Model, PowerField, SerLoadout};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Line {
    ctx: Value,
    loadout: SerLoadout,
    result: Option<Value>,
    #[serde(default, rename = "nonFinite")]
    non_finite: Option<Vec<String>>,
    threw: Option<String>,
    picks: Option<Value>,
}

#[test]
fn edge_matches_js() {
    let Some(resource) = resource() else { return };
    let path = testdata().join("review-edge.jsonl");
    if !path.exists() {
        eprintln!("skipping: run oracle/review-edge.sh");
        return;
    }
    let mut diff = Diff::default();
    let mut bad: Vec<String> = Vec::new();
    let mut n_lines = 0;
    read_lines(&path, |n, text| {
        let line: Line = serde_json::from_str(text).unwrap_or_else(|e| panic!("line {n}: {e}"));
        n_lines += 1;
        let (ctx, weights) = ctx_from_json(&line.ctx, resource.clone());
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut model = Model::new(&ctx);
            let l = Loadout::from_serialized(&mut model, &line.loadout, ctx.pet_profile.bank).unwrap();
            evaluate(&model, &ctx, &l, &weights)
        }));
        let r = match r {
            Ok(r) => r,
            Err(_) => {
                bad.push(format!("line {n}: Rust panicked (JS {})", if line.threw.is_some() { "threw" } else { "ok" }));
                return;
            }
        };
        if let Some(t) = &line.threw {
            bad.push(format!("line {n}: JS threw '{t}', Rust returned {}/{}", r.damage[0], r.damage[1]));
            return;
        }
        if let Some(nf) = &line.non_finite {
            let ok = r.damage.iter().any(|d| !d.is_finite()) || r.ap.is_nan() || !r.speed.is_finite() || r.stats.iter().any(|s| !s.damage.is_finite() || !s.strength.is_finite() || !s.crit_damage.is_finite() || !s.crit_chance.is_finite() || !s.intelligence.is_finite()) || !r.attack_speed.is_finite() || !r.tuning.dps.is_finite();
            if !ok {
                bad.push(format!("line {n}: JS non-finite {nf:?} but Rust all finite ({}/{})", r.damage[0], r.damage[1]));
            }
            return;
        }
        diff.eval(&format!("edge#{n}"), &r, line.result.as_ref().unwrap());
        if let Some(p) = &line.picks {
            if p.get("threw").is_some() {
                bad.push(format!("line {n}: JS picks threw {}", p["threw"]));
                return;
            }
            let mut model = Model::new(&ctx);
            let l = Loadout::from_serialized(&mut model, &line.loadout, ctx.pet_profile.bank).unwrap();
            let ev = Evaluator::new(&model, &ctx, weights);
            let pk = |id: dps_calc::ids::PetId| {
                let p = model.pet(id);
                format!("{}:{}:{}", p.kind, p.tier, p.held_item.clone().unwrap_or_default())
            };
            let want_strs = |v: &Value| -> Vec<Option<String>> { v.as_array().unwrap().iter().map(|x| x.as_str().map(String::from)).collect() };
            let got: Vec<Option<String>> = ev.dps_pets(&l.pets).iter().map(|&i| Some(pk(i))).collect();
            if got != want_strs(&p["dpsPets"]) {
                bad.push(format!("line {n}: dpsPets got {got:?} want {}", p["dpsPets"]));
            }
            let sp = ev.select_pet(&l);
            let got: Vec<Option<String>> = sp.active_pets.iter().map(|o| o.map(pk)).collect();
            if got != want_strs(&p["selectPet"]) {
                bad.push(format!("line {n}: selectPet got {got:?} want {}", p["selectPet"]));
            }
            let pw = ev.select_power(&l).power;
            let want_pw = match p["selectPower"].as_str() {
                None => PowerField::Unset,
                Some(k) => match dps_calc::ctx::power_index(k) {
                    Some(i) => PowerField::Known(i as u8),
                    None => PowerField::Unknown,
                },
            };
            if pw != want_pw {
                bad.push(format!("line {n}: selectPower got {pw:?} want {}", p["selectPower"]));
            }
            let split = ev.best_tuning_split(&l);
            let ok = match (&split, p["split"].as_object()) {
                (None, None) => true,
                (Some(g), Some(w)) => g.len() == w.len() && g.iter().zip(w.iter()).all(|((gk, gv), (wk, wv))| gk.js_name() == wk && *gv == wv.as_f64().unwrap()),
                _ => false,
            };
            if !ok {
                bad.push(format!("line {n}: split got {split:?} want {}", p["split"]));
            }
            let counted: Vec<String> = model.counted_accessories(l.accessories).iter().map(|&i| model.item(i).id.clone()).collect();
            let want_c: Vec<String> = p["counted"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
            if counted != want_c {
                bad.push(format!("line {n}: counted got {counted:?} want {want_c:?}"));
            }
            if model.total_accessory_power(l.accessories) != p["totalAp"].as_f64().unwrap() {
                bad.push(format!("line {n}: totalAp got {} want {}", model.total_accessory_power(l.accessories), p["totalAp"]));
            }
            let a = model.accessory_stats(&ctx, l.accessories, l.power, 3.0, 0);
            if a.ap != p["acc"]["ap"].as_f64().unwrap() || a.tuning_points != p["acc"]["tp"].as_f64().unwrap() {
                bad.push(format!("line {n}: acc ap/tp got {} {} want {}", a.ap, a.tuning_points, p["acc"]));
            }
            for (k, v) in p["acc"]["stats"].as_object().unwrap() {
                let st = dps_core::stat::Stat::from_js(k).unwrap();
                if a.stats.get(st) != v.as_f64() {
                    bad.push(format!("line {n}: acc stat {k} got {:?} want {v}", a.stats.get(st)));
                }
            }
            if a.stats.iter().count() != p["acc"]["stats"].as_object().unwrap().len() {
                bad.push(format!("line {n}: acc stat key count"));
            }
            if ctx.sa_cloak_multiplier(2.5) != p["sa"].as_f64().unwrap() {
                bad.push(format!("line {n}: sa cloak"));
            }
        }
    });
    eprintln!("edge: {n_lines} lines, {} numbers, max rel {:e} ({}), {} problems", diff.compared, diff.max_rel, diff.worst, bad.len());
    for m in diff.mismatches.iter().take(15) {
        eprintln!("  {m}");
    }
    for m in bad.iter().take(15) {
        eprintln!("  {m}");
    }
    assert!(diff.mismatches.is_empty() && bad.is_empty());
}
