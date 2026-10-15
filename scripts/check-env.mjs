// npm run check-env: shows whether .env can be read, without printing any secret.
import fs from 'node:fs';
import dotenv from 'dotenv';

if (!fs.existsSync('.env')) {
  const txt = fs.existsSync('.env.txt');
  console.log(txt ? 'Found .env.txt instead of .env. Rename it: Rename-Item .env.txt .env' : 'No .env file in this folder. Create it: copy .env.example .env');
  process.exit(1);
}
const raw = fs.readFileSync('.env');
const utf16 = (raw[0] === 0xff && raw[1] === 0xfe) || (raw[0] === 0xfe && raw[1] === 0xff);
if (utf16) {
  console.log('.env is saved as UTF-16 (Unicode), which cannot be read. In Notepad: File > Save As > Encoding: UTF-8, then save over .env.');
  process.exit(1);
}
const values = dotenv.parse(raw);
console.log('Settings found in .env:', Object.keys(values).join(', ') || '(none)');
for (const key of ['DISCORD_TOKEN', 'CLIENT_ID', 'HYPIXEL_API_KEY']) {
  const v = values[key] ?? '';
  const note = !v ? 'MISSING or empty'
    : /^["'\s]|["'\s]$/.test(v) ? `set (${v.length} characters) but has a space or quote at an end: remove it`
    : key === 'DISCORD_TOKEN' && v.split('.').length !== 3 ? `set (${v.length} characters) but a bot token has 3 parts separated by dots: copy it again from the Bot page`
    : `set (${v.length} characters)`;
  console.log(`  ${key}: ${note}`);
}
if (process.env.DISCORD_TOKEN !== undefined && process.env.DISCORD_TOKEN !== values.DISCORD_TOKEN) {
  console.log('A DISCORD_TOKEN is also set in Windows and overrides .env. Clear it: Remove-Item Env:DISCORD_TOKEN');
}
