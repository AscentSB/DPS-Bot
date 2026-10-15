import test from 'node:test';
import assert from 'node:assert/strict';
import { execute } from '../src/commands/dps.js';

const fakeInteraction = (userId, replies, ign = '@everyone') => ({
  user: { id: userId },
  options: { getSubcommand: () => 'archer', getString: (name) => (name === 'ign' ? ign : null) },
  reply: async (msg) => replies.push(msg.content),
});

test('/dps has a 10 second per-user cooldown', async () => {
  const replies = [];
  await execute(fakeInteraction('u1', replies));
  await execute(fakeInteraction('u1', replies));
  await execute(fakeInteraction('u2', replies));
  assert.match(replies[0], /not a valid Minecraft username/);
  assert.match(replies[1], /wait 10s/);
  assert.match(replies[2], /not a valid Minecraft username/);
});

test('/dps answers right away while prices are still loading', async () => {
  const replies = [];
  await execute(fakeInteraction('u3', replies, 'p3wr'));
  assert.match(replies[0], /Prices are still loading/);
});
