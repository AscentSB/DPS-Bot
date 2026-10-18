//! evaluate() throughput on the oracle's eval vectors (run with `cargo test --release -p dps-calc --test bench --
//! --nocapture`). JS does about 44 us per evaluate with warm caches.

mod common;
use common::*;
use dps_calc::{evaluate, EvalMemo, Evaluator, Loadout, Model, SerLoadout};
use rayon::prelude::*;
use serde::Deserialize;
use std::time::Instant;

#[derive(Deserialize)]
struct Line {
    loadout: SerLoadout,
}

#[test]
fn evaluate_throughput() {
    let Some(resource) = resource() else {
        eprintln!("skipping: testdata/snapshot.json missing");
        return;
    };
    let mut total_ns_warm = 0f64;
    let mut total_cold_ns = 0f64;
    let mut total_n = 0usize;
    let mut total_memo_hit_ns = 0f64;
    let mut par_ns = 0f64;
    let mut par_n = 0usize;
    for case in case_names() {
        let evals = testdata().join("evals").join(format!("{case}.jsonl"));
        if !evals.exists() {
            continue;
        }
        let ctx_json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(testdata().join("ctx").join(format!("{case}.json"))).unwrap()).unwrap();
        let (ctx, weights) = ctx_from_json(&ctx_json, resource.clone());
        let mut model = Model::new(&ctx);
        let mut loadouts: Vec<Loadout> = Vec::new();
        read_lines(&evals, |_, text| {
            let line: Line = serde_json::from_str(text).unwrap();
            loadouts.push(Loadout::from_serialized(&mut model, &line.loadout, ctx.pet_profile.bank).unwrap());
        });
        let n = loadouts.len();
        // Cold: first pass computes every item's derived stats.
        let t = Instant::now();
        let mut sink = 0.0;
        for l in &loadouts {
            sink += evaluate(&model, &ctx, l, &weights).damage[0];
        }
        let cold = t.elapsed().as_nanos() as f64 / n as f64;
        // Warm.
        let reps = (20000 / n).max(3);
        let t = Instant::now();
        for _ in 0..reps {
            for l in &loadouts {
                sink += evaluate(&model, &ctx, l, &weights).damage[0];
            }
        }
        let warm = t.elapsed().as_nanos() as f64 / (n * reps) as f64;
        // Memo hits.
        let memo = EvalMemo::new(&ctx);
        let ev = Evaluator::new(&model, &ctx, weights).with_memo(&memo);
        for l in &loadouts {
            sink += ev.evaluate(l).damage[0];
        }
        let t = Instant::now();
        for _ in 0..reps {
            for l in &loadouts {
                sink += ev.evaluate(l).damage[0];
            }
        }
        let hit = t.elapsed().as_nanos() as f64 / (n * reps) as f64;
        // Parallel.
        let ev0 = Evaluator::new(&model, &ctx, weights);
        let t = Instant::now();
        let preps = reps * 4;
        for _ in 0..preps {
            let s: f64 = loadouts.par_iter().map(|l| ev0.evaluate(l).damage[0]).sum();
            sink += s;
        }
        let par = t.elapsed().as_nanos() as f64 / (n * preps) as f64;
        eprintln!("{case:60} n={n:4} cold {:8.0} ns  warm {:7.0} ns  memo hit {:6.0} ns  parallel {:6.0} ns/eval", cold, warm, hit, par);
        total_ns_warm += warm * n as f64;
        total_cold_ns += cold * n as f64;
        total_memo_hit_ns += hit * n as f64;
        par_ns += par * n as f64;
        par_n += n;
        total_n += n;
        assert!(sink.is_finite());
    }
    if total_n > 0 {
        eprintln!(
            "average over {total_n} loadouts: cold {:.0} ns, warm {:.0} ns ({:.0} evaluates/s), memo hit {:.0} ns, parallel {:.0} ns/eval on {} threads",
            total_cold_ns / total_n as f64,
            total_ns_warm / total_n as f64,
            1e9 / (total_ns_warm / total_n as f64),
            total_memo_hit_ns / total_n as f64,
            par_ns / par_n as f64,
            rayon::current_num_threads()
        );
    }
}

