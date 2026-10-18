//! Synthetic auction downloads (oracle/review-prices.mjs) through AuctionTracker vs the JS fetchAuctions + smoothing.

use dps_skyblock::prices::AuctionTracker;
use serde_json::Value;

fn pairs(v: &Value) -> Vec<(String, f64)> {
    v.as_array().unwrap().iter().map(|e| (e[0].as_str().unwrap().to_owned(), e[1].as_f64().unwrap())).collect()
}

fn sorted(m: impl IntoIterator<Item = (String, f64)>) -> Vec<(String, f64)> {
    let mut v: Vec<_> = m.into_iter().collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

#[test]
fn synthetic_downloads_match_js() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/review-skyblock/prices.json");
    let Ok(raw) = std::fs::read(path) else {
        eprintln!("prices.json missing, skipped");
        return;
    };
    let doc: Value = serde_json::from_slice(&raw).unwrap();
    let mut t = AuctionTracker::new();
    for (n, d) in doc["downloads"].as_array().unwrap().iter().enumerate() {
        let outs = d["pages"].as_array().unwrap().iter().map(|p| t.process(&serde_json::to_vec(p).unwrap()).unwrap()).collect();
        let smooth = t.accept(d["lastUpdated"].as_f64().unwrap(), outs);
        let (got_l, exp_l) = (sorted(t.latest_lowest()), pairs(&d["lowest"]));
        let missing: Vec<_> = exp_l.iter().filter(|e| !got_l.contains(e)).take(8).collect();
        let extra: Vec<_> = got_l.iter().filter(|e| !exp_l.contains(e)).take(8).collect();
        assert!(missing.is_empty() && extra.is_empty() && got_l.len() == exp_l.len(), "download {n} lowest: missing {missing:?} extra {extra:?} ({} vs {})", got_l.len(), exp_l.len());
        let (got_s, exp_s) = (sorted(smooth), pairs(&d["smooth"]));
        let missing: Vec<_> = exp_s.iter().filter(|e| !got_s.contains(e)).take(8).collect();
        let extra: Vec<_> = got_s.iter().filter(|e| !exp_s.contains(e)).take(8).collect();
        assert!(missing.is_empty() && extra.is_empty() && got_s.len() == exp_s.len(), "download {n} smooth: missing {missing:?} extra {extra:?}");
    }
}
