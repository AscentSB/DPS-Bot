//! Parses every file the oracle wrote into rust/testdata and round-trips the parts that have a type in
//! crates/core/src/model.rs (Member, ItemState, Pet, DpsJob, Tuning, Weights, ...): JSON -> model type -> JSON, then
//! compares with the original. Reports, per field path:
//!   - "dropped": the JS has the field, the model type has no field for it (fine when nothing reads it);
//!   - "null-for-missing": the model writes a value (usually null) where the JS has no field (JS undefined);
//!   - "differs": the value changed on the way through (a real mismatch);
//!   - deserialize errors (a real mismatch).
//! Exit code 1 on deserialize errors or "differs".

#[allow(dead_code)]
#[path = "../../../crates/core/src/model.rs"]
mod model;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct Report {
    files: usize,
    bytes: u64,
    // (kind, path pattern) -> (count, example)
    findings: BTreeMap<(String, String), (usize, String)>,
    errors: Vec<String>,
    checked: BTreeMap<&'static str, usize>,
}

impl Report {
    fn note(&mut self, kind: &str, path: &str, example: String) {
        let pattern = normalize(path);
        let e = self.findings.entry((kind.to_string(), pattern)).or_insert((0, example));
        e.0 += 1;
    }
}

/// `$.items[12].enchants.sharpness` -> `$.items[].enchants.*` for map-like objects is not knowable here, so only
/// array indices are folded.
fn normalize(path: &str) -> String {
    let mut out = String::new();
    let mut in_index = false;
    for ch in path.chars() {
        match ch {
            '[' => {
                in_index = true;
                out.push_str("[]");
            }
            ']' => in_index = false,
            _ if in_index => {}
            _ => out.push(ch),
        }
    }
    out
}

fn short(v: &Value) -> String {
    let s = v.to_string();
    if s.len() > 60 {
        format!("{}...", &s[..60])
    } else {
        s
    }
}

fn diff(path: &str, js: &Value, rs: &Value, r: &mut Report) {
    match (js, rs) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in a {
                match b.get(k) {
                    None => r.note("dropped", &format!("{path}.{k}"), short(v)),
                    Some(w) => diff(&format!("{path}.{k}"), v, w, r),
                }
            }
            for (k, w) in b {
                if !a.contains_key(k) {
                    r.note("null-for-missing", &format!("{path}.{k}"), short(w));
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                r.note("differs", path, format!("array length {} vs {}", a.len(), b.len()));
                return;
            }
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                diff(&format!("{path}[{i}]"), x, y, r);
            }
        }
        (Value::Number(x), Value::Number(y)) => {
            let (fx, fy) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            if fx != fy {
                r.note("differs", path, format!("{x} vs {y}"));
            }
        }
        _ => {
            if js != rs {
                r.note("differs", path, format!("{} vs {}", short(js), short(rs)));
            }
        }
    }
}

/// JSON -> T -> JSON, compared with the input.
fn round_trip<T: DeserializeOwned + Serialize>(what: &'static str, path: &str, v: &Value, r: &mut Report) -> Option<T> {
    *r.checked.entry(what).or_default() += 1;
    match T::deserialize(v) {
        Ok(t) => {
            let back = serde_json::to_value(&t).unwrap();
            diff(path, v, &back, r);
            Some(t)
        }
        Err(e) => {
            r.errors.push(format!("{path}: {what}: {e}"));
            None
        }
    }
}

fn parse_only<T: DeserializeOwned>(what: &'static str, path: &str, v: &Value, r: &mut Report) {
    *r.checked.entry(what).or_default() += 1;
    if let Err(e) = T::deserialize(v) {
        r.errors.push(format!("{path}: {what}: {e}"));
    }
}

fn read(path: &Path, r: &mut Report) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(s) => {
            r.files += 1;
            r.bytes += s.len() as u64;
            Some(s)
        }
        Err(e) => {
            r.errors.push(format!("{}: {e}", path.display()));
            None
        }
    }
}

fn parse(path: &Path, text: &str, r: &mut Report) -> Option<Value> {
    match serde_json::from_str(text) {
        Ok(v) => Some(v),
        Err(e) => {
            r.errors.push(format!("{}: JSON parse: {e}", path.display()));
            None
        }
    }
}

