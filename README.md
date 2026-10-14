# DPS Bot

Discord bot for Hypixel SkyBlock dungeons. `/dps` reads your gear and lists your next upgrades, ranked by weighted % DPS gained per million coins.

## Command

`/dps ign:<name> class:<Archer|Berserker|Mage> [target:<Withers|Dragons>] [upgrades:<All levels|Max level only>]`

`upgrades` picks how leveled upgrades are listed. All levels (default) lists Duplex I, II, III... as separate steps. Max level only jumps straight to Duplex V at the price of the Duplex V book, all remaining master stars in one step, Perfect gems, and max essence perk levels.

1. Looks up your UUID at `https://api.mojang.com/users/profiles/minecraft/<name>`.
2. Pulls your selected SkyBlock profile from Hypixel. It reads every item by `ITEM_ID` (inventory, armor, equipment, wardrobe, ender chest, backpacks, vault, accessory bag), your Catacombs level, secrets, pets, selected power and tuning.
3. Picks your best owned item for each slot.
4. Tries every upgrade: master stars, enchant books, potato books, The Art of War, recombobulators, reforge stones, blacksmith reforges (Blended on equipment, Light on boots), gemstones (with slot unlock costs), fragging (8 boss fragments), enrichments, attribute shards, essence shop perks, pets and pet items, ability items, and buying any weapon, armor, equipment, accessory or Ragnarock you don't own. Gear is also offered bundled with its best reforge stone, so a piece that is only worth it reforged (Necron's armor with Ancient) still shows up.
5. Takes the best % per million, applies it, recalculates every option from the new stats, and repeats. A master star never lands on an item you don't have, and the 5th never comes before the 4th. Nothing locks: Duplex I can be followed by Duplex II and later V, and a later reforge or purchase can replace an earlier one, as long as each step adds damage.

Only gear the player can use is offered: Catacombs level, floor completion (from Catacombs level), skill and slayer requirements from the items resource are checked, and shards need the Hunting level to syphon them (Uncommon 5, Rare 10, Epic 15, Legendary 20).

