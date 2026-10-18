//! Hypixel, Mojang and price data (port of src/api and src/lib/nbt.js). See rust/PORTING_NOTES.md.
//! Module owners: http, mojang, hypixel, member, nbt = agent "sky-profile"; prices, price_nbt, update_data = agent "sky-prices".

pub mod http;
pub mod mojang;
pub mod hypixel;
pub mod member;
pub mod nbt;
pub mod prices;
pub mod price_nbt;
pub mod update_data;
