//! Contention check: cloning one shared Loadout from many threads (what parallel candidate scoring does).
mod common;
use common::*;
use dps_calc::{Loadout, Model, SerLoadout};
use rayon::prelude::*;
use serde::Deserialize;
use std::hint::black_box;
use std::time::Instant;

#[derive(Deserialize)]
struct Line {
    loadout: SerLoadout,
}

#[test]
fn clone_contention() {
    let Some(resource) = resource() else { return };
    let case = "Rwob__bers-lcm-master-all-coins";
    let evals = testdata().join("evals").join(format!("{case}.jsonl"));
    if !evals.exists() {
        return;
    }
    let ctx_json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(testdata().join("ctx").join(format!("{case}.json"))).unwrap()).unwrap();
    let (ctx, _w) = ctx_from_json(&ctx_json, resource.clone());
    let mut model = Model::new(&ctx);
    let mut first = None;
    read_lines(&evals, |n, text| {
        if n == 0 {
            let line: Line = serde_json::from_str(text).unwrap();
            first = Some(Loadout::from_serialized(&mut model, &line.loadout, ctx.pet_profile.bank).unwrap());
        }
    });
    let l = first.unwrap();
    let n = 4_000_000usize;
    for threads in [1usize, 4, 16] {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        let t = Instant::now();
        pool.install(|| (0..n).into_par_iter().for_each(|_| drop(black_box(l.clone()))));
        let s = t.elapsed().as_secs_f64();
        eprintln!("clone: {threads} threads: {:.1} ns wall per clone, {:.1} ns cpu per clone", s * 1e9 / n as f64, s * 1e9 * threads as f64 / n as f64);
    }
}
