import 'dotenv/config';
import { Client, Events, GatewayIntentBits } from 'discord.js';
import * as dps from './commands/dps.js';
import { startWorker } from './worker/client.js';
import { createInteractionServer } from './http/interactions.js';
import { startPriceRefresh } from './api/prices.js';
import { getItemsResource } from './api/hypixel.js';

async function route(interaction) {
  if (interaction.isButton() && interaction.customId.startsWith('dps:')) return dps.handleButton(interaction);
  if (!interaction.isChatInputCommand() || interaction.commandName !== dps.data.name) return;
  await dps.execute(interaction);
}

if (!process.env.HYPIXEL_API_KEY?.trim()) console.warn('HYPIXEL_API_KEY is missing from .env: /dps cannot read profiles until it is set.');

// Prices (bazaar and auction house, every 5 minutes) and the item list (hourly) load once at launch, in the
// background; commands read the cache and say "try again" until the first load is done.
startPriceRefresh();
getItemsResource().catch((err) => console.error('Item list load failed:', err.message));

// Interactions arrive one of two ways (Discord sends them to only one): over the Gateway (default), or over HTTP
// when INTERACTIONS_PORT is set and the app's Interactions Endpoint URL points at this server.
const port = Number(process.env.INTERACTIONS_PORT);
if (port) {
  const publicKey = process.env.DISCORD_PUBLIC_KEY;
  if (!/^[0-9a-f]{64}$/i.test(publicKey ?? '')) throw new Error('DISCORD_PUBLIC_KEY (64 hex characters) is needed for HTTP interactions.');
  startWorker();
  createInteractionServer({ publicKey, onInteraction: route }).listen(port, () => console.log(`HTTP interactions on port ${port}`));
} else {
  if (!process.env.DISCORD_TOKEN?.trim()) {
    console.error('DISCORD_TOKEN is missing. Put DISCORD_TOKEN=your-bot-token in a file named .env (not .env.txt) in the bot folder, next to package.json.');
    process.exit(1);
  }
  // No message from the bot may ping anyone, whatever text ends up in it.
  const client = new Client({ intents: [GatewayIntentBits.Guilds], allowedMentions: { parse: [], repliedUser: false } });
  client.once(Events.ClientReady, (c) => {
    console.log(`Logged in as ${c.user.tag}`);
    startWorker();
  });
  client.on(Events.InteractionCreate, route);
  client.login(process.env.DISCORD_TOKEN);
}
