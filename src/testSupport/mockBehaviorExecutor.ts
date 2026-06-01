import { blockAttackRangeTiles, DRILL_MINE_BASE_TICKS } from "../gameMetadata";
import { terrainAt } from "../mapSeed";
import type { Block, Direction, ItemKind, LogLevel } from "../types";
import {
  emptyMockBehaviorResult,
  evaluateMockBehaviorScript,
  type MockBehaviorResult
} from "./mockBehaviorRuntime";
import { runMockDronePort, dockedDroneCountAtPort, type MockDroneContext } from "./mockDrones";
import { distance } from "./mockGeometry";
import { addItem, inventoryCount, inventoryTotal, removeItem } from "./mockInventory";
import {
  networkStockCount,
  outputAvailable,
  outputBlocked,
  transferFrom,
  type MockLogisticsContext
} from "./mockLogistics";
import type { MockState, RuntimeEntity } from "./mockState";

const MOCK_ASSEMBLER_RECIPE_TICKS = 20;
const MOCK_TURRET_DAMAGE = 12;

export type MockBehaviorExecutionContext = {
  state: MockState;
  logistics: () => MockLogisticsContext;
  drones: () => MockDroneContext;
  log: (level: LogLevel, source: string, message: string) => void;
};

export function runMockBlockBehavior(context: MockBehaviorExecutionContext, block: Block) {
  if (!block.active || !block.behavior_ref) return;
  applyMockBehavior(context, block, runMockBehavior(context, block));
}

export function forceMockOverBudget(context: MockBehaviorExecutionContext, entityId: string) {
  const block = context.state.blocks.find((candidate) => candidate.id === entityId);
  if (block) {
    block.status = "over_budget";
    recordMockRuntime(context, block, 40, 40, 0, "mocked-over-budget-wasm", true);
    context.log("warn", entityId, "over_budget with 40 fuel");
    return;
  }

  const drone = context.state.drones.find((candidate) => candidate.id === entityId);
  if (drone) {
    recordMockRuntime(context, drone, 40, 40, 0, "mocked-over-budget-wasm", true);
    context.log("warn", entityId, "drone over_budget with 40 fuel");
    return;
  }

  throw new Error(`unknown runtime entity: ${entityId}`);
}

export function forceMockRuntimeError(
  context: MockBehaviorExecutionContext,
  entityId: string,
  message = "mocked wasm unreachable trap"
) {
  const block = context.state.blocks.find((candidate) => candidate.id === entityId);
  if (block) {
    block.status = "runtime error";
    recordMockRuntime(context, block, 40, 12, 28, "mocked-runtime-error-wasm", false, message);
    context.log("error", entityId, message);
    return;
  }

  const drone = context.state.drones.find((candidate) => candidate.id === entityId);
  if (drone) {
    recordMockRuntime(context, drone, 40, 12, 28, "mocked-runtime-error-wasm", false, message);
    context.log("error", entityId, message);
    return;
  }

  throw new Error(`unknown runtime entity: ${entityId}`);
}

function runMockBehavior(context: MockBehaviorExecutionContext, block: Block) {
  const minInvocationFuel = 40;
  const fuelRate = block.effective_cpu_rate;
  const maxBank = Math.max(fuelRate * 8, minInvocationFuel);
  block.fuel_bank = Math.min(maxBank, block.fuel_bank + fuelRate / 20);
  if (block.fuel_bank < minInvocationFuel) return emptyMockBehaviorResult();

  const fuelBudget = Math.floor(block.fuel_bank);
  const fuelSpent = Math.min(8, fuelBudget);
  block.fuel_bank = Math.max(0, block.fuel_bank - fuelSpent);
  recordMockRuntime(context, block, fuelBudget, fuelSpent, fuelBudget - fuelSpent, "mocked-wasm-hash");
  const source = block.behavior_ref ? context.state.behaviors[block.behavior_ref]?.source ?? "" : "";
  return evaluateMockBehaviorScript(block, source, {
    outputBlocked: () => outputBlocked(context.logistics(), block),
    outputAvailable: (dir, item) => outputAvailable(context.logistics(), block, dir, item),
    terrainAtSelf: () => terrainAt(block.pos),
    visibleTurretTargetCount: () => visibleTurretTargets(context, block).length,
    canAttackTurretIndex: (index) => Boolean(visibleTurretTargets(context, block)[index]),
    stockCount: (item) => networkStockCount(context.state.blocks, block, item),
    dockedDroneCount: () => dockedDroneCountAtPort(context.drones(), block.id),
    pendingJobCount: () => context.state.pendingJobs.length
  });
}

