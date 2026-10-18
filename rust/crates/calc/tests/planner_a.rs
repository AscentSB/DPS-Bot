//! Planner part A parity with the JS reference (run with `--release`):
//! (1) `build_loadout` equals the `start` line of every evals/<case>.jsonl (and the tuning split equals the ctx's);
//! (2) for every states/<case>.jsonl line, `available_upgrades` gives exactly the JS candidates (same order, all
//!     fields) and applying each candidate gives the JS gains.

mod common;
use common::*;
use dps_calc::edit::Edit;
use dps_calc::{build_loadout, weighted_gain, CandidateGen, Ctx, CtxInit, Evaluator, Loadout, Model, SerLoadout};
use dps_core::kinds::{Target, Class};
use dps_core::model::{DpsJob, Member, PriceTables};
use serde_json::{json, Value};
use std::sync::Arc;

fn num_eq(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= 1e-12 * a.abs().max(b.abs())
}

/// Differences between two JSON values (numbers compared with a tiny tolerance), at most `max` of them.
fn value_diff(path: &str, a: &Value, b: &Value, out: &mut Vec<String>) {
    if out.len() >= 12 {
        return;
    }
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            if !num_eq(x.as_f64().unwrap(), y.as_f64().unwrap()) {
                out.push(format!("{path}: {x} vs {y}"));
            }
        }
        (Value::Object(x), Value::Object(y)) => {
            for (k, v) in x {
                match y.get(k) {
                    Some(w) => value_diff(&format!("{path}.{k}"), v, w, out),
                    None => out.push(format!("{path}.{k}: only in left")),
                }
            }
            for k in y.keys() {
                if !x.contains_key(k) {
                    out.push(format!("{path}.{k}: only in right"));
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
            if a != b {
                out.push(format!("{path}: {a} vs {b}"));
            }
        }
    }
}

fn prices() -> Option<PriceTables> {
    let text = std::fs::read_to_string(testdata().join("snapshot.json")).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let mut t = PriceTables::default();
    for pair in v["bazaar"].as_array()? {
        t.bazaar.insert(pair[0].as_str()?.to_string(), pair[1].as_f64()?);
    }
    for pair in v["auctions"].as_array()? {
        t.auctions.insert(pair[0].as_str()?.to_string(), pair[1].as_f64()?);
    }
    Some(t)
}

fn norm(l: &SerLoadout) -> Value {
    serde_json::to_value(l).unwrap()
}

#[test]
fn build_loadout_matches_js() {
    let Some(resource) = resource() else {
        eprintln!("skipping: testdata/snapshot.json missing");
        return;
    };
    let cases: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(testdata().join("cases.json")).unwrap()).unwrap();
    let mut n = 0;
    let mut failures: Vec<String> = Vec::new();
    for case in case_names() {
        let evals = testdata().join("evals").join(format!("{case}.jsonl"));
        let Some(c) = cases.iter().find(|c| c["id"] == case.as_str()) else { continue };
        if !evals.exists() {
            continue;
        }
        let job: DpsJob = serde_json::from_value(c["job"].clone()).unwrap();
        let profile = c["profile"].as_str().unwrap();
        let member: Member =
            serde_json::from_str(&std::fs::read_to_string(testdata().join("members").join(format!("{profile}.json"))).unwrap()).unwrap();
        let ctx_json: Value = serde_json::from_str(&std::fs::read_to_string(testdata().join("ctx").join(format!("{case}.json"))).unwrap()).unwrap();
        let (dump, dump_weights) = ctx_from_json(&ctx_json, resource.clone());
        let mut ctx = Ctx::new(CtxInit::from_member(&member, &job, resource.clone()).unwrap());
        let target = job.target.as_deref().and_then(Target::from_js);
        let weights = dps_calc::weights_for(ctx.cls, target, ctx.mode, ctx.floor_mode);
        assert_eq!((weights.wither, weights.dragon), (dump_weights.wither, dump_weights.dragon), "{case}: weights");
        let mut model = Model::new(&ctx);
        let l = build_loadout(&mut model, &mut ctx, weights, &member);
        let mut first: Option<Value> = None;
        read_lines(&evals, |i, text| {
            if i == 0 {
                first = Some(serde_json::from_str(text).unwrap());
            }
        });
        let first = first.unwrap();
        assert_eq!(first["kind"], "start");
        let want: SerLoadout = serde_json::from_value(first["loadout"].clone()).unwrap();
        let mut m2 = Model::new(&dump);
        let want_l = Loadout::from_serialized(&mut m2, &want, dump.pet_profile.bank).unwrap();
        let want_v = norm(&want_l.to_serialized(&m2));
        let got_v = norm(&l.to_serialized(&model));
        let mut d = Vec::new();
        value_diff("loadout", &got_v, &want_v, &mut d);
        let got_split = ctx.tuning_split();
        let want_split = dump.tuning_split();
        if got_split != want_split {
            d.push(format!("tuning split: {got_split:?} vs {want_split:?}"));
        }
        if d.is_empty() {
            n += 1;
        } else {
            failures.push(format!("{case}: {}", d.join("; ")));
        }
    }
    eprintln!("build_loadout: {n} cases equal");
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n"));
    assert!(n > 0);
}

