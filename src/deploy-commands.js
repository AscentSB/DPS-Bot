import 'dotenv/config';
import { REST, Routes } from 'discord.js';
import * as dps from './commands/dps.js';

const { DISCORD_TOKEN, CLIENT_ID, GUILD_ID } = process.env;
const rest = new REST().setToken(DISCORD_TOKEN);
const body = [dps.data.toJSON()];

// With GUILD_ID the command shows up instantly in that server. Without it, it registers globally.
const route = GUILD_ID ? Routes.applicationGuildCommands(CLIENT_ID, GUILD_ID) : Routes.applicationCommands(CLIENT_ID);
await rest.put(route, { body });
console.log(`Registered /dps ${GUILD_ID ? `in guild ${GUILD_ID}` : 'globally'}.`);
