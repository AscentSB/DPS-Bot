// testdata/snapshot.json = testdata/snapshot.raw.json plus "now" (the raw file's mtime in ms), which the other
// scripts use as Date.now() for readMember. Everything after the added "now" field is the raw file byte for byte.
import fs from 'node:fs';
import path from 'node:path';
import { TD } from './common.mjs';

const rawPath = path.join(TD, 'snapshot.raw.json');
const outPath = path.join(TD, 'snapshot.json');
const raw = fs.readFileSync(rawPath, 'utf8');
const now = Math.floor(fs.statSync(rawPath).mtimeMs);
if (!raw.startsWith('{') || raw.startsWith('{"now"')) throw new Error('snapshot.raw.json is not the expected object.');
const out = `{"now":${now},${raw.slice(1)}`;

// Check: same content as the raw file plus now.
const a = JSON.parse(raw);
const b = JSON.parse(out);
const { now: n, ...rest } = b;
if (n !== now || JSON.stringify(rest) !== JSON.stringify(a)) throw new Error('snapshot.json does not match snapshot.raw.json.');

fs.writeFileSync(outPath, out);
console.log(`snapshot.json: now=${now} (${new Date(now).toISOString()}), items=${a.items.length}, collections=${a.collections.length}, bazaar=${a.bazaar.length}, auctions=${a.auctions.length}`);
