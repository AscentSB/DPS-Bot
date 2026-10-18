//! Memo soundness: scoring every fuzz loadout of a case through one shared EvalMemo (in two orders) must equal the
//! direct scoring evaluation bit for bit (EvalKey must contain everything evaluate reads).
mod common;
use common::*;
use dps_calc::eval::evaluate_scoring as evaluate;
use dps_calc::{EvalMemo, Evaluator, Loadout, Model, SerLoadout};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Line {
    loadout: SerLoadout,
}

#[test]
fn memo_is_lossless() {
    let Some(resource) = resource() else { return };
    let mut checked = 0;
    for case in case_names() {
        let ctx_json: Value = serde_json::from_str(&std::fs::read_to_string(testdata().join("ctx").join(format!("{case}.json"))).unwrap()).unwrap();
        let (ctx, weights) = ctx_from_json(&ctx_json, resource.clone());
        let mut model = Model::new(&ctx);
        let mut loads: Vec<Loadout> = Vec::new();
        for dir in ["fuzz", "evals"] {
            let ext = if dir == "fuzz" { "jsonl" } else { "jsonl" };
            let path = testdata().join(dir).join(format!("{case}.{ext}"));
            if !path.exists() {
                continue;
            }
            read_lines(&path, |_, text| {
                let line: Line = serde_json::from_str(text).unwrap();
                loads.push(Loadout::from_serialized(&mut model, &line.loadout, ctx.pet_profile.bank).unwrap());
            });
        }
        let memo = EvalMemo::new(&ctx);
        let ev = Evaluator::new(&model, &ctx, weights).with_memo(&memo);
        let order: Vec<usize> = (0..loads.len()).chain((0..loads.len()).rev()).collect();
        for i in order {
            let direct = evaluate(&model, &ctx, &loads[i], &weights);
            let via = ev.evaluate(&loads[i]);
            assert_eq!(direct.damage[0].to_bits(), via.damage[0].to_bits(), "{case}#{i} wither");
            assert_eq!(direct.damage[1].to_bits(), via.damage[1].to_bits(), "{case}#{i} dragon");
            assert_eq!(direct.ap.to_bits(), via.ap.to_bits(), "{case}#{i} ap");
            assert_eq!(direct.extras.keys().collect::<Vec<_>>(), via.extras.keys().collect::<Vec<_>>());
            checked += 1;
        }
    }
    eprintln!("memo: {checked} evaluations");
}
