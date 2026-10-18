//! src/lib/requirements.js: can the player use this item?

use crate::config::constants::{DUNGEON_FLOOR_CATA, MASTER_FLOOR_CATA};
use crate::model::{ItemInfo, Map, Requirement};

/// JS `ctx.player`: `{ cataLevel, skills, slayers }` (`None` maps = the profile hides them).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Player {
    pub cata_level: f64,
    pub skills: Option<Map<i32>>,
    pub slayers: Option<Map<i32>>,
}

/// JS `meetsRequirements(info, player)`: every `requirements` and `catacombs_requirements` entry is met. Missing
/// info or player, unknown requirement types and hidden profile data count as met.
pub fn meets_requirements(info: Option<&ItemInfo>, player: Option<&Player>) -> bool {
    let (Some(info), Some(player)) = (info, player) else {
        return true;
    };
    let reqs = info.requirements.iter().flatten().chain(info.catacombs_requirements.iter().flatten());
    for r in reqs {
        if !meets(r, player) {
            return false;
        }
    }
    true
}

fn meets(r: &Requirement, player: &Player) -> bool {
    let cata = player.cata_level;
    let level = r.level.unwrap_or(0.0);
    match r.kind.as_str() {
        "DUNGEON_SKILL" => cata >= level,
        "DUNGEON_TIER" => {
            let tier = r.tier.unwrap_or(0.0);
            if tier <= 0.0 {
                return true;
            }
            let table: &[f64] = if r.dungeon_type.as_deref() == Some("MASTER_CATACOMBS") { &MASTER_FLOOR_CATA } else { &DUNGEON_FLOOR_CATA };
            // table[Math.min(tier, table.length) - 1] ?? Infinity
            let idx = crate::jsmath::min(tier, table.len() as f64) - 1.0;
            let need = if idx >= 0.0 && idx.fract() == 0.0 { table[idx as usize] } else { f64::INFINITY };
            cata >= need
        }
        "SKILL" => {
            let key = r.skill.as_deref().unwrap_or("undefined");
            match player.skills.as_ref().and_then(|s| s.get(key)) {
                None => true,
                Some(&l) => l as f64 >= level,
            }
        }
        "SLAYER" => {
            let Some(slayers) = player.slayers.as_ref() else {
                return true;
            };
            let key = r.slayer_boss_type.as_deref().unwrap_or("undefined");
            slayers.get(key).copied().unwrap_or(0) as f64 >= level
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(json: &str) -> ItemInfo {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn requirement_rules() {
        let p = Player {
            cata_level: 30.0,
            skills: Some([("COMBAT".to_string(), 24)].into_iter().collect()),
            slayers: Some([("zombie".to_string(), 5)].into_iter().collect()),
        };
        let ok = |j: &str| meets_requirements(Some(&info(j)), Some(&p));
        assert!(ok(r#"{"id":"A"}"#));
        assert!(ok(r#"{"id":"A","requirements":[{"type":"DUNGEON_SKILL","level":30}]}"#));
        assert!(!ok(r#"{"id":"A","requirements":[{"type":"DUNGEON_SKILL","level":31}]}"#));
        assert!(ok(r#"{"id":"A","catacombs_requirements":[{"type":"DUNGEON_TIER","tier":7,"dungeon_type":"CATACOMBS"}]}"#));
        assert!(!ok(r#"{"id":"A","catacombs_requirements":[{"type":"DUNGEON_TIER","tier":5,"dungeon_type":"MASTER_CATACOMBS"}]}"#));
        assert!(ok(r#"{"id":"A","catacombs_requirements":[{"type":"DUNGEON_TIER","tier":4,"dungeon_type":"MASTER_CATACOMBS"}]}"#));
        assert!(ok(r#"{"id":"A","catacombs_requirements":[{"type":"DUNGEON_TIER","tier":4,"dungeon_type":"CATACOMBS"}]}"#));
        assert!(!ok(r#"{"id":"A","catacombs_requirements":[{"type":"DUNGEON_TIER","tier":99,"dungeon_type":"CATACOMBS"}]}"#));
        assert!(ok(r#"{"id":"A","catacombs_requirements":[{"type":"DUNGEON_TIER","tier":0}]}"#));
        assert!(ok(r#"{"id":"A","requirements":[{"type":"SKILL","skill":"COMBAT","level":24}]}"#));
        assert!(!ok(r#"{"id":"A","requirements":[{"type":"SKILL","skill":"COMBAT","level":25}]}"#));
        assert!(ok(r#"{"id":"A","requirements":[{"type":"SKILL","skill":"MINING","level":60}]}"#));
        assert!(ok(r#"{"id":"A","requirements":[{"type":"SLAYER","slayer_boss_type":"zombie","level":5}]}"#));
        assert!(!ok(r#"{"id":"A","requirements":[{"type":"SLAYER","slayer_boss_type":"wolf","level":1}]}"#));
        assert!(ok(r#"{"id":"A","requirements":[{"type":"COLLECTION","level":9}]}"#));
        assert!(meets_requirements(None, Some(&p)));
        assert!(meets_requirements(Some(&info(r#"{"id":"A","requirements":[{"type":"DUNGEON_SKILL","level":99}]}"#)), None));
        let hidden = Player { cata_level: 0.0, skills: None, slayers: None };
        assert!(meets_requirements(
            Some(&info(r#"{"id":"A","requirements":[{"type":"SLAYER","slayer_boss_type":"wolf","level":9},{"type":"SKILL","skill":"COMBAT","level":60}]}"#)),
            Some(&hidden)
        ));
    }
}
