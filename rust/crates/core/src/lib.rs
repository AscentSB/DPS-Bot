//! Shared types, static SkyBlock data and small pure helpers for the DPS bot: ports of src/config/*.js,
//! src/data/*.json, src/calc/formulas.js, src/lib/{pets,drops,requirements}.js and `cleanName`, plus JS-exact
//! number formatting (`jsfmt`) and math (`jsmath`). Pure Rust, no I/O. See rust/PORTING_NOTES.md.
//!
//! Module map:
//! - [`model`]: the shared JSON types (contract between crates).
//! - [`kinds`]: `Class`, `Target`, `StatTarget`, `FloorMode`, `Mode`, `Rarity` (`from_js` / `js_name`).
//! - [`stat`]: `Stat` (dense `#[repr(u8)]` key, bucket keys first), `StatList` (inline ordered stat object),
//!   `stat_alias` (items-resource stat names).
//! - [`config`]: every table of src/config, as ordered `static` slices plus key lookups.
//! - [`data`]: reforges.json / accessories.json / fragged.json / pets.json, parsed once (`LazyLock`).
//! - [`formulas`]: src/calc/formulas.js (bit-exact, JS operation order).
//! - [`pets`]: src/lib/pets.js (`compute_pet_stats` is `petStats` without the cache).
//! - [`drops`], [`requirements`]: src/lib/drops.js, src/lib/requirements.js.
//! - [`text`]: `clean_name`, JS `trim`, JS `Number(string)`.
//! - [`jsfmt`]: `toFixed`, `toLocaleString('en-US')`, `String(x)`, `Math.round`, `formatCoins`.
//! - [`jsmath`]: V8's `Math.log` and `**` (fdlibm ports, bit-exact), JS `Math.min` / `Math.max`.
//!
//! All table strings are `&'static str` and lookups return `&'static` references, so hot loops never allocate.

pub mod config;
pub mod data;
pub mod drops;
pub mod formulas;
pub mod jsfmt;
pub mod jsmath;
pub mod kinds;
pub mod model;
pub mod pets;
pub mod requirements;
pub mod stat;
pub mod text;

pub use kinds::{Class, FloorMode, Mode, Rarity, StatTarget, Target};
pub use stat::{Stat, StatList};
pub use text::clean_name;
