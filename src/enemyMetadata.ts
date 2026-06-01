import enemyMetadata from "../assets/enemy_metadata.json";
import type { EnemyKind } from "./types";

type EnemyMetadataSource = {
  maxHp: number;
  moveSpeed: number;
  attackDamage: number;
  attackCooldownTicks: number;
  color: string;
};

export interface EnemyMetadata {
  maxHp: number;
  moveSpeed: number;
  attackDamage: number;
  attackCooldownTicks: number;
  color: number;
}

export const ENEMY_KINDS = ["grunt", "runner", "armored", "wire_cutter"] as const satisfies readonly EnemyKind[];

const metadataSource = enemyMetadata as Record<EnemyKind, EnemyMetadataSource>;

export const ENEMY_METADATA = Object.fromEntries(
  ENEMY_KINDS.map((kind) => [
    kind,
    {
      ...metadataSource[kind],
      color: parseHexColor(metadataSource[kind].color)
    }
  ])
) as Record<EnemyKind, EnemyMetadata>;

export function enemyMaxHp(kind: EnemyKind) {
  return ENEMY_METADATA[kind].maxHp;
}

export function enemyMoveSpeed(kind: EnemyKind) {
  return ENEMY_METADATA[kind].moveSpeed;
}

export function enemyAttackDamage(kind: EnemyKind) {
  return ENEMY_METADATA[kind].attackDamage;
}

export function enemyAttackCooldownTicks(kind: EnemyKind) {
  return ENEMY_METADATA[kind].attackCooldownTicks;
}

export function enemyColor(kind: EnemyKind) {
  return ENEMY_METADATA[kind].color;
}

function parseHexColor(value: string) {
  const match = /^#([0-9a-fA-F]{6})$/.exec(value);
  if (!match) throw new Error(`invalid enemy color ${value}`);
  return Number.parseInt(match[1], 16);
}
