//! Shared helpers for the parity, benchmark and planner-port tests: load the oracle snapshot, build contexts from the
//! dumped ctx JSON, stream eval lines.
#![allow(dead_code)]

use dps_calc::{Ctx, CtxInit, EvalResult, SerLoadout, UpgradeMode};
use dps_core::kinds::{Class, FloorMode, Mode};
use dps_core::model::{Blessings, Collections, ItemInfo, PetProfile, ProfileStats, Resource, Tuning, Weights};
use serde_json::Value;
use std::io::BufRead;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

pub fn testdata() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata"))
}

pub fn resource() -> Option<Arc<Resource>> {
    static R: OnceLock<Option<Arc<Resource>>> = OnceLock::new();
    R.get_or_init(|| {
        let path = testdata().join("snapshot.json");
        let text = std::fs::read_to_string(&path).ok()?;
        let v: Value = serde_json::from_str(&text).expect("snapshot.json");
        let mut r = Resource::default();
        for pair in v["items"].as_array().unwrap() {
            let id = pair[0].as_str().unwrap().to_string();
            let info: ItemInfo = serde_json::from_value(pair[1].clone()).unwrap_or_else(|e| panic!("{id}: {e}"));
            r.insert(id, info);
        }
        Some(Arc::new(r))
    })
    .clone()
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}

/// A ctx dump (`serializeCtx`) as a context plus the weights it was dumped with.
pub fn ctx_from_json(v: &Value, resource: Arc<Resource>) -> (Ctx, Weights) {
    let cls = Class::from_js(v["cls"].as_str().unwrap()).unwrap();
    let mut init = CtxInit::new(cls, num(&v["cataLevel"]), resource);
    init.secrets = num(&v["secrets"]);
    init.power = v["power"].as_str().map(String::from);
    if !v["tuning"].is_null() {
        init.tuning = Some(serde_json::from_value::<Tuning>(v["tuning"].clone()).unwrap());
    }
    init.profile_speed = num(&v["profileSpeed"]);
    if !v["petProfile"].is_null() {
        init.pet_profile = serde_json::from_value::<PetProfile>(v["petProfile"].clone()).unwrap();
    }
    init.archer_level = num(&v["archerLevel"]);
    init.berserk_level = num(&v["berserkLevel"]);
    init.mage_level = num(&v["mageLevel"]);
    init.combat_level = num(&v["combatLevel"]);
    if let Some(p) = v["player"].as_object() {
        if !p["skills"].is_null() {
            init.skills = Some(serde_json::from_value(p["skills"].clone()).unwrap());
        }
        if !p["slayers"].is_null() {
            init.slayers = Some(serde_json::from_value(p["slayers"].clone()).unwrap());
        }
    }
    if !v["profileStats"].is_null() {
        init.profile_stats = Some(serde_json::from_value::<ProfileStats>(v["profileStats"].clone()).unwrap());
    }
    init.upgrade_mode = UpgradeMode::from_js(v["upgradeMode"].as_str().unwrap_or("all"));
    init.mode = Mode::from_js(v["mode"].as_str().unwrap()).unwrap();
    init.floor_mode = FloorMode::from_js(v["floorMode"].as_str().unwrap()).unwrap();
    let b = &v["blessings"];
    init.blessings = Some(Blessings {
        power: num(&b["power"]["level"]) as i32,
        wisdom: num(&b["wisdom"]["level"]) as i32,
        time: num(&b["time"]["level"]) as i32,
    });
    if !v["collections"].is_null() {
        init.collections = Some(serde_json::from_value::<Collections>(v["collections"].clone()).unwrap());
    }
    init.any_chest = v["anyChest"].as_bool().unwrap_or(false);
    let w = &v["weights"];
    let weights = Weights { wither: num(&w["wither"]), dragon: num(&w["dragon"]) };
    (Ctx::new(init), weights)
}

pub fn case_names() -> Vec<String> {
    let dir = testdata().join("ctx");
    let mut v: Vec<String> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".json")).map(String::from))
            .collect(),
        Err(_) => Vec::new(),
    };
    v.sort();
    v
}

