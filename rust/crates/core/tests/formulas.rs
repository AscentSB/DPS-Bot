//! Port of test/formulas.test.js.

use dps_core::config::constants::{blessings_with, BlessingDef};
use dps_core::formulas::*;
use dps_core::Stat;

fn close(a: f64, b: f64, eps: f64) {
    assert!((a - b).abs() <= eps * 1f64.max(b.abs()), "{a} != {b}");
}

fn close6(a: f64, b: f64) {
    close(a, b, 1e-6);
}

fn js_pow(x: f64, y: f64) -> f64 {
    dps_core::jsmath::pow(x, y)
}

#[test]
fn blessings_match_the_wiki_worked_example() {
    // Power 29 + Time 5, Benediction, Forbidden Blessing X
    let effect = 1.1 * 1.25 * 1.2;
    close6(effect, 1.65);
    let b = apply_blessings_with(
        6000.0,
        Stat::Strength,
        effect,
        &[
            BlessingDef { name: "power", level: 29, stats: &[Stat::Strength] },
            BlessingDef { name: "time", level: 5, stats: &[Stat::Strength] },
        ],
    );
    close(b.flat * b.pct, 14191.04, 1e-6);
}

#[test]
fn default_blessing_effect_is_the_floor_buff_only() {
    let d = DEFAULT_BLESSINGS;
    close6(blessing_effect(0.0, 0.0), 1.2);
    close6(blessing_effect(0.1, 0.1), 1.1 * 1.1 * 1.2);
    // Strength: Power 29 and Time 5 (defaults), flat and percent both scaled by the effect.
    let e = blessing_effect(0.1, 0.1);
    let expected = (1000.0 + 4.0 * 29.0 * e + 4.0 * 5.0 * e) * 1.1 * 1.2 * (1.0 + 0.02 * 29.0 * e) * (1.0 + 0.02 * 5.0 * e);
    close6(calc_strength(1000.0, 0.1, 0.0, 1.2, e, &d), expected);
    close6(calc_crit_damage(1000.0, 0.0, 0.0, 1.0, e, &d), (1000.0 + 4.0 * 29.0 * e) * (1.0 + 0.02 * 29.0 * e));
    close6(
        calc_intelligence(500.0, 0.0, 0.0, 1.0, e, &d),
        (500.0 + 4.0 * 14.0 * e + 4.0 * 5.0 * e) * (1.0 + 0.02 * 14.0 * e) * (1.0 + 0.02 * 5.0 * e),
    );
    // Crit chance from the sheet: flat 424.34 (+30 base), add 0.056, Terminator 0.25 -> 119.95
    close6(calc_crit_chance(424.3383228, 0.056, 0.25), 119.9453172);
}

#[test]
fn expected_hit_matches_the_sheet_output_including_overload() {
    let s = FinalStats {
        damage: 2233.0,
        strength: 10544.21294,
        crit_damage: 8390.796299,
        additive_damage: 807.0,
        crit_chance: 119.9453172,
        ..Default::default()
    };
    close(expected_hit(&s, 0.0, true), 201299506.7, 1e-8);
    // Below 100% crit chance: plain expected value.
    let low = FinalStats { crit_chance: 50.0, ..s };
    let non_crit = 2233.0 * (1.0 + 105.4421294) * (1.0 + 8.07);
    close6(expected_hit(&low, 0.0, true), 0.5 * non_crit * (1.0 + 83.90796299) + 0.5 * non_crit);
    // At 200%+ every hit overloads.
    close6(expected_hit(&FinalStats { crit_chance: 250.0, ..s }, 0.0, true), 1.5 * non_crit * (1.0 + 83.90796299));
}

#[test]
fn catacombs_level_boost_and_dungeon_multiplier() {
    assert_eq!(cata_level_from_xp(0.0), 0);
    assert_eq!(cata_level_from_xp(50.0), 1);
    assert_eq!(cata_level_from_xp(569_809_640.0), 50);
    close6(cata_boost(0.0), 0.10);
    close6(cata_boost(21.0), 1.95);
    close6(cata_boost(60.0), 4.85);
    close6(dungeon_multiplier(50.0, 5.0, 0.0, 0.0), 6.35);
    close6(dungeon_multiplier(50.0, 5.0, 2.0, 3.0), 6.35 + 0.1 + 0.03);
    close6(dungeon_multiplier(50.0, 3.0, 0.0, 0.0), 1.0 + 4.85 + 0.3);
    close6(dungeon_crit_multiplier(5.0, 5.0, 9.0), 1.5 + 0.25 + 0.05);
}

