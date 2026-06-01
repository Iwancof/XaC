import { enemyAttackCooldownTicks, enemyAttackDamage } from "../enemyMetadata";
import type { Block, BlockKind, Enemy, LogLevel, Pos } from "../types";
import { closestPointOnBlock, distance, moveToward } from "./mockGeometry";

const ENEMY_ATTACK_RANGE = 0.2;

export interface MockCombatContext {
  blocks: Block[];
  enemies: Enemy[];
  selectedId: string | null;
  running: boolean;
  log: (level: LogLevel, source: string, message: string) => void;
}

export interface MockCombatCleanupResult {
  blocks: Block[];
  enemies: Enemy[];
  selectedId: string | null;
  running: boolean;
}

export function currentCoreHp(blocks: Block[]) {
  const core = blocks.find((block) => block.kind === "core");
  return Math.max(0, core?.hp ?? 0);
}

export function coreDefeated(blocks: Block[]) {
  return currentCoreHp(blocks) <= 0;
}

export function runEnemies({ blocks, enemies }: MockCombatContext) {
  for (const enemy of enemies) {
    if (enemy.hp <= 0) continue;
    const target = nearestEnemyTarget(blocks, enemy);
    if (!target) continue;

    enemy.target_id = target.block.id;
    if (enemy.attack_cooldown > 0) {
      enemy.attack_cooldown -= 1;
    }

    if (distance(enemy.pos, target.pos) <= ENEMY_ATTACK_RANGE) {
      if (enemy.attack_cooldown === 0) {
        target.block.hp = Math.max(0, target.block.hp - enemyAttackDamage(enemy.kind));
        enemy.attack_cooldown = enemyAttackCooldownTicks(enemy.kind);
      } else {
        target.block.status = `under attack by ${enemy.id}`;
      }
    } else {
      enemy.pos = moveToward(enemy.pos, target.pos, enemy.move_speed);
      target.block.status = `targeted by ${enemy.id}`;
    }
  }
}

export function cleanupDestroyed({
  blocks,
  enemies,
  selectedId,
  running,
  log
}: MockCombatContext): MockCombatCleanupResult {
  const defeatedEnemyIds = new Set(enemies.filter((enemy) => enemy.hp <= 0).map((enemy) => enemy.id));
  if (defeatedEnemyIds.size > 0) {
    for (const id of defeatedEnemyIds) {
      log("info", id, "enemy destroyed");
    }
    enemies = enemies.filter((enemy) => !defeatedEnemyIds.has(enemy.id));
    if (selectedId && defeatedEnemyIds.has(selectedId)) {
      selectedId = null;
    }
  }

  const destroyedBlockIds = new Set(blocks.filter((block) => block.kind !== "core" && block.hp <= 0).map((block) => block.id));
  if (destroyedBlockIds.size > 0) {
    for (const id of destroyedBlockIds) {
      log("warn", id, "block destroyed");
    }
    blocks = blocks.filter((block) => !destroyedBlockIds.has(block.id));
    if (selectedId && destroyedBlockIds.has(selectedId)) {
      selectedId = null;
    }
  }

  const core = blocks.find((block) => block.kind === "core");
  if (core && core.hp <= 0) {
    core.hp = 0;
    if (core.status !== "core breached") {
      core.status = "core breached";
      log("error", core.id, "core destroyed; simulation halted");
    }
    running = false;
  }

  return { blocks, enemies, selectedId, running };
}

function nearestEnemyTarget(blocks: Block[], enemy: Enemy) {
  const targetKinds =
    enemy.kind === "wire_cutter" ? new Set<BlockKind>(["wire", "cpu_node", "drone_port"]) : new Set<BlockKind>(["core"]);
  return (
    nearestBlockTarget(blocks, enemy.pos, (kind) => targetKinds.has(kind)) ??
    nearestBlockTarget(blocks, enemy.pos, (kind) => kind === "core")
  );
}

function nearestBlockTarget(blocks: Block[], origin: Pos, predicate: (kind: BlockKind) => boolean) {
  return blocks
    .filter((block) => predicate(block.kind))
    .map((block) => ({ block, pos: closestPointOnBlock(origin, block) }))
    .sort((a, b) => distance(origin, a.pos) - distance(origin, b.pos))[0];
}