fn candidate_json(c: &dps_calc::Candidate) -> Value {
    let mut o = serde_json::Map::new();
    o.insert("label".into(), json!(c.label));
    o.insert("cost".into(), json!(c.cost));
    o.insert("source".into(), json!(c.source));
    if let Some(u) = c.unverified {
        o.insert("unverified".into(), json!(u));
    }
    if let Some(g) = &c.group {
        o.insert("group".into(), json!(g));
    }
    if let Some(g) = &c.stacks_on {
        o.insert("stacksOn".into(), json!(g));
    }
    if let Some(r) = c.repick {
        o.insert("repick".into(), json!(r));
    }
    if let Some(a) = c.armor_slot {
        o.insert("armorSlot".into(), json!(a));
    }
    if let Some(a) = &c.armor_id {
        o.insert("armorId".into(), json!(a));
    }
    if c.bank {
        o.insert("bank".into(), json!(true));
    }
    Value::Object(o)
}

struct StateStats {
    states: usize,
    candidates: usize,
    scored: usize,
    max_gain_rel: f64,
}

/// Compares the candidates and gains of one state with the JS ones. `line` has `candidates`, `scored` and optionally
/// `banned`. Returns the differences.
#[allow(clippy::too_many_arguments)]
fn check_state(
    tag: &str,
    model: &mut Model,
    ctx: &Ctx,
    weights: dps_core::model::Weights,
    gen: &mut CandidateGen,
    prices: &PriceTables,
    l: &Loadout,
    line: &Value,
    stats: &mut StateStats,
) -> Vec<String> {
    let mut errors = Vec::new();
    let cands = gen.available_upgrades(model, ctx, weights, prices, l);
    let want = line["candidates"].as_array().unwrap();
    let got: Vec<Value> = cands.iter().map(candidate_json).collect();
    let mut d = Vec::new();
    if got.len() != want.len() {
        d.push(format!("candidate count {} vs {}", got.len(), want.len()));
        let k = (0..got.len().min(want.len())).find(|&i| got[i]["label"] != want[i]["label"]);
        if let Some(k) = k {
            d.push(format!("first label difference at {k}: {} vs {}", got[k]["label"], want[k]["label"]));
        }
    }
    for (i, (g, w)) in got.iter().zip(want.iter()).enumerate() {
        value_diff(&format!("cand[{i}]"), g, w, &mut d);
        if d.len() > 8 {
            break;
        }
    }
    if !d.is_empty() {
        errors.push(format!("{tag}: {}", d.join("; ")));
        return errors;
    }
    stats.states += 1;
    stats.candidates += cands.len();
    let ev = Evaluator::new(model, ctx, weights);
    let current = ev.evaluate(l);
    let banned: Vec<&str> = line["banned"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
    let scored = line["scored"].as_array().unwrap();
    let mut seen = vec![false; cands.len()];
    for s in scored {
        let idx = s["index"].as_u64().unwrap() as usize;
        let want_gain = s["gain"].as_f64().unwrap();
        seen[idx] = true;
        let c = &cands[idx];
        let mut next = c.edit.apply_scoring(&ev, l);
        let mut gain = weighted_gain(&current, &ev.evaluate(&next), &weights);
        if c.repick == Some(true) && gain > 1e-9 {
            next = ev.select_power(&ev.select_pet(&next));
            gain = weighted_gain(&current, &ev.evaluate(&next), &weights);
        }
        stats.scored += 1;
        let rel = if gain == want_gain { 0.0 } else { (gain - want_gain).abs() / gain.abs().max(want_gain.abs()) };
        if rel > stats.max_gain_rel {
            stats.max_gain_rel = rel;
        }
        if rel > 1e-9 && !(gain.abs() < 1e-12 && want_gain.abs() < 1e-12) {
            errors.push(format!("{tag} cand {idx} {:?}: gain {gain:e} vs {want_gain:e}", c.label));
        }
    }
    for (idx, c) in cands.iter().enumerate() {
        if seen[idx] || !(c.cost > 0.0) || banned.contains(&&*c.label) {
            continue;
        }
        let next = c.edit.apply_scoring(&ev, l);
        let gain = weighted_gain(&current, &ev.evaluate(&next), &weights);
        if gain > 1e-9 || c.armor_slot.is_some() {
            errors.push(format!("{tag} cand {idx} {:?}: not scored by JS but gain {gain:e} armor {:?}", c.label, c.armor_slot));
        }
    }
    errors
}

fn run_states_case(case: &str, resource: &Arc<dps_core::model::Resource>, prices: &PriceTables) -> Result<StateStats, Vec<String>> {
    let dir = testdata().join("states");
    let ctx_json: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{case}.ctx.json"))).unwrap()).unwrap();
    let (ctx, weights) = ctx_from_json(&ctx_json, resource.clone());
    let mut model = Model::new(&ctx);
    let mut gen = CandidateGen::new();
    let mut errors: Vec<String> = Vec::new();
    let mut stats = StateStats { states: 0, candidates: 0, scored: 0, max_gain_rel: 0.0 };
    read_lines(&dir.join(format!("{case}.jsonl")), |n, text| {
        if errors.len() > 30 {
            return;
        }
        let line: Value = serde_json::from_str(text).unwrap();
        let ser: SerLoadout = serde_json::from_value(line["loadout"].clone()).unwrap();
        let l = Loadout::from_serialized(&mut model, &ser, ctx.pet_profile.bank).unwrap();
        errors.extend(check_state(&format!("{case}#{n}"), &mut model, &ctx, weights, &mut gen, prices, &l, &line, &mut stats));
    });
    if errors.is_empty() {
        Ok(stats)
    } else {
        Err(errors)
    }
}

/// Mutated loadouts (oracle fuzz): `FUZZ_FILE=<jsonl> cargo test --release --test planner_a fuzz_file -- --ignored
/// --nocapture`. Each line is `{ case, log, loadout, candidates, scored }` from JS with a fresh context per line.
#[test]
#[ignore]
fn fuzz_file() {
    let Some(resource) = resource() else { return };
    let Ok(path) = std::env::var("FUZZ_FILE") else { return };
    let prices = prices().unwrap();
    let dir = testdata().join("states");
    let mut ctxs: std::collections::HashMap<String, (Ctx, dps_core::model::Weights, Model)> = Default::default();
    let mut stats = StateStats { states: 0, candidates: 0, scored: 0, max_gain_rel: 0.0 };
    let mut errors: Vec<String> = Vec::new();
    let mut lines = 0;
    read_lines(std::path::Path::new(&path), |n, text| {
        let line: Value = serde_json::from_str(text).unwrap();
        let case = line["case"].as_str().unwrap().to_string();
        let entry = ctxs.entry(case.clone()).or_insert_with(|| {
            let ctx_json: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{case}.ctx.json"))).unwrap()).unwrap();
            let (ctx, weights) = ctx_from_json(&ctx_json, resource.clone());
            let model = Model::new(&ctx);
            (ctx, weights, model)
        });
        let (ctx, weights, model) = (&entry.0, entry.1, &mut entry.2);
        let ser: SerLoadout = serde_json::from_value(line["loadout"].clone()).unwrap();
        let l = Loadout::from_serialized(model, &ser, ctx.pet_profile.bank).unwrap();
        let mut gen = CandidateGen::new();
        let errs = check_state(&format!("{case}#{n} {}", line["log"]), model, ctx, weights, &mut gen, &prices, &l, &line, &mut stats);
        errors.extend(errs);
        lines += 1;
    });
    eprintln!("fuzz: {lines} lines, {} states compared, {} candidates, {} scored, max gain rel diff {:e}", stats.states, stats.candidates, stats.scored, stats.max_gain_rel);
    for e in errors.iter().take(40) {
        eprintln!("  {e}");
    }
    assert!(errors.is_empty(), "{} mismatches", errors.len());
}

#[test]
fn candidates_match_js() {
    let Some(resource) = resource() else {
        eprintln!("skipping: testdata/snapshot.json missing");
        return;
    };
    let prices = Arc::new(prices().unwrap());
    let dir = testdata().join("states");
    let Ok(rd) = std::fs::read_dir(&dir) else { return };
    let mut names: Vec<String> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".jsonl")).map(String::from))
        .filter(|n| dir.join(format!("{n}.ctx.json")).exists())
        .collect();
    names.sort();
    if let Ok(only) = std::env::var("CASE") {
        names.retain(|n| n.contains(&only));
    }
    let results: Vec<(String, Result<StateStats, Vec<String>>)> = std::thread::scope(|s| {
        let handles: Vec<_> = names
            .iter()
            .map(|n| {
                let (resource, prices) = (resource.clone(), prices.clone());
                let n = n.clone();
                std::thread::Builder::new().stack_size(64 << 20).spawn_scoped(s, move || {
                    let r = run_states_case(&n, &resource, &prices);
                    (n, r)
                }).unwrap()
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut failed = false;
    let (mut states, mut cands, mut scored) = (0, 0, 0);
    let mut max_rel: f64 = 0.0;
    for (name, r) in &results {
        match r {
            Ok(s) => {
                eprintln!("{name}: {} states, {} candidates, {} scored, max gain rel diff {:e}", s.states, s.candidates, s.scored, s.max_gain_rel);
                states += s.states;
                cands += s.candidates;
                scored += s.scored;
                max_rel = max_rel.max(s.max_gain_rel);
            }
            Err(e) => {
                failed = true;
                eprintln!("{name}: FAILED");
                for m in e.iter().take(10) {
                    eprintln!("  {m}");
                }
            }
        }
    }
    eprintln!("total: {states} states, {cands} candidates, {scored} scored, max gain rel diff {max_rel:e}");
    assert!(!failed, "mismatches (see above)");
    let _ = (Class::Arch, Edit::Extra(0));
}

/// Timing of `available_upgrades` on the recorded states (single thread): `cargo test --release --test planner_a
/// bench_candidates -- --ignored --nocapture`.
#[test]
#[ignore]
fn bench_candidates() {
    let Some(resource) = resource() else { return };
    let prices = prices().unwrap();
    for case in ["recticc__arch-lcm-master-all-coins", "recticc__mage-lcm-master-all-coins", "tolv__mage-lcm-master-all-coins"] {
        let dir = testdata().join("states");
        let ctx_json: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{case}.ctx.json"))).unwrap()).unwrap();
        let (ctx, weights) = ctx_from_json(&ctx_json, resource.clone());
        let mut model = Model::new(&ctx);
        let mut gen = CandidateGen::new();
        let mut loadouts = Vec::new();
        read_lines(&dir.join(format!("{case}.jsonl")), |_, text| {
            let line: Value = serde_json::from_str(text).unwrap();
            let ser: SerLoadout = serde_json::from_value(line["loadout"].clone()).unwrap();
            loadouts.push(Loadout::from_serialized(&mut model, &ser, ctx.pet_profile.bank).unwrap());
        });
        let mut first = 0.0;
        let mut later = Vec::new();
        for (i, l) in loadouts.iter().enumerate() {
            let t = std::time::Instant::now();
            let c = gen.available_upgrades(&mut model, &ctx, weights, &prices, l);
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            std::hint::black_box(&c);
            if i == 0 { first = ms } else { later.push(ms) }
        }
        let names = ["items", "spare", "buys", "clearboots", "shards", "enrichswap", "bop", "bank", "powers", "scrolls", "pets", "petitems", "essence", "extras"];
        let total_calls = loadouts.len() as f64;
        eprintln!("  per call ms: {}", names.iter().zip(gen.section_nanos.iter()).map(|(n, t)| format!("{n} {:.2}", *t as f64 / 1e6 / total_calls)).collect::<Vec<_>>().join(", "));
        later.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let avg = later.iter().sum::<f64>() / later.len() as f64;
        eprintln!("{case}: first {first:.2} ms, later avg {avg:.2} ms, median {:.2} ms, min {:.2} ms ({} states)", later[later.len() / 2], later[0], loadouts.len());
        // And the cost of applying every candidate (what the scorer pays besides evaluate).
        let cands = gen.available_upgrades(&mut model, &ctx, weights, &prices, &loadouts[loadouts.len() / 2]);
        let ev2 = Evaluator::new(&model, &ctx, weights);
        let l = &loadouts[loadouts.len() / 2];
        let t = std::time::Instant::now();
        let mut k = 0usize;
        for c in &cands {
            if c.edit.needs_evaluator() { continue; }
            let n = c.edit.apply_scoring(&ev2, l);
            k += n.blades.len();
        }
        std::hint::black_box(k);
        eprintln!("  apply all {} candidates: {:.3} ms", cands.len(), t.elapsed().as_secs_f64() * 1000.0);
    }
}

/// Timing of `build_loadout`: `cargo test --release --test planner_a bench_build -- --ignored --nocapture`.
#[test]
#[ignore]
fn bench_build() {
    let Some(resource) = resource() else { return };
    let cases: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(testdata().join("cases.json")).unwrap()).unwrap();
    for case in ["recticc__arch-lcm-master-all-coins", "recticc__mage-lcm-master-all-coins", "tolv__mage-lcm-master-all-coins"] {
        let Some(c) = cases.iter().find(|c| c["id"] == case) else { continue };
        let job: DpsJob = serde_json::from_value(c["job"].clone()).unwrap();
        let member: Member =
            serde_json::from_str(&std::fs::read_to_string(testdata().join("members").join(format!("{}.json", c["profile"].as_str().unwrap()))).unwrap()).unwrap();
        let mut times = Vec::new();
        for _ in 0..5 {
            let mut ctx = Ctx::new(CtxInit::from_member(&member, &job, resource.clone()).unwrap());
            let weights = dps_calc::weights_for(ctx.cls, None, ctx.mode, ctx.floor_mode);
            let mut model = Model::new(&ctx);
            let t = std::time::Instant::now();
            let l = build_loadout(&mut model, &mut ctx, weights, &member);
            times.push(t.elapsed().as_secs_f64() * 1000.0);
            std::hint::black_box(l);
        }
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        eprintln!("{case}: build_loadout min {:.2} ms, median {:.2} ms", times[0], times[2]);
    }
}

/// `build_loadout` for many cases: `BUILD_FILE=<jsonl>` with `{ id, loadout, split }` per line (JS `prepareCase`).
#[test]
#[ignore]
fn build_file() {
    let Some(resource) = resource() else { return };
    let Ok(path) = std::env::var("BUILD_FILE") else { return };
    let cases: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(testdata().join("cases.json")).unwrap()).unwrap();
    let mut ok = 0;
    let mut errors = Vec::new();
    read_lines(std::path::Path::new(&path), |_, text| {
        let line: Value = serde_json::from_str(text).unwrap();
        let id = line["id"].as_str().unwrap();
        let c = cases.iter().find(|c| c["id"] == id).unwrap();
        let job: DpsJob = serde_json::from_value(c["job"].clone()).unwrap();
        let member: Member =
            serde_json::from_str(&std::fs::read_to_string(testdata().join("members").join(format!("{}.json", c["profile"].as_str().unwrap()))).unwrap()).unwrap();
        let mut ctx = Ctx::new(CtxInit::from_member(&member, &job, resource.clone()).unwrap());
        let target = job.target.as_deref().and_then(Target::from_js);
        let weights = dps_calc::weights_for(ctx.cls, target, ctx.mode, ctx.floor_mode);
        let mut model = Model::new(&ctx);
        let l = build_loadout(&mut model, &mut ctx, weights, &member);
        let want: SerLoadout = serde_json::from_value(line["loadout"].clone()).unwrap();
        let mut m2 = Model::new(&ctx);
        let want_l = Loadout::from_serialized(&mut m2, &want, ctx.pet_profile.bank).unwrap();
        let mut d = Vec::new();
        value_diff("loadout", &norm(&l.to_serialized(&model)), &norm(&want_l.to_serialized(&m2)), &mut d);
        let want_split: Option<Vec<(dps_core::Stat, f64)>> = line["split"]
            .as_object()
            .map(|o| o.iter().filter_map(|(k, v)| dps_core::Stat::from_js(k).map(|s| (s, v.as_f64().unwrap()))).collect());
        if ctx.tuning_split() != want_split {
            d.push(format!("split {:?} vs {:?}", ctx.tuning_split(), want_split));
        }
        if d.is_empty() {
            ok += 1;
        } else {
            errors.push(format!("{id}: {}", d.join("; ")));
        }
    });
    eprintln!("build_file: {ok} equal, {} different", errors.len());
    for e in errors.iter().take(20) {
        eprintln!("  {e}");
    }
    assert!(errors.is_empty());
}

#[test]
fn planner_types_are_thread_safe() {
    fn ok<T: Send + Sync>() {}
    ok::<Edit>();
    ok::<dps_calc::Candidate>();
    ok::<Vec<dps_calc::Candidate>>();
}