Prices come from bazaar instant buy and auction house lowest BIN. NPC prices are never used. Essence shop perks cost essence × the bazaar price of that essence, and only show at Catacombs 20+ (the bazaar requirement for essence). Blacksmith reforges roll at random for 500 coins, so they cost 500 × the number of basic reforges for that item type (9 for equipment, so 4,500 coins).

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
- Pets: one per boss. Golden Dragon against withers for every class. Ender Dragon against dragons for Archer and Berserker (dragons are Ender mobs). Without that pet, or for Mage against dragons, the owned pet that does the most damage against that boss is used (`PHASE_PETS` in `src/config/pets.js`). All pets count as level 100 (Golden Dragon 200) and are priced from max-level auction listings. Base stats come from NEU; Golden Dragon, Ender Dragon and Jellyfish perks are modeled. Ender Dragon's Superior perk (+10%) and Legion also raise attack speed, tuning points included.
- Tier Boost: a pet with a Tier Boost plays one rarity higher (an Epic Ender Dragon plays as Legendary). Pets can be bought a rarity lower plus a Tier Boost. Swapping the Tier Boost for another pet item can come with a Kat upgrade to keep the rarity: Kat's level-100 cost (30% off), its items, one Kat Flower and the new item.
- Dungeon VII potion: Strength V, Critical III, Archery III (+50% bow damage) and Speed III, always on. A Jellyfish makes it up to 50% stronger and keeps working after you swap pets.
- Armor: one piece per slot. Set and piece bonuses (Skeleton Master, Maxor's, Superior Dragon, Young Dragon, Adaptive) are in `src/config/armorSets.js`. Necron heads give 2x stats on Floor VII.
- DPS = damage per arrow × arrows per volley × volleys per second. Terminator fires 3 arrows + 0.04 per Duplex level + Archer Doubleshot (100% as the only Archer + 1% per Archer level, so 1.5 at level 50), every ceil(10 / (1 + attack speed/100)) ticks, so 4 volleys/s at 100 attack speed. Berserkers use the Terminator too.
- Weird and Weirder Tuba: Howl gets used during dragons only, so the buff (Weirder: +40 strength, +5 attack speed, +10 crit damage) counts against dragons only. Attack speed and volleys are worked out per target.
- Armor only ever gets Ancient (`ARMOR_REFORGE`). Other armor reforges are never offered or assumed.
- Tuning: one setup for the whole run. Points fill attack speed to 100 for whichever boss needs more (usually dragons, with Ender Dragon) first (0.3 per point), then speed to 500 (1.5 per point); only the rest go to crit chance / crit damage. A Crochet Tiger or Hephaestus Plushie on your pet is the usual way to free the attack speed points.
- Speed counts base 100, the Dungeon VII potion, Wolf Slayer, the Rift Blessing of Time perk, gear, reforges, Sugar Rush, accessories, enrichments, Obsidian Defender and Forbidden Speed. Speed only matters in the clear, so your fastest boots (e.g. Maxor's) count for speed while your boss boots count for damage. With Maxor's boots, the boss boots stay Maxor's unless you swap to Necron's. The Rogue Sword is not counted; `EXTRA_SPEED` covers anything else.
- Beacon: a Tier V Beacon powered by Scorched Power Crystals (+20%) is assumed on crit chance (+6) for Archer and Berserker and on ability damage (+6%) for Mage.
- Crit chance: `(flat + 30 + 0.5 × Combat level)(1 + additive)(multiplicative)`. Terminator divides it by 4 (0.25). Combat is read from the profile, maxed (60) when the skills API is off.
- Archer and Berserker hits use the expected value of crits below 100% crit chance and overload between 100% and 200%.
- Class bonuses (each class assumed solo, which doubles its class bonuses): Archer gets x(1 + 1.6% per Archer level) arrow damage, multiplicative (x1.8 at 50). Berserker gets 40 + 1.5 per Berserk level additive damage (115 at 50). Archer and Berserker also get Combat's Warrior bonus: +4 additive per level to 50, +1 per level after (210 at Combat 60).
- Dungeon gear (every dungeon item assumed 5-starred, dungeonizable items like Terminator assumed converted): `stat × (cata boost + 1.5 + 0.05 × master stars + 0.01 × secret digits)`. Crit chance and attack speed skip the cata boost. Speed gets the full multiplier, so Maxor's boots (30 speed) give 30 × 6.35 = 190.5 at Catacombs 50 before master stars and the medallion. Secret digits need a General's Medallion.
- Floor-scaled dungeon armor (Skeleton Master and other pieces with per-floor stats) is assumed dropped from the highest floor your Catacombs level allows (floor 10 at 36, 9 at 34, 8 at 32, 7 at 24, 6 at 19, 5 at 14, 4 at 9, 3 at 5, 2 at 3, 1 at 1) with the full 50% stat boost, which also raises it one rarity.
- Dungeon accessories and their enrichments get x (1 + cata boost) on every stat except crit chance and attack speed (no stars or medallion on accessories). Every damage enrichment is offered: crit damage, crit chance, strength, attack speed and speed.
- A buy can come bundled with its recombobulator, enrichment or reforge as one step, with the prices and gains combined before ranking.
- Accessory Power: `29.97 × ln(0.0019 × AP + 1)^1.2` times your power's stats, plus tuning points and enrichments. Dungeon accessories count double.
- Mage is scored as LCM (the mage beam, used in the boss): each beam hits like a melee hit (strength, crit damage, crit chance, overload), times ability damage, x1.5 vs withers with Hyperion. Beams per second = 20 / round(10 / (1 + attack speed/100)), so attack speed matters. Chimera on the Mage weapon copies the active pet's stats, which then get the weapon's dungeon boost (Golden Dragon's attack speed x1.5 or more from stars). An RCM option can come later.
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
- `src/config/pets.js`: pet items and pet perks.
- `src/config/armorSets.js`: armor set and piece bonuses.
- `src/config/upgrades.js`: ability items.
- `src/config/essence.js`: essence shop perks.
- `src/config/shards.js`: attribute shards.
- `src/config/enchants.js`: enchant stats per level and which boss they affect.
- `src/config/accessories.js`: Accessory Power, powers, tuning, enrichments.
- `src/config/gemstones.js`: gemstone stats.
- `src/config/constants.js`: weights, boss defense, blessing levels (Time 0 or 5, Paul's Benediction), Catacombs table, soft cap.
- `src/calc/formulas.js`: the damage and stat formulas.

`npm run update-data` refreshes reforge stats, accessory families, fragged item recipes and pet stats from the NotEnoughUpdates repo. `npm test` runs the tests.