#[test]
fn general_medallion_secret_digits() {
    assert_eq!(secret_digits(0.0), 0.0);
    assert_eq!(secret_digits(1234.0), 4.0);
    assert_eq!(secret_digits(123456.0), 5.0);
}

#[test]
fn accessory_power_multiplier_matches_the_wiki() {
    // Silky at 250 AP is about 220 crit damage.
    close6(power_multiplier(250.0), 29.97 * js_pow(dps_core::jsmath::log(0.0019 * 250.0 + 1.0), 1.2));
    assert!((22.8 * power_multiplier(250.0) - 220.0).abs() < 2.0);
    assert_eq!(power_multiplier(0.0), 0.0);
}

#[test]
fn berserker_adds_extra_additive_damage_archer_takes_a_class_multiplier() {
    let s = FinalStats { damage: 10.0, crit_damage: 0.0, strength: 0.0, additive_damage: 0.0, crit_chance: 100.0, ..Default::default() };
    close6(berserker_damage(&s, 0.0, 115.0, 1.0, false), archer_damage(&s, 0.0, 1.0, false) * 2.15);
    close6(archer_damage(&s, 0.0, 1.8, false), archer_damage(&s, 0.0, 1.0, false) * 1.8);
}

#[test]
fn mage_beam_multipliers_scale_the_beam() {
    let s = FinalStats { damage: 10.0, intelligence: 100.0, ..Default::default() };
    close6(mage_damage(&s, 0.0, 1.5), mage_damage(&s, 0.0, 1.0) * 1.5);
}

#[test]
fn rcm_wither_impact() {
    // 10,000 x cata boost x intelligence (0.3 scaling) x ability damage x magic additive
    let s = FinalStats { intelligence: 1000.0, ability_damage: 50.0, ..Default::default() };
    // 10000 x 1 x (1 + 3) x 1.5 = 60,000.
    close6(rcm_damage(&s, 0.0, 0.0, 0.0, 1.0), 60000.0);
    close6(rcm_damage(&s, 0.0, 0.0, 100.0, 1.0), 120000.0);
    close6(rcm_damage(&s, 0.0, 0.0, 0.0, 1.05), 63000.0);
    // Cata boost 4.85: 60,000 x 5.85 = 351,000. No soft cap: big casts stay whole.
    close6(rcm_damage(&s, 0.0, 4.85, 0.0, 1.0), 351000.0);
    close6(
        rcm_damage(&FinalStats { intelligence: 20000.0, ability_damage: 0.0, ..Default::default() }, 0.0, 4.85, 0.0, 1.0),
        10000.0 * 5.85 * 61.0,
    );
}

#[test]
fn mage_gets_no_overload() {
    let s = FinalStats { damage: 100.0, crit_damage: 500.0, strength: 1000.0, additive_damage: 50.0, intelligence: 5000.0, ..Default::default() };
    let with = |cc: f64| FinalStats { crit_chance: cc, ..s };
    close6(mage_damage(&with(150.0), 0.0, 1.0), mage_damage(&with(100.0), 0.0, 1.0));
    close6(mage_damage(&with(250.0), 0.0, 1.0), mage_damage(&with(100.0), 0.0, 1.0));
    assert!(mage_damage(&with(100.0), 0.0, 1.0) > mage_damage(&with(90.0), 0.0, 1.0));
    // Archer overloads only with the Overload enchant on the bow.
    assert!(archer_damage(&with(150.0), 0.0, 1.0, true) > archer_damage(&with(100.0), 0.0, 1.0, true));
    close6(archer_damage(&with(150.0), 0.0, 1.0, false), archer_damage(&with(100.0), 0.0, 1.0, false));
}

#[test]
fn blessing_levels_can_be_chosen_within_their_limits() {
    let b = blessings_with(Some(10.0), Some(3.0), Some(0.0));
    assert_eq!([b.power, b.wisdom, b.time], [10, 3, 0]);
    let bad = blessings_with(Some(4.0), Some(15.0), Some(3.0));
    assert_eq!([bad.power, bad.wisdom, bad.time], [29, 14, 5]);
    close6(calc_strength(1000.0, 0.0, 0.0, 1.0, 1.2, &b), (1000.0 + 4.0 * 10.0 * 1.2) * (1.0 + 0.02 * 10.0 * 1.2));
}