function applyMockBehavior(context: MockBehaviorExecutionContext, block: Block, behavior: MockBehaviorResult) {
  if (behavior.mine) runMockDrill(context, block);
  if (behavior.output) transferFrom(context.logistics(), block, behavior.output.dir, behavior.output.item);
  if (behavior.router) runMockRouter(context, block, behavior.router);
  if (behavior.assembler) runMockAssembler(block, behavior.assembler);
  if (behavior.turret) runMockTurret(context, block, behavior.turret.priority);
  if (behavior.dronePort) runMockDronePort(context.drones(), block, behavior.dronePort);
}

function runMockDrill(context: MockBehaviorExecutionContext, block: Block) {
  if (block.kind !== "drill" || terrainAt(block.pos) !== "ore_patch" || outputBlocked(context.logistics(), block)) return;
  block.progress += 1;
  if (block.progress >= DRILL_MINE_BASE_TICKS && inventoryCount(block.inventory, "ore") < block.inventory.capacity) {
    block.progress = 0;
    addItem(block.inventory, "ore", 1);
    block.status = "mined ore";
    context.log("info", block.id, "mined ore");
  }
}

function runMockRouter(
  context: MockBehaviorExecutionContext,
  block: Block,
  router: { item: ItemKind | null; dirs: Direction[] }
) {
  if (block.kind !== "router") return;
  for (const dir of router.dirs) {
    if (transferFrom(context.logistics(), block, dir, router.item)) break;
  }
}

function runMockAssembler(block: Block, assembler: { recipe: ItemKind | null; produce: boolean }) {
  if (block.kind !== "assembler") return;
  if (assembler.recipe) {
    block.recipe = assembler.recipe;
    block.status = `recipe: ${assembler.recipe}`;
  }
  if (!assembler.produce || !canMockProduce(block, block.recipe)) return;
  block.progress += 1;
  if (block.progress < MOCK_ASSEMBLER_RECIPE_TICKS) return;
  block.progress = 0;
  if (block.recipe === "plate") {
    removeItem(block.inventory, "ore", 2);
    addItem(block.inventory, "plate", 1);
    block.status = "produced plate";
  } else if (block.recipe === "ammo") {
    removeItem(block.inventory, "plate", 1);
    addItem(block.inventory, "ammo", 2);
    block.status = "produced ammo";
  }
}

function runMockTurret(context: MockBehaviorExecutionContext, block: Block, priority: string[]) {
  if (block.kind !== "turret" || inventoryCount(block.inventory, "ammo") <= 0) return;
  const target = chooseTurretTarget(context, block, priority);
  if (!target) return;
  target.hp -= MOCK_TURRET_DAMAGE;
  removeItem(block.inventory, "ammo", 1);
  block.target_id = target.id;
  block.status = `attacking ${target.id}`;
}

function canMockProduce(block: Block, recipe: string | null) {
  if (recipe === "plate") {
    return inventoryCount(block.inventory, "ore") >= 2 && inventoryTotal(block.inventory) < block.inventory.capacity;
  }
  if (recipe === "ammo") {
    return inventoryCount(block.inventory, "plate") >= 1 && inventoryTotal(block.inventory) + 1 < block.inventory.capacity;
  }
  return false;
}

function chooseTurretTarget(context: MockBehaviorExecutionContext, block: Block, priority: string[]) {
  const targets = visibleTurretTargets(context, block);
  if (targets.length === 0) return null;
  for (const rawRule of priority) {
    for (const rule of rawRule.split(",").map((part) => part.trim()).filter(Boolean)) {
      if (rule.startsWith("index:")) {
        const target = targets[Number(rule.slice("index:".length))];
        if (target) return target;
      }
      if (rule === "lowest_hp" || rule === "weakest") return [...targets].sort((a, b) => a.hp - b.hp)[0];
      if (rule === "nearest") return targets[0];
      const byKind = targets.find((target) => target.kind === rule || (rule === "wire-cutter" && target.kind === "wire_cutter"));
      if (byKind) return byKind;
    }
  }
  return targets[0];
}

function visibleTurretTargets(context: MockBehaviorExecutionContext, block: Block) {
  const range = blockAttackRangeTiles("turret") ?? 0;
  const origin = { x: block.pos.x + 0.5, y: block.pos.y + 0.5 };
  return context.state.enemies
    .filter((enemy) => enemy.hp > 0 && distance(origin, enemy.pos) <= range)
    .sort((a, b) => distance(origin, a.pos) - distance(origin, b.pos));
}

function recordMockRuntime(
  context: MockBehaviorExecutionContext,
  entity: RuntimeEntity,
  fuelBudget: number,
  fuelSpent: number,
  fuelRemaining: number,
  wasmHash: string,
  overBudget = false,
  runtimeError: string | null = null
) {
  entity.behavior_runtime = {
    last_tick: context.state.tick,
    run_count: (entity.behavior_runtime?.run_count ?? 0) + 1,
    fuel_budget: fuelBudget,
    fuel_spent: fuelSpent,
    fuel_remaining: fuelRemaining,
    over_budget: overBudget,
    runtime_error: runtimeError,
    wasm_hash: wasmHash
  };
}
