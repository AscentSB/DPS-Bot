//! The /dps registration body (JS `data.toJSON()` of the SlashCommandBuilder in src/commands/dps.js).

use dps_core::config::constants::{
    BLESSING_POWER_MAX, BLESSING_POWER_MIN, BLESSING_TIME_VALUES, BLESSING_WISDOM_MAX, BLESSING_WISDOM_MIN,
};
use serde_json::{json, Value};

fn ign() -> Value {
    json!({"type": 3, "name": "ign", "description": "Your Hypixel IGN", "required": true})
}

fn target() -> Value {
    json!({"type": 3, "choices": [{"name": "Withers", "value": "wither"}, {"name": "Dragons", "value": "dragon"}],
        "name": "target", "description": "Only rank for Withers or Dragons", "required": false})
}

fn blessing_options() -> Vec<Value> {
    let time: Vec<Value> = BLESSING_TIME_VALUES.iter().map(|v| json!({"name": v.to_string(), "value": v})).collect();
    vec![
        json!({"max_value": BLESSING_POWER_MAX, "min_value": BLESSING_POWER_MIN, "type": 4, "name": "power",
            "description": "Blessing of Power level (5-29, default 29)", "required": false}),
        json!({"max_value": BLESSING_WISDOM_MAX, "min_value": BLESSING_WISDOM_MIN, "type": 4, "name": "wisdom",
            "description": "Blessing of Wisdom level (0-14, default 14)", "required": false}),
        json!({"type": 4, "choices": time, "name": "time",
            "description": "Blessing of Time level (0 or 5, default 5)", "required": false}),
    ]
}

fn sub(name: &str, description: &str, mut options: Vec<Value>) -> Value {
    options.extend(blessing_options());
    json!({"type": 1, "name": name, "description": description, "options": options})
}

/// The JSON body for registering the /dps command.
pub fn registration() -> Value {
    let mage = vec![
        ign(),
        json!({"type": 3, "choices": [{"name": "Boss set (LCM)", "value": "lcm"}, {"name": "Clear set (RCM)", "value": "rcm"}],
            "name": "set", "description": "Boss set (LCM beam) or clear set (RCM, 500 speed)", "required": false}),
        json!({"type": 3, "choices": [{"name": "Master Mode", "value": "master"}, {"name": "Normal Mode", "value": "normal"}],
            "name": "floor", "description": "Master Mode (M7) or Normal Mode (F7)", "required": false}),
        target(),
    ];
    json!({
        "name": "dps",
        "description": "Your next DPS upgrades, ranked by DPS gained per million coins",
        "type": 1,
        "options": [
            sub("archer", "Archer upgrades", vec![ign(), target()]),
            sub("berserker", "Berserker upgrades", vec![ign(), target()]),
            sub("mage", "Mage upgrades", mage),
        ],
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn matches_command_json() {
        let base = std::env::var("DPS_TESTDATA").unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata").into());
        let text = std::fs::read_to_string(format!("{base}/command.json")).expect("testdata/command.json");
        let want: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(super::registration(), want);
    }
}
