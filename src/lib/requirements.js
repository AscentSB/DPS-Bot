import { DUNGEON_FLOOR_CATA, MASTER_FLOOR_CATA } from '../config/constants.js';

// Can the player use this item? Reads the items resource's requirements (and catacombs_requirements).
// player: { cataLevel, skills: { COMBAT: 60, ... } | null, slayers: { spider: 3, ... } | null }.
// Requirements the bot cannot check (collections, Kuudra, Garden, ...) or data the profile hides count as met.
export function meetsRequirements(info, player) {
  if (!player || !info) return true;
  const reqs = [...(info.requirements ?? []), ...(info.catacombs_requirements ?? [])];
  return reqs.every((r) => meets(r, player));
}

function meets(r, player) {
  const cata = player.cataLevel ?? 0;
  switch (r.type) {
    case 'DUNGEON_SKILL':
      return cata >= (r.level ?? 0);
    case 'DUNGEON_TIER': {
      const tier = r.tier ?? 0;
      if (tier <= 0) return true;
      const table = r.dungeon_type === 'MASTER_CATACOMBS' ? MASTER_FLOOR_CATA : DUNGEON_FLOOR_CATA;
      return cata >= (table[Math.min(tier, table.length) - 1] ?? Infinity);
    }
    case 'SKILL': {
      const level = player.skills?.[r.skill];
      return level == null || level >= (r.level ?? 0);
    }
    case 'SLAYER': {
      if (!player.slayers) return true;
      return (player.slayers[r.slayer_boss_type] ?? 0) >= (r.level ?? 0);
    }
    default:
      return true;
  }
}