pub struct EvalLine {
    pub kind: String,
    pub label: String,
    pub loadout: SerLoadout,
    pub result: Value,
}

pub fn read_lines(path: &std::path::Path, mut f: impl FnMut(usize, &str)) {
    let file = std::fs::File::open(path).unwrap();
    let mut r = std::io::BufReader::with_capacity(1 << 20, file);
    let mut line = String::new();
    let mut n = 0;
    loop {
        line.clear();
        if r.read_line(&mut line).unwrap() == 0 {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        f(n, &line);
        n += 1;
    }
}

/// Relative difference as the tests define it: 0 when equal, else |a - b| / max(|a|, |b|).
pub fn rel(a: f64, b: f64) -> f64 {
    if a == b || (a.is_nan() && b.is_nan()) {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

#[derive(Default, Debug)]
pub struct Diff {
    pub max_rel: f64,
    pub worst: String,
    pub mismatches: Vec<String>,
    pub compared: usize,
}

impl Diff {
    pub fn num(&mut self, what: &str, got: f64, want: &Value) {
        let want = want.as_f64().unwrap_or_else(|| panic!("{what}: {want}"));
        let r = rel(got, want);
        self.compared += 1;
        if r > self.max_rel {
            self.max_rel = r;
            self.worst = format!("{what}: got {got:e} want {want:e}");
        }
        if r > 1e-9 && self.mismatches.len() < 20 {
            self.mismatches.push(format!("{what}: got {got:e} want {want:e} (rel {r:e})"));
        }
    }

    /// Compare an `EvalResult` with the JS result object R.
    pub fn eval(&mut self, tag: &str, got: &EvalResult, want: &Value) {
        let t = ["wither", "dragon"];
        for (i, name) in t.iter().enumerate() {
            let s = &got.stats[i];
            let w = &want["stats"][name];
            self.num(&format!("{tag} stats.{name}.damage"), s.damage, &w["damage"]);
            self.num(&format!("{tag} stats.{name}.strength"), s.strength, &w["strength"]);
            self.num(&format!("{tag} stats.{name}.critDamage"), s.crit_damage, &w["critDamage"]);
            self.num(&format!("{tag} stats.{name}.intelligence"), s.intelligence, &w["intelligence"]);
            self.num(&format!("{tag} stats.{name}.critChance"), s.crit_chance, &w["critChance"]);
            self.num(&format!("{tag} stats.{name}.additiveDamage"), s.additive_damage, &w["additiveDamage"]);
            self.num(&format!("{tag} stats.{name}.abilityDamage"), s.ability_damage, &w["abilityDamage"]);
            self.num(&format!("{tag} attackSpeeds.{name}"), got.attack_speeds[i], &want["attackSpeeds"][name]);
            self.num(&format!("{tag} volleys.{name}"), got.volleys[i], &want["volleys"][name]);
            self.num(&format!("{tag} {name}"), got.damage[i], &want[name]);
        }
        self.num(&format!("{tag} ap"), got.ap, &want["ap"]);
        self.num(&format!("{tag} speed"), got.speed, &want["speed"]);
        self.num(&format!("{tag} attackSpeed"), got.attack_speed, &want["attackSpeed"]);
        self.num(&format!("{tag} volleysPerSecond"), got.volleys_per_second, &want["volleysPerSecond"]);
        self.num(&format!("{tag} arrowsPerVolley"), got.arrows_per_volley, &want["arrowsPerVolley"]);
        self.num(&format!("{tag} tuning.attackSpeed"), got.tuning.attack_speed, &want["tuning"]["attackSpeed"]);
        self.num(&format!("{tag} tuning.speed"), got.tuning.speed, &want["tuning"]["speed"]);
        self.num(&format!("{tag} tuning.dps"), got.tuning.dps, &want["tuning"]["dps"]);
        let want_extras: Vec<&str> = want["extras"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        let got_extras: Vec<&str> = got.extras.keys().collect();
        self.compared += 1;
        if want_extras != got_extras && self.mismatches.len() < 20 {
            self.mismatches.push(format!("{tag} extras: got {got_extras:?} want {want_extras:?}"));
            self.max_rel = f64::INFINITY;
        }
    }
}