fn json_files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == ext)).collect())
        .unwrap_or_default();
    v.sort();
    v
}

#[derive(Deserialize, Serialize)]
struct Case {
    id: String,
    profile: String,
    job: model::DpsJob,
}

/// Items and pets inside a serialized loadout L.
fn loadout_parts(at: &str, l: &Value, r: &mut Report) {
    let item = |p: String, v: &Value, r: &mut Report| {
        if !v.is_null() {
            round_trip::<model::ItemState>("ItemState (loadout)", &p, v, r);
        }
    };
    for k in ["weapon", "rag", "clearBoots", "rcmBlade"] {
        item(format!("{at}.{k}"), &l[k], r);
    }
    for g in ["armor", "equipment"] {
        if let Some(m) = l[g].as_object() {
            for (slot, v) in m {
                item(format!("{at}.{g}.{slot}"), v, r);
            }
        }
    }
    if let Some(d) = l["dragonGear"].as_object() {
        item(format!("{at}.dragonGear.weapon"), &d["weapon"], r);
        for g in ["armor", "equipment"] {
            if let Some(m) = d[g].as_object() {
                for (slot, v) in m {
                    item(format!("{at}.dragonGear.{g}.{slot}"), v, r);
                }
            }
        }
    }
    for k in ["accessories", "blades", "spareBlades"] {
        for (i, v) in l[k].as_array().into_iter().flatten().enumerate() {
            item(format!("{at}.{k}[{i}]"), v, r);
        }
    }
    for (i, v) in l["pets"].as_array().into_iter().flatten().enumerate() {
        round_trip::<model::Pet>("Pet (loadout)", &format!("{at}.pets[{i}]"), v, r);
    }
    for t in ["wither", "dragon"] {
        let v = &l["activePets"][t];
        if !v.is_null() {
            round_trip::<model::Pet>("Pet (loadout)", &format!("{at}.activePets.{t}"), v, r);
        }
    }
    parse_only::<model::Map<i32>>("shards", &format!("{at}.shards"), &l["shards"], r);
    parse_only::<Vec<String>>("owned", &format!("{at}.owned"), &l["owned"], r);
}

