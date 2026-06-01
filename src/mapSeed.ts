import mapSeed from "../assets/map_seed.json";
import type { Pos, TerrainKind } from "./types";

type OrePatchSeed = {
  center: readonly [number, number];
  radiusSquared: number;
};

type MapSeed = {
  width: number;
  height: number;
  orePatches: readonly OrePatchSeed[];
};

export const MAP_SEED = mapSeed as unknown as MapSeed;
export const MAP_WIDTH = MAP_SEED.width;
export const MAP_HEIGHT = MAP_SEED.height;

export function terrainAt(pos: Pos): TerrainKind {
  return isOrePatch(pos) ? "ore_patch" : "ground";
}

export function isOrePatch(pos: Pos) {
  return MAP_SEED.orePatches.some((patch) => {
    const dx = pos.x - patch.center[0];
    const dy = pos.y - patch.center[1];
    return dx * dx + dy * dy < patch.radiusSquared;
  });
}
