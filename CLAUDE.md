# DPS Bot: notes for Claude

Discord bot (`/dps archer|berserker|mage`) for Hypixel SkyBlock M7. It reads a player's profile and ranks DPS upgrades by weighted % DPS per million coins. README.md covers setup, commands, threads and caches. This file holds the game rules the owner settled, so later work keeps them.

## Working on the code

- Node ESM, discord.js 14, worker threads, `node:test`. Run `npm test` before every push (all tests must pass).
- Never write the Hypixel API key or Discord token into files, tests or commits. `.env` is ignored by git.
- Prices: bazaar instant buy, else auction house lowest BIN, and an NPC shop that sells the item for less wins (`src/data/npcShop.json` from NEU, `npm run update-data`; parts of an NPC cost are priced the same way). NPC sell prices (what an NPC pays you) are never used. No guessed prices unless a rule below says so.
- Match the code around you: short comments that say why, config values in `src/config/*`, tests in `test/planner.test.js` for planner rules.
- Scratch profile dumps: `npm run debug-profile <ign>` (needs `HYPIXEL_API_KEY`).
- Rust port in `rust/` (what the Dockerfile ships; Node image is `Dockerfile.node`). The JS in `src/` is the reference: make every rule change in both, with the same labels (`rust/PORTING_NOTES.md`). Test with `cargo test --release --workspace` in `rust/`; parity and bot tests need `rust/testdata/` from `rust/oracle/`.

## Layout

- `src/commands/dps.js`: slash command, buttons, sessions (30s inactivity), run counter (`src/lib/usage.js`, DMs the owner at each new digit).
- `src/calc/stats.js`: damage model (`evaluate`), pets, Legion, SA cloak, Dungeon potion and Jellyfish.
- `src/calc/formulas.js`: hit formulas (`archerDamage`, `berserkerDamage`, Mage).
- `src/calc/upgrades.js`: candidates (`availableUpgrades`), scoring, greedy planner (`planUpgrades`), profile upgrades (`profileUpgrades`).
- `src/api/*`: Hypixel, Mojang, price fetch (price worker). `src/worker/*`: plan workers and eval pool (eval workers rebuild the same candidate list, so candidate code must stay deterministic).
- `src/lib/drops.js`: floor-scaled dungeon drop pricing.
- `rust/crates/*`: `core` (config, formulas), `calc` (`candidates.rs` = availableUpgrades, `parts.rs` = pricing helpers, planner), `skyblock` (APIs), `bot` (Discord).

## Damage model rules

