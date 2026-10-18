//! Rough timings of the hot helpers (run with `cargo test --release -p dps-core --test bench -- --ignored --nocapture`).

use dps_core::model::PetProfile;
use std::hint::black_box;
use std::time::Instant;

#[test]
#[ignore]
fn timings() {
    let types: Vec<(&str, &str)> = dps_core::pets::pet_types().iter().flat_map(|t| t.tiers.iter().map(move |d| (t.kind, d.tier))).collect();
    let items = [None, Some("HEPHAESTUS_RELIC"), Some("GOLD_CLAWS"), Some("PET_ITEM_TIER_BOOST")];
    let profile = PetProfile { bank: Some(5e8), gold_collection: None };
    let t = Instant::now();
    let mut calls = 0u64;
    for _ in 0..200 {
        for &(k, tier) in &types {
            for it in items {
                black_box(dps_core::pets::compute_pet_stats(black_box(k), tier, it, None, &profile));
                calls += 1;
            }
        }
    }
    eprintln!("compute_pet_stats: {:.1} ns/call ({calls} calls)", t.elapsed().as_nanos() as f64 / calls as f64);

    let t = Instant::now();
    let mut acc = 0.0;
    for rep in 0..100 {
        for ap in 0..10000 {
            acc += dps_core::formulas::power_multiplier(black_box((ap + rep) as f64));
        }
    }
    black_box(acc);
    eprintln!("power_multiplier: {:.1} ns/call", t.elapsed().as_nanos() as f64 / 1e6);

    let xs: Vec<f64> = (0..100_000).map(|i| (i as f64 * 7919.123457).sin().abs() * 10f64.powi(i % 12 - 3)).collect();
    let mut s = String::with_capacity(64);
    let t = Instant::now();
    for &x in &xs {
        s.clear();
        dps_core::jsfmt::write_to_fixed(&mut s, x, 2);
        black_box(&s);
    }
    eprintln!("toFixed(2): {:.1} ns/call", t.elapsed().as_nanos() as f64 / xs.len() as f64);
    let t = Instant::now();
    for &x in &xs {
        s.clear();
        dps_core::jsfmt::write_js_string(&mut s, x);
        black_box(&s);
    }
    eprintln!("String(x): {:.1} ns/call", t.elapsed().as_nanos() as f64 / xs.len() as f64);
    let t = Instant::now();
    for &x in &xs {
        s.clear();
        dps_core::jsfmt::write_to_locale_string(&mut s, x);
        black_box(&s);
    }
    eprintln!("toLocaleString: {:.1} ns/call", t.elapsed().as_nanos() as f64 / xs.len() as f64);

    let names = ["Fabled", "ancient", "Withered", "giant", "Jerry's"];
    let t = Instant::now();
    let mut hits = 0;
    for _ in 0..200_000 {
        for n in names {
            hits += dps_core::data::get_reforge(black_box(n)).is_some() as u32;
        }
    }
    black_box(hits);
    eprintln!("get_reforge: {:.1} ns/call", t.elapsed().as_nanos() as f64 / 1e6);
}
