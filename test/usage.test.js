import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { createUsageCounter, isNewDigit } from '../src/lib/usage.js';

test('a new digit is 10, 100, 1000 and so on', () => {
  assert.deepEqual([1, 9, 10, 11, 99, 100, 101, 1000, 1010, 10000].filter(isNewDigit), [10, 100, 1000, 10000]);
});

test('the run total persists and notifies once per new digit', async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'usage-'));
  const file = path.join(dir, 'usage.json');
  await fs.writeFile(file, JSON.stringify({ total: 95 }));
  const sent = [];
  const counter = createUsageCounter({ file, notify: async (text) => { sent.push(text); } });
  for (let i = 0; i < 10; i++) await counter.record();
  await counter.flush();
  assert.equal(await counter.total(), 105);
  assert.deepEqual(sent, ['/dps has been run 100 times.']);
  assert.deepEqual(JSON.parse(await fs.readFile(file, 'utf8')), { total: 105 });
  const again = createUsageCounter({ file, notify: async () => {} });
  assert.equal(await again.total(), 105);
  await fs.rm(dir, { recursive: true });
});