- Dungeon speed: every point over 100 counts for 2/3 (200 -> 167, 300 -> 233). The speed caps (500, Mage boss 450) are dungeon speed, so 500 needs 700 on the stat sheet; tuning fills to that and the reply shows dungeon speed.
- Defense: Withers 9000, dragons 12500. Archer and Berserker use arrows per volley and attack speed.
- Tuning is shared by both bosses and fills attack speed to 100 for whichever needs more. F7 (Mage normal floor) has no dragons, so only the withers count there.
- Crit chance over 100% (overload formula) counts only with the Overload enchant on a bow. Otherwise crit chance stops at 100%.
- Legion: 4 players vs Withers (all classes) and vs dragons for Mage, 1 everywhere else and in RCM.
- Shadow Assassin cloak: 1.1x once per 3s, cooldown cut by Mage class CDR (level/2 %, max 25, doubled solo), averaged over hits. LCM scrolls show "(Shadow Assassin Cloak ability)".
- Dungeon VII potion is assumed. The best owned Jellyfish boosts it (data file: +50% at Legendary 100). The owner was asked whether it should double instead; unconfirmed.
- The One (necklace ultimate): +0.1 / +0.2 strength per maxed collection. Collections count co-op wide, including members who left.
- Berserker additive damage: Warrior (Combat level) + Berserk class bonus (40 + 2 x 0.75 per level) + Lust for Blood (2 x 12 per level, 1200 at Berserk 50 solo).
- Pets: no fixed pet per boss. The owned pet that does the most damage against each boss is active (RCM still keeps an owned Golden Dragon).
- Golden Dragon strength uses gold collection digits (the player's own gold only) and the bank: co-op bank on co-ops, the one bank on solo profiles. Members with a deletion notice count as gone.

## Planner rules

- If an item will later be replaced, do not put many coins into it (25M doomed cap per replaced piece). Replacing early items is fine.
- Floor-scaled dungeon drops are priced from fully boosted (50%) listings at the highest floor the player can enter: c36+ F10, c34-35 F9, c32-33 F8. Drops below F10 follow the 25M cap.
- Archer and Berserker at cata 36+: Skeleton Master Chestplate is the only chestplate (infinite arrows), unless the reply's "Skeleton Master chest" button is turned off. Necron's legs and boots should win the other slots.
- Archer and Berserker swap to clear boots (fastest owned, e.g. Maxor's) for the clear by default. The reply's "Clear boots" button turned off means no clear boots and no clear boots buys: the boss boots also carry the clear speed (speed cap via tuning).
- Golden Necron Head is free at 50 completions ("Claim" step). Diamond Necron Head = Golden + 8 Enchanted Diamond Blocks (~1.6M), upgrades transfer.
- Armor bundles: any mix of pieces can be bought as one step. Copy bundles exist for all classes.
- Mage: detect the RCM Hyperion among 2+ blades. Spare blades become a dual-wield copy, dragon weapon or boss weapon, costing only missing upgrades.
- A Hyperion with Ultimate Wise, Swarm or Combo is the RCM Hyperion: never upgraded into an LCM one (no Chimera, no spare conversion that adds it). A Hyperion with Chimera is the LCM Hyperion: never upgraded into an RCM one (no Swarm). Every mode.
- A max Mage has three Hyperions: RCM, LCM and the LCM one's dual-wield copy. An RCM Hyperion worn for LCM gets an "LCM Hyperion" step (a second Hyperion copying its upgrades, Chimera in place of its ultimate); an LCM one worn for RCM gets an "RCM Hyperion" step (Swarm). Dual wield is never offered on the RCM Hyperion.
- Golden Dragon stays the RCM pet (no Pigman, no pet buys in RCM when a GDrag is owned). Balloon Snake is banned in RCM only.
- First Strike V on a Prosecute weapon is never swapped for Triple Strike. FS IV or none gets Triple Strike.
- Unsold enchant levels: enchanting table enchants cost 1 Grand Experience Bottle (2 if 75 levels). Smite VII = VI + Severed Hand, Ender Slayer VII = VI + End Stone Idol (bazaar item upgrades).
- Sharpness and Smite stack on one sword (Hypixel allows both); only Bane of Arthropods conflicts with them.
- Bank deposit steps merge into one.
- Hatcessories (Crab, Sloth, Balloon and Cake Hats) are one accessory and all equal: owning any one blocks the others. Of owned hats the highest rarity counts, then a recombobulated one, then an enriched one. Balloon Hats (`BALLOON_HAT_2024`, `BALLOON_HAT_2025`) and the Cake Hat (`CAKE_HAT_2026`) are missing from the Hypixel items resource, so `MISSING_ITEMS` adds them as Special accessories.
- "Show max levels": only the best buyable accessory of each family is offered (Wither Relic, never Wither Artifact); a lower tier shows only when the higher ones can't be bought.
- A step with anything bought from an NPC names the NPC in parentheses: "(buy from Combat Merchant)" for a one-part step, else each NPC item by name ("(Intimidation Ring from Fear Mongerer, Intimidation Talisman from Combat Merchant)").

## Profile upgrades (no coins)

- Skills, slayers, class and cata levels, SkyBlock level, Harp songs.
- Gold collection digits (Golden Dragon), one row per unmaxed collection when The One is worn. The Gold row's label shows the strength against the main boss with what raises it: the pet item, Chimera on the weapon (active Golden Dragon) and a Chimera Ragnarock (strongest owned pet), e.g. "+11.1 strength, +81.6 with Chimera".
- Quest accessories (soulbound), listed only when magical power rises: free ones (unpriced) and ones an NPC sells for coins (Odger's teeth, Lucius, Kuudra Follower; the cost and NPC go in the label, never in the coin list). In a chain, list only the next tier that changes rarity.
- 2 IQ Points shows as "+2% intelligence" (additive intelligence).
