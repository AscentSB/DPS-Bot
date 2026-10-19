//! Port of test/prices.test.js plus parity against the JS bot over saved real auction pages
//! (testdata/auction-pages, produced by oracle/skyblock-prices.mjs) and a decode benchmark (`--ignored`).

use super::*;
use crate::price_nbt::tests::item;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::AtomicI64;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn item_bytes(fill: impl FnOnce(&mut crate::price_nbt::tests::W)) -> String {
    let nbt = item(fill);
    let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
    gz.write_all(&nbt).unwrap();
    STANDARD.encode(gz.finish().unwrap())
}

fn auction(uuid: u32, bid: f64, bytes: &str) -> String {
    format!(r#"{{"uuid":"{:032x}","bin":true,"starting_bid":{bid},"item_bytes":"{bytes}"}}"#, uuid)
}

fn page_json(total: usize, last_updated: u64, auctions: &[String]) -> String {
    format!(r#"{{"success":true,"page":0,"totalPages":{total},"lastUpdated":{last_updated},"auctions":[{}]}}"#, auctions.join(","))
}

struct Mock {
    base: String,
    peak: Arc<AtomicI64>,
    calls: Arc<AtomicI64>,
}

/// A tiny HTTP server: `handler(page)` returns (status, body). Each request takes 5 ms.
async fn mock<F>(handler: F) -> Mock
where
    F: Fn(usize) -> (u16, String) + Send + Sync + 'static,
{
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (peak, calls, active) = (Arc::new(AtomicI64::new(0)), Arc::new(AtomicI64::new(0)), Arc::new(AtomicI64::new(0)));
    let handler = Arc::new(handler);
    let (p, c) = (peak.clone(), calls.clone());
    tokio::spawn(async move {
        loop {
            let (mut sock, _) = listener.accept().await.unwrap();
            let (handler, peak, calls, active) = (handler.clone(), p.clone(), c.clone(), active.clone());
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                loop {
                    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                        let n = sock.read(&mut tmp).await.unwrap_or(0);
                        if n == 0 {
                            return;
                        }
                        buf.extend_from_slice(&tmp[..n]);
                    }
                    let end = buf.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
                    let head = String::from_utf8_lossy(&buf[..end]).into_owned();
                    buf.drain(..end);
                    let path = head.split_whitespace().nth(1).unwrap_or("").to_owned();
                    let page = path.split("page=").nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                    calls.fetch_add(1, Ordering::SeqCst);
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    let (status, body) = handler(page);
                    let resp = format!("HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}", body.len());
                    if sock.write_all(resp.as_bytes()).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    Mock { base: format!("http://{addr}"), peak, calls }
}

fn fast_fetcher(base: &str) -> Fetcher {
    let mut f = Fetcher::with_api(base);
    f.backoff_ms = 20;
    f
}

fn terminator() -> String {
    item_bytes(|w| {
        w.string("id", "TERMINATOR");
    })
}

#[tokio::test]
async fn auction_pages_download_a_few_at_a_time_retry_and_keep_the_lowest_bin() {
    let tb = terminator();
    let failures = Arc::new(AtomicI64::new(2));
    let f2 = failures.clone();
    let m = mock(move |page| {
        if page == 7 && f2.fetch_sub(1, Ordering::SeqCst) > 0 {
            return (500, "down".into());
        }
        (200, page_json(20, 1, &[auction(page as u32, 1000.0 + page as f64, &tb)]))
    })
    .await;
    let mut t = AuctionTracker::new();
    let prices = t.refresh(&fast_fetcher(&m.base)).await.unwrap().unwrap();
    assert_eq!(prices["TERMINATOR"], 1000.0);
    assert!(m.peak.load(Ordering::SeqCst) <= 6, "peak {}", m.peak.load(Ordering::SeqCst));
    assert_eq!(m.calls.load(Ordering::SeqCst), 22);
}

#[tokio::test]
async fn a_page_that_keeps_failing_fails_the_whole_refresh() {
    let tb = terminator();
    let m = mock(move |page| if page == 3 { (500, "down".into()) } else { (200, page_json(5, 1, &[auction(page as u32, 1000.0, &tb)])) }).await;
    let mut t = AuctionTracker::new();
    let err = t.refresh(&fast_fetcher(&m.base)).await.unwrap_err();
    assert!(err.contains("HTTP 500"), "{err}");
    assert!(t.last_updated().is_none());
    assert_eq!(t.cache_len(), 0);
}

#[tokio::test]
async fn an_unchanged_auction_house_is_not_downloaded_again() {
    let tb = terminator();
    let m = mock(move |page| (200, page_json(20, 42, &[auction(page as u32, 1000.0, &tb)]))).await;
    let f = fast_fetcher(&m.base);
    let mut t = AuctionTracker::new();
    assert!(t.refresh(&f).await.unwrap().is_some());
    let before = m.calls.load(Ordering::SeqCst);
    assert!(t.refresh(&f).await.unwrap().is_none());
    assert_eq!(m.calls.load(Ordering::SeqCst), before + 1);
}

#[tokio::test]
async fn floor_scaled_armor_is_priced_only_from_fully_boosted_listings() {
    let listings: [(f64, i32, i32); 4] = [(40e3, 6, 4), (2e6, 10, 27), (95e6, 10, 50), (3e6, 7, 50)];
    let autos: Vec<String> = listings
        .iter()
        .enumerate()
        .map(|(i, (price, tier, boost))| {
            let b = item_bytes(|w| {
                w.string("id", "SKELETON_MASTER_CHESTPLATE").int("item_tier", *tier).int("baseStatBoostPercentage", *boost);
            });
            auction(i as u32, *price, &b)
        })
        .collect();
    let body = page_json(1, 1, &autos);
    let m = mock(move |_| (200, body.clone())).await;
    let mut t = AuctionTracker::new();
    t.refresh(&fast_fetcher(&m.base)).await.unwrap().unwrap();
    let e = t.latest_lowest();
    assert_eq!(e["SKELETON_MASTER_CHESTPLATE_FLOOR10"], 95e6);
    assert_eq!(e["SKELETON_MASTER_CHESTPLATE_FLOOR7"], 3e6);
    assert_eq!(e["SKELETON_MASTER_CHESTPLATE_FLOOR8"], 95e6);
    assert_eq!(e["SKELETON_MASTER_CHESTPLATE"], 40e3);
}

#[tokio::test]
async fn service_loads_both_sources_and_bumps_version() {
    let tb = terminator();
    let m = mock(move |page| {
        if page == usize::MAX {
            unreachable!()
        }
        (200, page_json(1, 9, &[auction(1, 123.0, &tb)]))
    })
    .await;
    // the mock serves any path with the auction body; the bazaar body is served by a second mock
    let bz = r#"{"success":true,"products":{"A":{"quick_status":{"buyPrice":5.5}},"B":{"quick_status":{"buyPrice":0.5}},"C":{}}}"#;
    let mb = mock(move |_| (200, bz.to_owned())).await;
    let svc = PriceService::with_fetcher(fast_fetcher(&m.base));
    assert!(!svc.ready());
    assert_eq!(svc.version(), 0);
    svc.refresh_auctions().await.unwrap();
    assert!(!svc.ready());
    let bsvc = PriceService::with_fetcher(fast_fetcher(&mb.base));
    bsvc.refresh_bazaar().await.unwrap();
    let t = bsvc.snapshot();
    assert_eq!(t.bazaar.len(), 1);
    assert_eq!(t.get("A"), Some((5.5, dps_core::model::PriceSource::Bazaar)));
    assert_eq!(svc.snapshot().get("TERMINATOR"), Some((123.0, dps_core::model::PriceSource::Auction)));
    assert_eq!(svc.version(), 1);
    // unchanged: no new version
    assert!(!svc.refresh_auctions().await.unwrap());
    assert_eq!(svc.version(), 1);
}

fn tracker_with_history(history: &[&[(&str, f64)]]) -> AuctionTracker {
    let mut t = AuctionTracker::new();
    {
        let mut g = t.interner.lock();
        for m in history {
            for (k, _) in *m {
                g.intern((*k).to_owned());
            }
        }
    }
    for m in history {
        let g = t.interner.lock();
        let mut v = vec![f64::NAN; g.names.len()];
        for (k, p) in *m {
            v[g.map[*k] as usize] = *p;
        }
        drop(g);
        t.history.push(v);
    }
    t
}

#[test]
fn auction_prices_are_the_median_over_recent_downloads_for_items_still_listed() {
    let t = tracker_with_history(&[&[("A", 100.0), ("B", 7.0)], &[("A", 10.0)], &[("A", 120.0), ("C", 5.0)]]);
    let s = t.smooth();
    assert_eq!(s.len(), 2);
    assert_eq!(s["A"], 100.0);
    assert_eq!(s["C"], 5.0);
    let t = tracker_with_history(&[&[("A", 100.0)], &[("A", 200.0)]]);
    assert_eq!(t.smooth()["A"], 150.0);
}

#[test]
fn price_lookups_read_the_current_tables_bazaar_first() {
    let svc = PriceService::new();
    svc.set_tables([("X".to_owned(), 5.0)].into_iter().collect(), [("X".to_owned(), 9.0), ("Y".to_owned(), 2.0)].into_iter().collect());
    let t = svc.snapshot();
    assert_eq!(t.get("X"), Some((5.0, dps_core::model::PriceSource::Bazaar)));
    assert_eq!(t.get("Y"), Some((2.0, dps_core::model::PriceSource::Auction)));
    assert!(svc.ready());
    assert_eq!(t.version, 1);
}

/// NPC offers from src/data/npcShop.json (Combat Merchant: Zombie Talisman 500, Intimidation Talisman 10k; Fear
/// Mongerer: Intimidation Ring = Intimidation Talisman + 100 Green Candy; Agnes: Witch Talisman = 32 Festering Fly).
#[test]
fn an_npc_that_sells_an_item_for_less_than_the_bazaar_or_auction_house_is_used_with_the_npc_named() {
    use dps_core::model::{PriceSource, PriceTables};
    let lookup = |bz: &[(&str, f64)], ah: &[(&str, f64)]| {
        let mut t = PriceTables::default();
        t.bazaar = bz.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        t.auctions = ah.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        t
    };
    let t = lookup(&[], &[]);
    assert_eq!(t.get("ZOMBIE_TALISMAN"), Some((500.0, PriceSource::Npc)));
    assert_eq!(t.npc_route("ZOMBIE_TALISMAN"), vec![("ZOMBIE_TALISMAN", "Combat Merchant")]);
    assert_eq!(lookup(&[], &[("ZOMBIE_TALISMAN", 900.0)]).get("ZOMBIE_TALISMAN"), Some((500.0, PriceSource::Npc)));
    let t = lookup(&[], &[("ZOMBIE_TALISMAN", 400.0)]);
    assert_eq!(t.get("ZOMBIE_TALISMAN"), Some((400.0, PriceSource::Auction)));
    assert!(t.npc_route("ZOMBIE_TALISMAN").is_empty());
    // A tie keeps the market price.
    assert_eq!(lookup(&[("ZOMBIE_TALISMAN", 500.0)], &[]).get("ZOMBIE_TALISMAN"), Some((500.0, PriceSource::Bazaar)));
    // Item parts of an NPC cost are priced the cheapest way too, and NPC-bought parts are named.
    let t = lookup(&[("GREEN_CANDY", 1000.0)], &[]);
    assert_eq!(t.get("INTIMIDATION_RING"), Some((110000.0, PriceSource::Npc)));
    assert_eq!(t.npc_route("INTIMIDATION_RING"), vec![("INTIMIDATION_RING", "Fear Mongerer"), ("INTIMIDATION_TALISMAN", "Combat Merchant")]);
    assert_eq!(lookup(&[("GREEN_CANDY", 1000.0)], &[("INTIMIDATION_RING", 50000.0)]).get("INTIMIDATION_RING"), Some((50000.0, PriceSource::Auction)));
    // A part with no price rules the offer out.
    assert_eq!(lookup(&[], &[]).get("WITCH_TALISMAN"), None);
    assert_eq!(lookup(&[], &[]).get("INTIMIDATION_RING"), None);
    assert_eq!(lookup(&[], &[]).get("TERMINATOR"), None);
}

// ----- parity with the JS bot over real pages ------------------------------------------------------------------

fn td() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/auction-pages")
}

fn load_real() -> Option<(Vec<Vec<u8>>, Vec<u8>, serde_json::Value)> {
    let d = td();
    let expected = std::fs::read(d.join("expected.json")).ok()?;
    let expected: serde_json::Value = serde_json::from_slice(&expected).ok()?;
    let n = expected["pages"].as_u64()? as usize;
    let pages = (0..n).map(|i| std::fs::read(d.join(format!("page{i}.json"))).unwrap()).collect();
    Some((pages, std::fs::read(d.join("bazaar.json")).unwrap(), expected))
}

fn pairs(v: &serde_json::Value) -> Vec<(String, f64)> {
    v.as_array().unwrap().iter().map(|e| (e[0].as_str().unwrap().to_owned(), e[1].as_f64().unwrap())).collect()
}

fn sorted(m: FxHashMap<String, f64>) -> Vec<(String, f64)> {
    let mut v: Vec<_> = m.into_iter().collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

fn run(t: &mut AuctionTracker, pages: &[Vec<u8>], n: usize, lu: f64) -> FxHashMap<String, f64> {
    let outs = pages[..n].iter().map(|p| t.process(p).unwrap()).collect();
    t.accept(lu, outs)
}

#[test]
fn real_pages_match_the_js_lowest_and_smoothed_maps() {
    let Some((pages, bazaar, exp)) = load_real() else {
        eprintln!("testdata/auction-pages missing: skipped (oracle/run.sh skyblock-prices.mjs)");
        return;
    };
    let n = pages.len();
    let mut t = AuctionTracker::new();
    run(&mut t, &pages, n.min(3), 1.0);
    run(&mut t, &pages, n.min(4), 2.0);
    let smooth = run(&mut t, &pages, n, 3.0);
    assert_eq!(sorted(t.latest_lowest()), pairs(&exp["lowest"]), "lowest map");
    assert_eq!(sorted(smooth), pairs(&exp["smooth"]), "smoothed map");
    let mut bz = parse_bazaar(&bazaar).unwrap();
    bz.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(bz, pairs(&exp["bazaar"]), "bazaar");
}

#[test]
fn warm_cache_gives_identical_results() {
    let Some((pages, _, _)) = load_real() else { return };
    let mut t = AuctionTracker::new();
    let a = run(&mut t, &pages, pages.len(), 1.0);
    let before = t.cache_len();
    let outs: Vec<_> = pages.iter().map(|p| t.process(p).unwrap()).collect();
    assert!(outs.iter().all(|o| o.decoded == 0), "second pass must decode nothing");
    let b = t.accept(2.0, outs);
    assert_eq!(t.cache_len(), before);
    assert_eq!(sorted(a), sorted(b));
}

#[test]
#[ignore]
fn bench_decode_and_bazaar() {
    let Some((pages, bazaar, exp)) = load_real() else { return };
    let best = |f: &mut dyn FnMut()| (0..5).map(|_| { let t = std::time::Instant::now(); f(); t.elapsed().as_secs_f64() * 1e3 }).fold(f64::MAX, f64::min);
    let mut cold_decoded = 0;
    let cold = best(&mut || {
        let t = AuctionTracker::new();
        let outs: Vec<_> = pages.iter().map(|p| t.process(p).unwrap()).collect();
        cold_decoded = outs.iter().map(|o| o.decoded).sum();
    });
    let mut t = AuctionTracker::new();
    run(&mut t, &pages, pages.len(), 1.0);
    let warm = best(&mut || {
        let outs: Vec<_> = pages.iter().map(|p| t.process(p).unwrap()).collect();
        assert!(outs.iter().all(|o| o.decoded == 0));
    });
    let accept = best(&mut || {
        let outs: Vec<_> = pages.iter().map(|p| t.process(p).unwrap()).collect();
        t.accept(2.0, outs);
    });
    let bz = best(&mut || {
        parse_bazaar(&bazaar).unwrap();
    });
    let bytes: usize = pages.iter().map(|p| p.len()).sum();
    println!(
        "{} pages, {:.1} MB, {} decoded cold\n  cold (decode all) {cold:.1} ms\n  warm (cache hits)  {warm:.1} ms\n  warm + accept      {accept:.1} ms\n  bazaar parse       {bz:.2} ms\n  JS: auctions {:.0} ms (all pages, cold), bazaar {:.0} ms",
        pages.len(),
        bytes as f64 / 1e6,
        cold_decoded,
        exp["bench"]["auctionMs"].as_f64().unwrap(),
        exp["bench"]["bazaarMs"].as_f64().unwrap(),
    );
}
