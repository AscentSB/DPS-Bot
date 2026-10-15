// npm run debug-profile -- <ign> [class] [set]
// Runs one /dps the way the bot does and prints the full error if it fails. Saves the raw profile to
// debug-<ign>.json (game data only, no keys) so it can be shared for a fix.
import 'dotenv/config';
import fs from 'node:fs';
import { getMinecraftProfile, isValidIgn } from '../src/api/mojang.js';
import { getItemsResource, getSelectedProfile, readMember } from '../src/api/hypixel.js';
import { ensurePrices, snapshotPrices } from '../src/api/prices.js';
import { runDps } from '../src/calc/run.js';

const [ign, cls = 'mage', mode = 'lcm'] = process.argv.slice(2);
if (!isValidIgn(ign)) { console.log('Usage: npm run debug-profile -- <ign> [mage|arch|bers] [lcm|rcm]'); process.exit(1); }
let stage = 'Mojang lookup';
try {
  const account = await getMinecraftProfile(ign);
  if (!account) throw new Error('No Minecraft account with that name.');
  stage = 'Hypixel profile and item list';
  const [resource, profile] = await Promise.all([getItemsResource(), getSelectedProfile(account.uuid)]);
  if (!profile) throw new Error('No SkyBlock profiles.');
  const file = `debug-${account.name}.json`;
  fs.writeFileSync(file, JSON.stringify({ uuid: account.uuid, profile }));
  console.log(`Saved the raw profile to ${file}`);
  stage = 'reading the profile (readMember)';
  const member = readMember(profile, account.uuid, resource);
  console.log(`Read ${member.items.length} items, Catacombs ${member.cataLevel}, bank ${member.petProfile?.bank ?? 'not shared (bank API off)'}.`);
  console.log(`Bank: profile ${profile.banking?.balance ?? 'hidden'}, members ${Object.keys(profile.members ?? {}).length}, personal ${profile.members?.[account.uuid]?.profile?.bank_account ?? 'hidden'}`);
  console.log(`Profile: ${profile.cute_name ?? '?'} (selected: ${Boolean(profile.selected)})`);
  console.log(`Pets read (${member.pets.length}): ${member.pets.map((p) => `${p.type} ${p.tier}${p.heldItem ? ` + ${p.heldItem}` : ''}`).join(', ') || 'none'}`);
  stage = 'loading prices';
  await ensurePrices();
  stage = `planning (${cls} ${mode})`;
  const r = await runDps({ member, cls, mode, target: null, resource, prices: snapshotPrices(), limit: 40 });
  console.log(`OK: ${r.plan?.steps.length ?? 0} steps. First: ${r.plan?.steps[0]?.label ?? '-'}`);
} catch (err) {
  console.log(`\nFAILED during ${stage}:`);
  console.log(err?.stack ?? err);
}
process.exit(0);
