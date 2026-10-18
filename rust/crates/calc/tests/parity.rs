//! Parity of `evaluate` with the JS reference on the oracle's dumped loadouts (run with `--release`):
//! every testdata/ctx/<case>.json with evals/<case>.jsonl, and every states/*.jsonl `current`.
//! Prints the maximum relative difference.

mod common;
use common::*;
use dps_calc::{evaluate, Loadout, Model, SerLoadout};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Line {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    label: String,
    loadout: SerLoadout,
    result: Value,
}

#[derive(Deserialize)]
struct StateLine {
    loadout: SerLoadout,
    #[serde(default)]
    current: Option<Value>,
}

#[test]
fn evals_match_js() {
    let Some(resource) = resource() else {
        eprintln!("skipping: testdata/snapshot.json missing");
        return;
    };
    let mut total = Diff::default();
    let mut lines = 0usize;
    let mut cases = 0usize;
    for case in case_names() {
        let evals = testdata().join("evals").join(format!("{case}.jsonl"));
        if !evals.exists() {
            continue;
        }
        let ctx_json: Value = serde_json::from_str(&std::fs::read_to_string(testdata().join("ctx").join(format!("{case}.json"))).unwrap()).unwrap();
        let (ctx, weights) = ctx_from_json(&ctx_json, resource.clone());
        let mut model = Model::new(&ctx);
        let mut diff = Diff::default();
        read_lines(&evals, |n, text| {
            let line: Line = serde_json::from_str(text).unwrap_or_else(|e| panic!("{case} line {n}: {e}"));
            let l = Loadout::from_serialized(&mut model, &line.loadout, ctx.pet_profile.bank).unwrap();
            let r = evaluate(&model, &ctx, &l, &weights);
            diff.eval(&format!("{case}#{n} {} {}", line.kind, line.label), &r, &line.result);
            lines += 1;
        });
        cases += 1;
        if !diff.mismatches.is_empty() {
            eprintln!("{case}: {} mismatches, worst {}", diff.mismatches.len(), diff.worst);
            for m in diff.mismatches.iter().take(5) {
                eprintln!("  {m}");
            }
        }
        if diff.max_rel > total.max_rel {
            total.max_rel = diff.max_rel;
            total.worst = diff.worst.clone();
        }
        total.compared += diff.compared;
        total.mismatches.extend(diff.mismatches.into_iter().take(5));
    }
    eprintln!("evals: {cases} cases, {lines} lines, {} numbers compared, max rel diff {:e} ({})", total.compared, total.max_rel, total.worst);
    assert!(total.mismatches.is_empty(), "mismatches: {:#?}", total.mismatches);
}

#[test]
fn states_current_match_js() {
    let Some(resource) = resource() else {
        eprintln!("skipping: testdata/snapshot.json missing");
        return;
    };
    let dir = testdata().join("states");
    let Ok(rd) = std::fs::read_dir(&dir) else { return };
    let mut total = Diff::default();
    let mut lines = 0usize;
    for e in rd.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(case) = name.strip_suffix(".jsonl") else { continue };
        let ctx_path = {
            let p = dir.join(format!("{case}.ctx.json"));
            if p.exists() { p } else { testdata().join("ctx").join(format!("{case}.json")) }
        };
        let Ok(text) = std::fs::read_to_string(&ctx_path) else { continue };
        let ctx_json: Value = serde_json::from_str(&text).unwrap();
        let (ctx, weights) = ctx_from_json(&ctx_json, resource.clone());
        let mut model = Model::new(&ctx);
        let mut diff = Diff::default();
        read_lines(&e.path(), |n, text| {
            let Ok(line) = serde_json::from_str::<StateLine>(text) else {
                eprintln!("{case} line {n}: unparsable (skipped)");
                return;
            };
            let Some(cur) = line.current else { return };
            let l = Loadout::from_serialized(&mut model, &line.loadout, ctx.pet_profile.bank).unwrap();
            let r = evaluate(&model, &ctx, &l, &weights);
            diff.eval(&format!("states {case}#{n}"), &r, &cur);
            lines += 1;
        });
        if !diff.mismatches.is_empty() {
            eprintln!("{case}: {} mismatches, worst {}", diff.mismatches.len(), diff.worst);
            for m in diff.mismatches.iter().take(5) {
                eprintln!("  {m}");
            }
        }
        if diff.max_rel > total.max_rel {
            total.max_rel = diff.max_rel;
            total.worst = diff.worst.clone();
        }
        total.compared += diff.compared;
        total.mismatches.extend(diff.mismatches.into_iter().take(5));
    }
    eprintln!("states: {lines} lines, {} numbers compared, max rel diff {:e} ({})", total.compared, total.max_rel, total.worst);
    assert!(total.mismatches.is_empty(), "mismatches: {:#?}", total.mismatches);
}

