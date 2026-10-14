import 'dotenv/config';
import { Client, Events, GatewayIntentBits } from 'discord.js';
import * as dps from './commands/dps.js';
import { startPriceRefresh } from './api/prices.js';

const client = new Client({ intents: [GatewayIntentBits.Guilds] });

client.once(Events.ClientReady, (c) => {
  console.log(`Logged in as ${c.user.tag}`);
  startPriceRefresh();
});

client.on(Events.InteractionCreate, async (interaction) => {
  if (!interaction.isChatInputCommand() || interaction.commandName !== dps.data.name) return;
  await dps.execute(interaction);
});

client.login(process.env.DISCORD_TOKEN);
