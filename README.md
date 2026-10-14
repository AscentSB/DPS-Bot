# DPS Bot

Discord bot for Hypixel SkyBlock dungeons. `/dps` reads your gear and lists your next upgrades, ranked by weighted % DPS gained per million coins.

## Command

`/dps ign:<name> class:<Archer|Berserker|Mage> [target:<Withers|Dragons>]`

1. Looks up your UUID at `https://api.mojang.com/users/profiles/minecraft/<name>`.
2. Pulls your selected SkyBlock profile from Hypixel. It reads every item by `ITEM_ID` (inventory, armor, equipment, wardrobe, ender chest, backpacks, vault, accessory bag), your Catacombs level, secrets, pets, selected power and tuning.
3. Picks your best owned item for each slot.
4. Tries every upgrade: master stars, enchant books, potato books, The Art of War, recombobulators, reforge stones, gemstones (with slot unlock costs), fragging (8 boss fragments), enrichments, attribute shards, essence shop perks, pets, ability items, and buying any weapon, armor, equipment, accessory or Ragnarock you don't own.
5. Takes the best % per million, applies it, and repeats. A master star never lands on an item you don't have, and the 5th never comes before the 4th. Each item gets at most one reforge, one level per enchant and one gem per slot in a plan.

Prices come from bazaar instant buy and auction house lowest BIN. NPC prices are never used. Essence shop perks cost essence × the bazaar price of that essence, and only show at Catacombs 20+ (the bazaar requirement for essence).

## Weights

| Class | Wither | Dragon |
|---|---|---|
| Archer | 50 | 50 |
| Mage | 80 | 20 |
| Berserker | 20 | 80 |

A global upgrade counts for 100. Picking `target` puts all 100 on that boss.

## Model

- Blessings: Power 19 (strength, crit damage), Wisdom 12 (intelligence) and Time 5 (strength, intelligence; from the Quiz, set it to 0 or 5). Each adds `4 × level × E` flat and multiplies by `1 + 0.02 × level × E`, where `E = 1.2 floor buff × (1 + Forbidden Blessing) × (1 + Mimic shard)`. With Forbidden Blessing X and Mimic X, E is the original 1.1 × 1.1 × 1.2.
- Strength: `(flat + blessing flat)(1 + additive + 2nd additive)(multiplicative)(blessing %)`. Crit damage and intelligence use the same shape.
- Speed: total speed is 100 + Dungeon VII potion (+15) + Wolf Slayer (up to +3) + every tracked gear, accessory and perk source. The Rogue Sword is not counted; `EXTRA_SPEED` covers anything else. Nothing counts until 500. Speed past 500 frees tuning points off speed onto crit damage / crit chance (1 point per 1.5 speed), capped at the points your profile has in speed.
- Crit chance: `(flat + 30)(1 + additive)(multiplicative)`. Terminator's multiplier is 0.25.
- Archer and Berserker hits use the expected value of crits below 100% crit chance and overload between 100% and 200%. Berserker adds 1200 additive damage, Archer 310.
- Dungeon gear: `stat × (1 + cata boost + 0.1 × stars + 0.05 × master stars + 0.01 × secret digits)`. Crit chance skips the cata boost. Secret digits need a General's Medallion.
- Accessory Power: `29.97 × ln(0.0019 × AP + 1)^1.2` times your power's stats, plus tuning points and enrichments. Dungeon accessories count double.
- All damage above 1,000,000 keeps 10%.

The hit formula reproduces the M7 archer sheet's output exactly, and the blessing math matches the wiki's worked example (see `test/formulas.test.js`).

## Setup

```
npm install
cp .env.example .env   # fill in DISCORD_TOKEN, CLIENT_ID, HYPIXEL_API_KEY, optional GUILD_ID
npm run deploy         # register /dps
npm start
```

## Where to edit

- `src/config/itemStats.js`: item ids and the stats they grant, misc buffs, reforge overrides.
- `src/config/upgrades.js`: pets and ability items.
- `src/config/essence.js`: essence shop perks.
- `src/config/shards.js`: attribute shards.
- `src/config/enchants.js`: enchant stats per level and which boss they affect.
- `src/config/accessories.js`: Accessory Power, powers, tuning, enrichments.
- `src/config/gemstones.js`: gemstone stats.
- `src/config/constants.js`: weights, boss defense, blessing levels (Time 0 or 5, Paul's Benediction), Catacombs table, soft cap.
- `src/calc/formulas.js`: the damage and stat formulas.

`npm run update-data` refreshes reforge stats, accessory families and fragged item recipes from the NotEnoughUpdates repo. `npm test` runs the tests.