/// Costs around evaluate in the planner's single-threaded phases: interning items, building candidate loadouts, and
/// evaluating a candidate whose accessory bag is new (every accessory recombobulated once).
#[test]
fn candidate_costs() {
    let Some(resource) = resource() else {
        eprintln!("skipping: testdata/snapshot.json missing");
        return;
    };
    for case in ["Rwob__bers-lcm-master-all-coins", "spang_b__mage-lcm-master-all-coins", "TapsiHx__arch-lcm-master-all-coins"] {
        let evals = testdata().join("evals").join(format!("{case}.jsonl"));
        if !evals.exists() {
            continue;
        }
        let ctx_json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(testdata().join("ctx").join(format!("{case}.json"))).unwrap()).unwrap();
        let (ctx, weights) = ctx_from_json(&ctx_json, resource.clone());
        let mut model = Model::new(&ctx);
        let mut first = String::new();
        read_lines(&evals, |n, text| {
            if n == 0 {
                first = text.to_string();
            }
        });
        let line: Line = serde_json::from_str(&first).unwrap();
        let t = Instant::now();
        let start = Loadout::from_serialized(&mut model, &line.loadout, ctx.pet_profile.bank).unwrap();
        let build_us = t.elapsed().as_micros();
        let accs: Vec<dps_calc::ItemId> = model.bag(start.accessories).items.to_vec();
        // Interning hits.
        let states: Vec<_> = accs.iter().map(|&i| model.item(i).clone()).collect();
        let t = Instant::now();
        for s in &states {
            std::hint::black_box(model.intern_item(s.clone()));
        }
        let hit_ns = t.elapsed().as_nanos() as f64 / states.len() as f64;
        // Interning misses: recombobulate each accessory.
        let t = Instant::now();
        let recombed: Vec<(dps_calc::ItemId, dps_calc::ItemId)> = accs
            .iter()
            .map(|&i| {
                let mut s = model.item(i).clone();
                s.recomb = !s.recomb;
                (i, model.intern_item(s))
            })
            .collect();
        let miss_ns = t.elapsed().as_nanos() as f64 / accs.len() as f64;
        // Candidate loadouts.
        let t = Instant::now();
        let cands: Vec<Loadout> = recombed.iter().map(|&(o, n)| start.replace_item(&mut model, o, n)).collect();
        let replace_ns = t.elapsed().as_nanos() as f64 / cands.len() as f64;
        let t = Instant::now();
        for _ in 0..1000 {
            std::hint::black_box(start.clone());
        }
        let clone_ns = t.elapsed().as_nanos() as f64 / 1000.0;
        // Evaluate the new-bag candidates (first evaluation: bag stats and item stats are computed).
        let t = Instant::now();
        let mut sink = 0.0;
        for l in &cands {
            sink += evaluate(&model, &ctx, l, &weights).damage[0];
        }
        let cold_ns = t.elapsed().as_nanos() as f64 / cands.len() as f64;
        let t = Instant::now();
        for l in &cands {
            sink += evaluate(&model, &ctx, l, &weights).damage[0];
        }
        let warm_ns = t.elapsed().as_nanos() as f64 / cands.len() as f64;
        eprintln!(
            "{case}: from_serialized {build_us} us ({} accessories); intern hit {hit_ns:.0} ns, miss {miss_ns:.0} ns; replace_item {replace_ns:.0} ns; clone {clone_ns:.0} ns; evaluate new-bag candidate cold {cold_ns:.0} ns, warm {warm_ns:.0} ns ({sink:.0})",
            accs.len()
        );
    }
}

/// Scaling over threads on a large batch (shared model, no memo).
#[test]
fn parallel_scaling() {
    let Some(resource) = resource() else {
        eprintln!("skipping: testdata/snapshot.json missing");
        return;
    };
    let case = "Rwob__bers-lcm-master-all-coins";
    let evals = testdata().join("evals").join(format!("{case}.jsonl"));
    if !evals.exists() {
        return;
    }
    let ctx_json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(testdata().join("ctx").join(format!("{case}.json"))).unwrap()).unwrap();
    let (ctx, weights) = ctx_from_json(&ctx_json, resource.clone());
    let mut model = Model::new(&ctx);
    let mut base: Vec<Loadout> = Vec::new();
    read_lines(&evals, |_, text| {
        let line: Line = serde_json::from_str(text).unwrap();
        base.push(Loadout::from_serialized(&mut model, &line.loadout, ctx.pet_profile.bank).unwrap());
    });
    let batch: Vec<Loadout> = base.iter().cycle().take(200_000).cloned().collect();
    let ev = Evaluator::new(&model, &ctx, weights);
    let run = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        // warm
        pool.install(|| batch.par_iter().map(|l| ev.evaluate(l).damage[0]).sum::<f64>());
        let t = Instant::now();
        let s: f64 = pool.install(|| batch.par_iter().map(|l| ev.evaluate(l).damage[0]).sum::<f64>());
        assert!(s.is_finite());
        t.elapsed().as_secs_f64()
    };
    let t1 = run(1);
    let tn = run(rayon::current_num_threads());
    eprintln!(
        "200k evaluates: 1 thread {:.0} ms ({:.0} ns each), {} threads {:.0} ms ({:.0} ns each wall), speedup {:.1}x",
        t1 * 1e3,
        t1 * 1e9 / 200_000.0,
        rayon::current_num_threads(),
        tn * 1e3,
        tn * 1e9 / 200_000.0,
        t1 / tn
    );
}

/// Where the time goes: active_extras alone and evaluate_with given a precomputed extras list.
#[test]
fn phases() {
    let Some(resource) = resource() else {
        return;
    };
    let case = "Rwob__bers-lcm-master-all-coins";
    let evals = testdata().join("evals").join(format!("{case}.jsonl"));
    if !evals.exists() {
        return;
    }
    let ctx_json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(testdata().join("ctx").join(format!("{case}.json"))).unwrap()).unwrap();
    let (ctx, weights) = ctx_from_json(&ctx_json, resource.clone());
    let mut model = Model::new(&ctx);
    let mut ls: Vec<Loadout> = Vec::new();
    read_lines(&evals, |_, text| {
        let line: Line = serde_json::from_str(text).unwrap();
        ls.push(Loadout::from_serialized(&mut model, &line.loadout, ctx.pet_profile.bank).unwrap());
    });
    for l in &ls {
        std::hint::black_box(evaluate(&model, &ctx, l, &weights));
    }
    let reps = 100;
    let t = Instant::now();
    let mut n = 0usize;
    for _ in 0..reps {
        for l in &ls {
            n += dps_calc::active_extras(&model, &ctx, l, Some(&weights)).len();
        }
    }
    let ae = t.elapsed().as_nanos() as f64 / (reps * ls.len()) as f64;
    let ex: Vec<_> = ls.iter().map(|l| dps_calc::active_extras(&model, &ctx, l, Some(&weights))).collect();
    let t = Instant::now();
    let mut s = 0.0;
    for _ in 0..reps {
        for (l, e) in ls.iter().zip(&ex) {
            s += dps_calc::evaluate_with(&model, &ctx, l, e).damage[0];
        }
    }
    let ew = t.elapsed().as_nanos() as f64 / (reps * ls.len()) as f64;
    eprintln!("active_extras {ae:.0} ns, evaluate_with {ew:.0} ns ({n} {s:.0})");
}
