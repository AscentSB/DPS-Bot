import test from 'node:test';
import assert from 'node:assert/strict';
import { execute } from '../src/commands/dps.js';

const fakeInteraction = (userId, replies) => ({
  user: { id: userId },
  options: { getSubcommand: () => 'archer', getString: (name) => (name === 'ign' ? '@everyone' : null) },
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
