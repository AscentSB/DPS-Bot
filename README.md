# DPS Bot

Discord bot for Hypixel SkyBlock dungeons. `/dps` reads your gear and lists your next upgrades, ranked by weighted % DPS gained per million coins.

## Command

`/dps ign:<name> class:<Archer|Berserker|Mage> [target:<Withers|Dragons>]`

1. Looks up your UUID at `https://api.mojang.com/users/profiles/minecraft/<name>`.
2. Pulls your selected SkyBlock profile from Hypixel. It reads every item by `ITEM_ID` (inventory, armor, equipment, wardrobe, ender chest, backpacks, vault, accessory bag) and your Catacombs level.
3. Picks your best owned item for each slot.
4. Tries every upgrade: the next master star, enchant books, hot/fuming potato books, and buying any weapon, armor, equipment or accessory you don't own.
5. Takes the best % per million, applies it, and repeats. A master star never lands on an item you don't have, and the 5th never comes before the 4th.

Prices come from bazaar instant buy and auction house lowest BIN. NPC prices are never used.

## Weights

| Class | Wither | Dragon |
|---|---|---|
| Archer | 50 | 50 |
| Mage | 80 | 20 |
| Berserker | 20 | 80 |

A global upgrade counts for 100. Picking `target` puts all 100 on that boss.

## Setup

```
npm install
cp .env.example .env   # fill in DISCORD_TOKEN, CLIENT_ID, HYPIXEL_API_KEY, optional GUILD_ID
npm run deploy         # register /dps
npm start
```

## Where to edit

- `src/config/itemStats.js`: add item ids and the stats they grant, misc buffs, reforges.
- `src/config/enchants.js`: enchant stats per level and which boss they affect.
- `src/config/constants.js`: weights, boss defense, formula constants, Catacombs bonus table, soft cap.
- `src/calc/formulas.js`: the damage and stat formulas.

`npm test` runs the formula and planner tests.