fn main() {
    let td = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata").into()));
    let mut r = Report::default();

    // snapshot.json
    let snap = td.join("snapshot.json");
    if let Some(text) = read(&snap, &mut r) {
        if let Some(v) = parse(&snap, &text, &mut r) {
            if !v["now"].is_u64() {
                r.errors.push("snapshot.json: now is not an integer".into());
            }
            parse_only::<Vec<(String, model::ItemInfo)>>("snapshot items (ItemInfo)", "snapshot.items", &v["items"], &mut r);
            parse_only::<Vec<(String, model::CollectionInfo)>>("snapshot collections", "snapshot.collections", &v["collections"], &mut r);
            parse_only::<Vec<(String, f64)>>("snapshot bazaar", "snapshot.bazaar", &v["bazaar"], &mut r);
            parse_only::<Vec<(String, f64)>>("snapshot auctions", "snapshot.auctions", &v["auctions"], &mut r);
        }
    }

    // profiles/*.json: parse only.
    for p in json_files(&td.join("profiles"), "json") {
        if let Some(text) = read(&p, &mut r) {
            if let Some(v) = parse(&p, &text, &mut r) {
                *r.checked.entry("profile").or_default() += 1;
                if !v["uuid"].is_string() || !v["profile"]["members"].is_object() {
                    r.errors.push(format!("{}: not {{ uuid, profile }}", p.display()));
                }
            }
        }
    }

    // members/*.json: Member round trip ({ error } files are counted apart).
    for p in json_files(&td.join("members"), "json") {
        if let Some(text) = read(&p, &mut r) {
            if let Some(v) = parse(&p, &text, &mut r) {
                if v.get("error").is_some() {
                    *r.checked.entry("member error file").or_default() += 1;
                } else {
                    round_trip::<model::Member>("Member", "$member", &v, &mut r);
                }
            }
        }
    }

    // cases.json
    let cases = td.join("cases.json");
    if let Some(text) = read(&cases, &mut r) {
        if let Some(v) = parse(&cases, &text, &mut r) {
            for (i, c) in v.as_array().into_iter().flatten().enumerate() {
                round_trip::<Case>("Case (DpsJob)", &format!("$cases[{i}]"), c, &mut r);
            }
        }
    }

    // ctx/*.json: the parts with a model type.
    for p in json_files(&td.join("ctx"), "json") {
        if let Some(text) = read(&p, &mut r) {
            if let Some(v) = parse(&p, &text, &mut r) {
                round_trip::<model::Tuning>("ctx.tuning", "$ctx.tuning", &v["tuning"], &mut r);
                if v["tuning"].get("split").is_none() || v.get("weights").is_none() || v.get("resource").is_some() {
                    r.errors.push(format!("{}: want tuning.split and weights, no resource", p.display()));
                }
                round_trip::<model::Weights>("ctx.weights", "$ctx.weights", &v["weights"], &mut r);
                round_trip::<model::PetProfile>("ctx.petProfile", "$ctx.petProfile", &v["petProfile"], &mut r);
                round_trip::<Option<model::ProfileStats>>("ctx.profileStats", "$ctx.profileStats", &v["profileStats"], &mut r);
                round_trip::<Option<model::Collections>>("ctx.collections", "$ctx.collections", &v["collections"], &mut r);
            }
        }
    }

    // evals/*.jsonl: every line parses; its loadout's items and pets round-trip.
    let mut lines = 0usize;
    for p in json_files(&td.join("evals"), "jsonl") {
        if let Some(text) = read(&p, &mut r) {
            for (n, line) in text.lines().enumerate() {
                let Some(v) = parse(&p, line, &mut r) else { continue };
                lines += 1;
                for k in ["kind", "label", "loadout", "result"] {
                    if v.get(k).is_none() {
                        r.errors.push(format!("{}:{}: no {k}", p.display(), n + 1));
                    }
                }
                if !v["result"]["wither"].is_number() || !v["result"]["dragon"].is_number() {
                    r.errors.push(format!("{}:{}: result.wither / dragon not numbers", p.display(), n + 1));
                }
                loadout_parts("$L", &v["loadout"], &mut r);
            }
        }
    }

    // ---- part 2 ----

    // golden/*.json: DpsResult round trip (without "ms", which must be a number).
    for p in json_files(&td.join("golden"), "json") {
        if let Some(text) = read(&p, &mut r) {
            if let Some(mut v) = parse(&p, &text, &mut r) {
                let ms = v.as_object_mut().and_then(|o| o.remove("ms"));
                if !ms.as_ref().is_some_and(|m| m.is_number()) {
                    r.errors.push(format!("{}: no numeric ms", p.display()));
                }
                round_trip::<model::DpsResult>("golden DpsResult", "$golden", &v, &mut r);
            }
        }
    }

    // states/*.jsonl: every line has the documented keys; loadout items / pets round-trip; candidates, scored and
    // bundles parse into the shapes below. states/*.ctx.json as ctx/.
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    #[allow(dead_code)]
    struct Candidate {
        label: String,
        cost: f64,
        source: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unverified: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        group: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stacks_on: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        repick: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        armor_slot: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        armor_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bank: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gain: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        changes: Option<Vec<Change>>,
    }
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    #[allow(dead_code)]
    struct Change {
        key: String,
        prev_key: Option<String>,
        kind: String,
    }
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    #[allow(dead_code)]
    struct Scored {
        index: usize,
        gain: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shortlist_only: Option<bool>,
        changes: Vec<Change>,
    }
    let mut state_lines = 0usize;
    for p in json_files(&td.join("states"), "jsonl") {
        if let Some(text) = read(&p, &mut r) {
            for (n, line) in text.lines().enumerate() {
                let Some(v) = parse(&p, line, &mut r) else { continue };
                state_lines += 1;
                for k in ["round", "loadout", "current", "banned", "candidates", "scored", "bundles"] {
                    if v.get(k).is_none() {
                        r.errors.push(format!("{}:{}: no {k}", p.display(), n + 1));
                    }
                }
                if !v["current"]["wither"].is_number() || !v["current"]["dragon"].is_number() {
                    r.errors.push(format!("{}:{}: current.wither / dragon not numbers", p.display(), n + 1));
                }
                loadout_parts("$state.L", &v["loadout"], &mut r);
                parse_only::<Vec<String>>("state banned", "$state.banned", &v["banned"], &mut r);
                round_trip::<Vec<Candidate>>("state candidates", "$state.candidates", &v["candidates"], &mut r);
                round_trip::<Vec<Scored>>("state scored", "$state.scored", &v["scored"], &mut r);
                round_trip::<Vec<Candidate>>("state bundles", "$state.bundles", &v["bundles"], &mut r);
                let count = v["candidates"].as_array().map_or(0, |a| a.len());
                if v["scored"].as_array().into_iter().flatten().any(|s| s["index"].as_u64().is_none_or(|i| i as usize >= count)) {
                    r.errors.push(format!("{}:{}: scored index out of range", p.display(), n + 1));
                }
            }
        }
    }
    for p in json_files(&td.join("states"), "json") {
        if let Some(text) = read(&p, &mut r) {
            if let Some(v) = parse(&p, &text, &mut r) {
                round_trip::<model::Tuning>("states ctx.tuning", "$sctx.tuning", &v["tuning"], &mut r);
                round_trip::<model::Weights>("states ctx.weights", "$sctx.weights", &v["weights"], &mut r);
                if v.get("reforgeCache").is_some() || v.get("resource").is_some() {
                    r.errors.push(format!("{}: has reforgeCache / resource", p.display()));
                }
            }
        }
    }

    // embeds/*.json: { case, profileSource, entries: [{ id, session, embed, components }] }; the session's plan and
    // profile menu round-trip through the model types.
    let mut embed_entries = 0usize;
    for p in json_files(&td.join("embeds"), "json") {
        if let Some(text) = read(&p, &mut r) {
            if let Some(v) = parse(&p, &text, &mut r) {
                for (i, e) in v["entries"].as_array().into_iter().flatten().enumerate() {
                    embed_entries += 1;
                    let s = &e["session"];
                    for (k, plan) in s["coins"].as_object().into_iter().flatten() {
                        round_trip::<model::DpsResult>("embed session coins (DpsResult)", &format!("$embed.session.coins.{k}"), plan, &mut r);
                    }
                    round_trip::<model::ProfileMenu>("embed session profile (ProfileMenu)", "$embed.session.profile", &s["profile"], &mut r);
                    parse_only::<model::Blessings>("embed session blessings", "$embed.session.blessings", &s["blessings"], &mut r);
                    if !e["id"].is_string() || !e["embed"]["title"].is_string() || !e["components"].is_array() || !s["page"].is_u64() {
                        r.errors.push(format!("{}: entry {i}: want id, embed.title, components, session.page", p.display()));
                    }
                }
            }
        }
    }
    let cmd = td.join("command.json");
    if let Some(text) = read(&cmd, &mut r) {
        if let Some(v) = parse(&cmd, &text, &mut r) {
            *r.checked.entry("command.json").or_default() += 1;
            if v["name"] != "dps" || !v["options"].is_array() {
                r.errors.push("command.json: not the /dps command".into());
            }
        }
    }
    let esc = td.join("escape-markdown.json");
    if let Some(text) = read(&esc, &mut r) {
        if let Some(v) = parse(&esc, &text, &mut r) {
            #[derive(Deserialize)]
            #[allow(dead_code)]
            struct Esc {
                #[serde(rename = "in")]
                input: String,
                out: String,
            }
            parse_only::<Vec<Esc>>("escape-markdown vectors", "$escape", &v, &mut r);
        }
    }
    println!("state lines: {state_lines}, embed entries: {embed_entries}");

    println!("files parsed: {} ({:.1} MB), eval lines: {lines}", r.files, r.bytes as f64 / 1e6);
    println!("checked:");
    for (k, n) in &r.checked {
        println!("  {k}: {n}");
    }
    let mut bad = !r.errors.is_empty();
    println!("findings (kind, path, count, example):");
    for ((kind, path), (n, ex)) in &r.findings {
        if kind == "differs" {
            bad = true;
        }
        println!("  {kind:17} {path}  x{n}  e.g. {ex}");
    }
    if !r.errors.is_empty() {
        println!("errors ({}):", r.errors.len());
        for e in r.errors.iter().take(40) {
            println!("  {e}");
        }
    }
    println!("{}", if bad { "RESULT: MISMATCHES" } else { "RESULT: OK" });
    std::process::exit(if bad { 1 } else { 0 });
}