/// from_serialized -> to_serialized gives back the same loadout (sets compared sorted, everything else in order).
#[test]
fn loadout_round_trip() {
    let Some(resource) = resource() else {
        eprintln!("skipping: testdata/snapshot.json missing");
        return;
    };
    let mut lines = 0;
    for case in case_names() {
        let evals = testdata().join("evals").join(format!("{case}.jsonl"));
        if !evals.exists() {
            continue;
        }
        let ctx_json: Value = serde_json::from_str(&std::fs::read_to_string(testdata().join("ctx").join(format!("{case}.json"))).unwrap()).unwrap();
        let (ctx, _) = ctx_from_json(&ctx_json, resource.clone());
        let mut model = Model::new(&ctx);
        read_lines(&evals, |n, text| {
            let line: Line = serde_json::from_str(text).unwrap();
            let l = Loadout::from_serialized(&mut model, &line.loadout, ctx.pet_profile.bank).unwrap();
            let mut got = l.to_serialized(&model);
            let mut want = line.loadout.clone();
            // The JS bank is null only before it was set; the Rust loadout always holds a number.
            if want.bank.is_none() {
                want.bank = Some(ctx.pet_profile.bank.unwrap_or(0.0));
            }
            for s in [&mut got, &mut want] {
                s.owned.sort();
                s.extras.sort();
                s.unlocked_powers.sort();
            }
            let (g, w) = (serde_json::to_string(&got).unwrap(), serde_json::to_string(&want).unwrap());
            assert!(g == w, "{case} line {n}: round trip differs");
            lines += 1;
        });
    }
    eprintln!("round trip: {lines} loadouts");
}

/// A context built from the member and job (as runDps does) behaves like the dumped one.
#[test]
fn ctx_from_member_matches_dump() {
    let Some(resource) = resource() else {
        eprintln!("skipping: testdata/snapshot.json missing");
        return;
    };
    let cases: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(testdata().join("cases.json")).unwrap()).unwrap();
    let mut n = 0;
    for case in case_names() {
        let Some(c) = cases.iter().find(|c| c["id"] == case.as_str()) else { continue };
        let job: dps_core::model::DpsJob = serde_json::from_value(c["job"].clone()).unwrap();
        let profile = c["profile"].as_str().unwrap();
        let member: dps_core::model::Member =
            serde_json::from_str(&std::fs::read_to_string(testdata().join("members").join(format!("{profile}.json"))).unwrap()).unwrap();
        let ctx_json: Value = serde_json::from_str(&std::fs::read_to_string(testdata().join("ctx").join(format!("{case}.json"))).unwrap()).unwrap();
        let (dump, _) = ctx_from_json(&ctx_json, resource.clone());
        let built = dps_calc::Ctx::new(dps_calc::CtxInit::from_member(&member, &job, resource.clone()).unwrap());
        assert_eq!(built.cls, dump.cls, "{case}");
        assert_eq!(built.cata_level, dump.cata_level, "{case}");
        assert_eq!(built.secrets, dump.secrets);
        assert_eq!(built.power, dump.power);
        assert_eq!(built.profile_speed, dump.profile_speed);
        assert_eq!(built.skills, dump.skills, "{case}");
        assert_eq!(built.slayers, dump.slayers, "{case}");
        assert_eq!(built.mode, dump.mode);
        assert_eq!(built.floor_mode, dump.floor_mode);
        assert_eq!(built.blessings, dump.blessings, "{case}");
        assert_eq!(built.any_chest, dump.any_chest);
        assert_eq!(built.d.item_key, dump.d.item_key, "{case}");
        assert_eq!(built.d.prefix, dump.d.prefix, "{case}");
        assert_eq!(built.d.class_additive_damage, dump.d.class_additive_damage);
        assert_eq!(built.profile_stats.as_ref().map(|p| p.sb_level), dump.profile_stats.as_ref().map(|p| p.sb_level));
        n += 1;
    }
    eprintln!("ctx from member: {n} cases");
}

#[test]
fn types_are_thread_safe() {
    fn ok<T: Send + Sync>() {}
    ok::<Model>();
    ok::<dps_calc::Ctx>();
    ok::<Loadout>();
    ok::<dps_calc::Evaluator>();
    ok::<dps_calc::EvalMemo>();
    ok::<dps_calc::EvalResult>();
}

/// `EvalResult::to_damage` is what runDps shows as `startDamage`.
#[test]
fn start_damage_matches_golden() {
    let Some(resource) = resource() else {
        eprintln!("skipping: testdata/snapshot.json missing");
        return;
    };
    let mut n = 0;
    for case in case_names() {
        let golden = testdata().join("golden").join(format!("{case}.json"));
        let evals = testdata().join("evals").join(format!("{case}.jsonl"));
        if !golden.exists() || !evals.exists() {
            continue;
        }
        let g: Value = serde_json::from_str(&std::fs::read_to_string(&golden).unwrap()).unwrap();
        let ctx_json: Value = serde_json::from_str(&std::fs::read_to_string(testdata().join("ctx").join(format!("{case}.json"))).unwrap()).unwrap();
        let (ctx, weights) = ctx_from_json(&ctx_json, resource.clone());
        let mut model = Model::new(&ctx);
        read_lines(&evals, |i, text| {
            if i != 0 {
                return;
            }
            let line: Line = serde_json::from_str(text).unwrap();
            let l = Loadout::from_serialized(&mut model, &line.loadout, ctx.pet_profile.bank).unwrap();
            let got = serde_json::to_value(evaluate(&model, &ctx, &l, &weights).to_damage()).unwrap();
            assert!(json_eq(&got, &g["plan"]["startDamage"]), "{case}: {got} vs {}", g["plan"]["startDamage"]);
            n += 1;
        });
    }
    eprintln!("startDamage: {n} cases");
    assert!(n > 0);
}

/// JSON equality where 689 and 689.0 are the same number.
fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| json_eq(v, w))),
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(v, w)| json_eq(v, w)),
        _ => a == b,
    }
}
