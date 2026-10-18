//! Differential test on random loadouts (testdata/fuzz, written by oracle/calc-fuzz.sh): rare paths (pet perks, set
//! bonuses, Legion, Chimera, dual wield, dragon gear, perks, shards, powers, banks, medallion) against the JS.

mod common;
use common::*;
use dps_calc::{evaluate, Loadout, Model, SerLoadout};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Line {
    loadout: SerLoadout,
    result: Value,
}

#[test]
fn fuzz_matches_js() {
    let Some(resource) = resource() else {
        eprintln!("skipping: testdata/snapshot.json missing");
        return;
    };
    let dir = testdata().join("fuzz");
    if !dir.exists() {
        eprintln!("skipping: testdata/fuzz missing (run oracle/calc-fuzz.sh)");
        return;
    }
    let mut total = Diff::default();
    let mut lines = 0usize;
    for case in case_names() {
        let path = dir.join(format!("{case}.jsonl"));
        if !path.exists() {
            continue;
        }
        let ctx_json: Value = serde_json::from_str(&std::fs::read_to_string(testdata().join("ctx").join(format!("{case}.json"))).unwrap()).unwrap();
        let (ctx, weights) = ctx_from_json(&ctx_json, resource.clone());
        let mut model = Model::new(&ctx);
        let mut diff = Diff::default();
        read_lines(&path, |n, text| {
            let line: Line = serde_json::from_str(text).unwrap_or_else(|e| panic!("{case} line {n}: {e}"));
            let l = Loadout::from_serialized(&mut model, &line.loadout, ctx.pet_profile.bank).unwrap();
            let r = evaluate(&model, &ctx, &l, &weights);
            diff.eval(&format!("fuzz {case}#{n}"), &r, &line.result);
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
    eprintln!("fuzz: {lines} lines, {} numbers compared, max rel diff {:e} ({})", total.compared, total.max_rel, total.worst);
    assert!(total.mismatches.is_empty(), "mismatches: {:#?}", total.mismatches);
}
