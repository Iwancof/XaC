import { mockIPC } from "@tauri-apps/api/mocks";
import {
  blockAttackRangeTiles,
  blockMaxHp,
  canAcceptItem,
  displayBlockKind,
  DRILL_MINE_BASE_TICKS
} from "../gameMetadata";
import { detectBehaviorSourceLanguage } from "../behaviorLanguage";
import { enemyMaxHp, enemyMoveSpeed } from "../enemyMetadata";
import { MAP_HEIGHT, MAP_WIDTH, terrainAt } from "../mapSeed";
import {
  emptyMockBehaviorResult,
  evaluateMockBehaviorScript,
  type MockBehaviorResult
} from "./mockBehaviorRuntime";
import { commonTemplates } from "./mockCommonTemplates";
import { blockAt, distance, footprintPositions, rotateDirection } from "./mockGeometry";
import {
  cleanupDestroyed,
  coreDefeated,
  currentCoreHp,
  runEnemies,
  type MockCombatContext
} from "./mockCombat";
import {
  dockedDroneCountAtPort,
  runMockDronePort,
  runMockDrones,
  spawnCarrierDrone as spawnCarrierDroneWithContext,
  type MockDroneContext
} from "./mockDrones";
import { addItem, inventoryCount, inventoryTotal, removeItem } from "./mockInventory";
import {
  networkStockCount,
  outputAvailable,
  outputBlocked,
  transferFrom,
  type MockLogisticsContext
} from "./mockLogistics";
import { recomputeNetworks } from "./mockNetwork";
import { validateMockBehaviorBuild } from "./mockBehaviorValidator";
import {
  clone,
  createInitialMockState,
  createMockBlock,
  type BehaviorOwner,
  type CommandCall,
  type IdKind,
  type MockState,
  type RuntimeEntity
} from "./mockState";
import type {
  BehaviorSource,
  BehaviorSummary,
  Block,
  BlockKind,
  BuildResult,
  Direction,
  Enemy,
  EnemyKind,
  GameSnapshot,
  ItemKind,
  LogLevel,
  Network,
  Pos,
  Tile
} from "../types";

const MOCK_ASSEMBLER_RECIPE_TICKS = 20;
const MOCK_TURRET_DAMAGE = 12;

declare global {
  interface Window {
    __XAC_TEST_STATE__?: {
      calls: CommandCall[];
      reset: () => void;
      snapshot: () => GameSnapshot;
      spawnCarrierDrone: (homePortId?: string) => string;
      spawnEnemy: (kind: EnemyKind, pos: Pos) => string;
      setBlockInventory: (blockId: string, items: Partial<Record<ItemKind, number>>) => void;
      forceOverBudget: (entityId: string) => void;
      forceRuntimeError: (entityId: string, message?: string) => void;
    };
  }
}

let state = createInitialMockState();

mockIPC((cmd, args = {}) => {
  state.calls.push({ cmd, args: clone(args) });

  switch (cmd) {
    case "get_snapshot":
      return snapshot();
    case "set_running":
      state.running = Boolean((args as { running?: boolean }).running) && !coreDefeated(state.blocks);
      return snapshot();
    case "step_ticks":
      runTicks(Number((args as { count?: number }).count ?? 0));
      return snapshot();
    case "advance": {
      if (state.running) {
        runTicks(Number((args as { maxTicks?: number }).maxTicks ?? 0));
      }
      return snapshot();
    }
    case "place_block":
      return placeBlock(args as { kind: BlockKind; x: number; y: number; dir: Direction });
    case "place_blocks":
      return placeBlocks(args as { kind: BlockKind; positions?: Pos[]; dir: Direction });
    case "deconstruct_block":
      return deconstructBlock((args as { blockId?: string }).blockId ?? "");
    case "rotate_block":
      return rotateBlock((args as { blockId?: string }).blockId ?? "");
    case "select_entity":
      state.selectedId = (args as { id?: string | null }).id ?? null;
      return snapshot();
    case "open_behavior":
      return openBehavior((args as { behaviorId?: string }).behaviorId ?? "");
    case "edit_builtin_copy":
      return copyBehavior((args as { blockId?: string }).blockId ?? "", false);
    case "fork_behavior":
      return copyBehavior((args as { blockId?: string }).blockId ?? "", true);
    case "assign_behavior":
      return assignBehavior(args as { blockId?: string; behaviorId?: string });
    case "save_behavior":
      return saveBehavior(args as { behaviorId?: string; source?: string });
    case "build_behavior":
      return buildBehavior((args as { behaviorId?: string }).behaviorId ?? "");
    case "save_world":
      return saveWorld((args as { slot?: string }).slot ?? "quick");
    case "load_world":
      return loadWorld((args as { slot?: string }).slot ?? "quick");
    case "common_templates":
      return commonTemplates();
    default:
      throw new Error(`Unhandled mock IPC command: ${cmd}`);
  }
});

window.__XAC_TEST_STATE__ = {
  calls: state.calls,
  reset: () => {
    state = createInitialMockState();
    window.__XAC_TEST_STATE__!.calls = state.calls;
  },
  snapshot,
  spawnCarrierDrone,
  spawnEnemy,
  setBlockInventory,
  forceOverBudget,
  forceRuntimeError
};

function snapshot(): GameSnapshot {
  const blocks = state.blocks.map((block) => ({ ...block, inventory: clone(block.inventory) }));
  const networks = recomputeNetworks(blocks, state.drones);
  const tiles = buildTiles(blocks);
  const enemies = state.enemies.map((enemy) => ({ ...enemy, pos: { ...enemy.pos } }));
  const drones = state.drones.map((drone) => ({
    ...drone,
    pos: { ...drone.pos },
    cargo: clone(drone.cargo),
    job: drone.job ? clone(drone.job) : null
  }));
  return clone({
    tick: state.tick,
    running: state.running,
    width: MAP_WIDTH,
    height: MAP_HEIGHT,
    tiles,
    blocks,
    enemies,
    drones,
    networks,
    logs: state.logs.slice(-160),
    selected_id: state.selectedId,
    behaviors: behaviorSummaries(),
    pending_jobs: state.pendingJobs,
    item_flows: state.itemFlows.slice(-160),
    status: gameStatus(blocks, networks, enemies)
  });
}

function gameStatus(blocks: Block[], networks: Network[], enemies: Enemy[]) {
  const wavePhase = state.tick % 80;
  const coreHp = currentCoreHp(blocks);
  return {
    wave: Math.floor(state.tick / 80) + 1,
    next_wave_in: wavePhase < 20 ? 20 - wavePhase : 100 - wavePhase,
    core_hp: coreHp,
    core_max_hp: blockMaxHp("core"),
    defeated: coreHp <= 0,
    wire_threats: enemies.filter((enemy) => enemy.kind === "wire_cutter" && enemy.hp > 0).length,
    damaged_wires: blocks.filter((block) => block.kind === "wire" && block.hp < blockMaxHp("wire")).length,
    network_cpu: networks.reduce((total, network) => total + network.cpu_pool, 0)
  };
}

function placeBlock({ kind, x, y, dir }: { kind: BlockKind; x: number; y: number; dir: Direction }) {
  placeBlockInternal(kind, { x, y }, dir);
  return snapshot();
}

function placeBlocks({ kind, positions = [], dir }: { kind: BlockKind; positions?: Pos[]; dir: Direction }) {
  let placed = 0;
  let lastError: Error | null = null;
  for (const pos of positions) {
    try {
      placeBlockInternal(kind, pos, dir);
      placed += 1;
    } catch (error) {
      lastError = error instanceof Error ? error : new Error(String(error));
    }
  }
  if (placed === 0 && lastError) {
    throw lastError;
  }
  return snapshot();
}

function placeBlockInternal(kind: BlockKind, pos: Pos, dir: Direction) {
  if (kind === "core") {
    throw new Error("core is the initial 4x4 objective and cannot be placed");
  }
  const footprint = footprintPositions(kind, pos);
  if (footprint.some((tile) => !inBounds(tile))) {
    throw new Error("position is outside the map");
  }
  if (footprint.some((tile) => blockAt(state.blocks, tile))) {
    throw new Error("tile is not buildable or is already occupied");
  }

  const block = createMockBlock(kind, pos, dir, makeId(kind));
  state.blocks.push(block);
  state.selectedId = block.id;
  log("info", block.id, `placed ${displayBlockKind(kind)} at ${pos.x},${pos.y}`);
}

function deconstructBlock(blockId: string) {
  const block = state.blocks.find((item) => item.id === blockId);
  if (!block) {
    throw new Error(`unknown block: ${blockId}`);
  }
  if (block.kind === "core") {
    throw new Error("core cannot be deconstructed");
  }
  state.blocks = state.blocks.filter((item) => item.id !== blockId);
  if (state.selectedId === blockId) {
    state.selectedId = null;
  }
  log("info", blockId, `deconstructed ${displayBlockKind(block.kind)}`);
  return snapshot();
}

function rotateBlock(blockId: string) {
  const block = state.blocks.find((item) => item.id === blockId);
  if (!block) {
    throw new Error(`unknown block: ${blockId}`);
  }
  block.dir = rotateDirection(block.dir);
  block.status = `facing ${block.dir}`;
  log("info", blockId, `rotated ${displayBlockKind(block.kind)} to ${block.dir}`);
  return snapshot();
}

function runTicks(count: number) {
  for (let i = 0; i < Math.max(0, Math.min(500, count)); i += 1) {
    if (coreDefeated(state.blocks)) {
      state.running = false;
      break;
    }
    state.tick += 1;
    recomputeNetworks(state.blocks, state.drones);

    for (const block of state.blocks) {
      if (block.active && block.behavior_ref) {
        applyMockBehavior(block, runMockBehavior(block));
      }
    }
    for (const block of state.blocks) {
      if (block.kind === "drill" || block.kind === "conveyor" || block.kind === "assembler") {
        transferFrom(logisticsContext(), block);
      }
    }
    runMockDrones(droneContext());
    runEnemies(combatContext());
    const combatCleanup = cleanupDestroyed(combatContext());
    state.blocks = combatCleanup.blocks;
    state.enemies = combatCleanup.enemies;
    state.selectedId = combatCleanup.selectedId;
    state.running = combatCleanup.running;
  }
}

function openBehavior(behaviorId: string): BehaviorSource {
  const behavior = state.behaviors[behaviorId];
  if (!behavior) {
    throw new Error(`unknown behavior: ${behaviorId}`);
  }
  return clone({
    ...behavior,
    summary: {
      ...behavior.summary,
      used_by: usedBy(behavior.summary.id)
    }
  });
}

function copyBehavior(entityId: string, fork: boolean): BehaviorSource {
  const owner = behaviorOwner(entityId);
  const behaviorRef = owner.entity.behavior_ref;
  if (!behaviorRef) throw new Error(`selected ${owner.kind} has no behavior`);

  const original = openBehavior(behaviorRef);
  if (!original.summary.builtin && !fork) {
    return original;
  }

  const id = makeId("behavior");
  const sourcePath = `projects/default_project/blocks/${id}/src/behavior.xac`;
  state.behaviors[id] = {
    summary: {
      ...original.summary,
      id,
      display_name: `${original.summary.display_name} ${fork ? "Fork" : "Copy"}`,
      builtin: false,
      used_by: 1,
      source_language: detectBehaviorSourceLanguage(original.source),
      source_path: sourcePath,
      build_status: fork ? "forked" : "copied"
    },
    source: original.source
  };
  owner.entity.behavior_ref = id;
  log("info", owner.entity.id, `${fork ? "forked" : "created editable copy"} ${id}`);
  return openBehavior(id);
}

function assignBehavior({ blockId = "", behaviorId = "" }: { blockId?: string; behaviorId?: string }) {
  const owner = behaviorOwner(blockId);
  const behavior = state.behaviors[behaviorId];
  if (!behavior) {
    throw new Error(`unknown behavior: ${behaviorId}`);
  }
  if (behavior.summary.base_kind !== owner.behaviorKind) {
    throw new Error(`behavior ${behaviorId} targets ${behavior.summary.base_kind}, but entity is ${owner.behaviorKind}`);
  }
  owner.entity.behavior_ref = behaviorId;
  if (owner.kind === "block") {
    owner.entity.status = `behavior: ${behavior.summary.display_name}`;
  }
  log("info", blockId, `assigned ${behavior.summary.display_name}`);
  return snapshot();
}

function behaviorOwner(entityId: string): BehaviorOwner {
  const block = state.blocks.find((item) => item.id === entityId);
  if (block) {
    const behaviorKind = block.kind;
    if (
      behaviorKind === "drill" ||
      behaviorKind === "router" ||
      behaviorKind === "assembler" ||
      behaviorKind === "turret" ||
      behaviorKind === "drone_port"
    ) {
      return { kind: "block", entity: block, behaviorKind };
    }
    throw new Error("selected block cannot run behavior");
  }
  const drone = state.drones.find((item) => item.id === entityId);
  if (drone) return { kind: "drone", entity: drone, behaviorKind: "carrier_drone" };
  throw new Error(`unknown behavior owner: ${entityId}`);
}

function saveBehavior({ behaviorId = "", source = "" }: { behaviorId?: string; source?: string }): BehaviorSource {
  const behavior = state.behaviors[behaviorId];
  if (!behavior) {
    throw new Error(`unknown behavior: ${behaviorId}`);
  }
  if (behavior.summary.builtin) {
    throw new Error("builtin presets are read-only; create a copy first");
  }
  behavior.source = source;
  behavior.summary.source_language = detectBehaviorSourceLanguage(source);
  behavior.summary.build_status = "saved";
  log("info", behaviorId, "source saved");
  return openBehavior(behaviorId);
}

function buildBehavior(behaviorId: string): BuildResult {
  const behavior = state.behaviors[behaviorId];
  if (!behavior) {
    throw new Error(`unknown behavior: ${behaviorId}`);
  }
  const error = validateMockBehaviorBuild(behavior);
  if (error) {
    behavior.summary.build_status = "build failed";
    log("error", behaviorId, error);
    return {
      behavior_id: behaviorId,
      success: false,
      message: error,
      wasm_hash: null
    };
  }
  behavior.summary.build_status = "built";
  return {
    behavior_id: behaviorId,
    success: true,
    message: "mock build succeeded",
    wasm_hash: "mocked-wasm-hash"
  };
}

function saveWorld(slot: string) {
  const safeSlot = saveSlotName(slot);
  log("info", "system", `world saved to ${safeSlot}`);
  state.saves[safeSlot] = clone({
    tick: state.tick,
    running: state.running,
    blocks: state.blocks,
    enemies: state.enemies,
    drones: state.drones,
    pendingJobs: state.pendingJobs,
    itemFlows: state.itemFlows,
    logs: state.logs,
    selectedId: state.selectedId,
    behaviors: state.behaviors,
    idCounters: state.idCounters
  });
  return snapshot();
}

function loadWorld(slot: string) {
  const safeSlot = saveSlotName(slot);
  const saved = state.saves[safeSlot];
  if (!saved) {
    throw new Error(`unknown save slot: ${safeSlot}`);
  }
  const calls = state.calls;
  const saves = state.saves;
  state = {
    ...clone(saved),
    calls,
    saves
  };
  window.__XAC_TEST_STATE__!.calls = state.calls;
  log("info", "system", `world loaded from ${safeSlot}`);
  return snapshot();
}

function saveSlotName(slot: string) {
  const trimmed = slot.trim();
  if (!trimmed || !/^[A-Za-z0-9_-]+$/.test(trimmed)) {
    throw new Error("save slot can only contain letters, numbers, '-' and '_'");
  }
  return trimmed;
}

function behaviorSummaries(): BehaviorSummary[] {
  return Object.values(state.behaviors).map((behavior) => ({
    ...behavior.summary,
    used_by: usedBy(behavior.summary.id)
  }));
}

function spawnCarrierDrone(homePortId?: string) {
  const id = spawnCarrierDroneWithContext(droneContext(), homePortId);
  state.selectedId = id;
  return id;
}

function spawnEnemy(kind: EnemyKind, pos: Pos) {
  const id = makeId("enemy");
  state.enemies.push({
    id,
    kind,
    pos: { ...pos },
    hp: enemyMaxHp(kind),
    max_hp: enemyMaxHp(kind),
    move_speed: enemyMoveSpeed(kind),
    attack_cooldown: 0,
    target_id: null
  });
  state.selectedId = id;
  log("warn", id, `${kind} test contact`);
  return id;
}

function setBlockInventory(blockId: string, items: Partial<Record<ItemKind, number>>) {
  const block = state.blocks.find((candidate) => candidate.id === blockId);
  if (!block) {
    throw new Error(`unknown block: ${blockId}`);
  }
  block.inventory.items = {};
  for (const [item, amount] of Object.entries(items) as [ItemKind, number][]) {
    if (!canAcceptItem(block.kind, item) || amount <= 0) continue;
    block.inventory.items[item] = Math.min(amount, block.inventory.capacity);
  }
  block.status = inventoryTotal(block.inventory) > 0 ? "test inventory seeded" : "empty";
}

function forceOverBudget(entityId: string) {
  const block = state.blocks.find((candidate) => candidate.id === entityId);
  if (block) {
    block.status = "over_budget";
    recordRuntime(block, 40, 40, 0, "mocked-over-budget-wasm", true);
    log("warn", entityId, "over_budget with 40 fuel");
    return;
  }

  const drone = state.drones.find((candidate) => candidate.id === entityId);
  if (drone) {
    recordRuntime(drone, 40, 40, 0, "mocked-over-budget-wasm", true);
    log("warn", entityId, "drone over_budget with 40 fuel");
    return;
  }

  throw new Error(`unknown runtime entity: ${entityId}`);
}

function forceRuntimeError(entityId: string, message = "mocked wasm unreachable trap") {
  const block = state.blocks.find((candidate) => candidate.id === entityId);
  if (block) {
    block.status = "runtime error";
    recordRuntime(block, 40, 12, 28, "mocked-runtime-error-wasm", false, message);
    log("error", entityId, message);
    return;
  }

  const drone = state.drones.find((candidate) => candidate.id === entityId);
  if (drone) {
    recordRuntime(drone, 40, 12, 28, "mocked-runtime-error-wasm", false, message);
    log("error", entityId, message);
    return;
  }

  throw new Error(`unknown runtime entity: ${entityId}`);
}

function buildTiles(blocks: Block[]): Tile[] {
  const tiles: Tile[] = [];
  for (let y = 0; y < MAP_HEIGHT; y += 1) {
    for (let x = 0; x < MAP_WIDTH; x += 1) {
      const pos = { x, y };
      tiles.push({
        pos,
        terrain: terrainAt(pos),
        buildable: true,
        enemy_passable: true,
        block_id: blockAt(blocks, pos)?.id ?? null
      });
    }
  }
  return tiles;
}

function inBounds(pos: Pos) {
  return pos.x >= 0 && pos.y >= 0 && pos.x < MAP_WIDTH && pos.y < MAP_HEIGHT;
}

function usedBy(behaviorId: string) {
  return (
    state.blocks.filter((block) => block.behavior_ref === behaviorId).length +
    state.drones.filter((drone) => drone.behavior_ref === behaviorId).length
  );
}

function runMockBehavior(block: Block) {
  const minInvocationFuel = 40;
  const fuelRate = block.effective_cpu_rate;
  const maxBank = Math.max(fuelRate * 8, minInvocationFuel);
  block.fuel_bank = Math.min(maxBank, block.fuel_bank + fuelRate / 20);
  if (block.fuel_bank < minInvocationFuel) return emptyMockBehaviorResult();

  const fuelBudget = Math.floor(block.fuel_bank);
  const fuelSpent = Math.min(8, fuelBudget);
  block.fuel_bank = Math.max(0, block.fuel_bank - fuelSpent);
  recordRuntime(block, fuelBudget, fuelSpent, fuelBudget - fuelSpent, "mocked-wasm-hash");
  const source = block.behavior_ref ? state.behaviors[block.behavior_ref]?.source ?? "" : "";
  return evaluateMockBehaviorScript(block, source, {
    outputBlocked: () => outputBlocked(logisticsContext(), block),
    outputAvailable: (dir, item) => outputAvailable(logisticsContext(), block, dir, item),
    terrainAtSelf: () => terrainAt(block.pos),
    visibleTurretTargetCount: () => visibleTurretTargets(block).length,
    canAttackTurretIndex: (index) => Boolean(visibleTurretTargets(block)[index]),
    stockCount: (item) => networkStockCount(state.blocks, block, item),
    dockedDroneCount: () => dockedDroneCountAtPort(droneContext(), block.id),
    pendingJobCount: () => state.pendingJobs.length
  });
}

function applyMockBehavior(block: Block, behavior: MockBehaviorResult) {
  if (behavior.mine) runMockDrill(block);
  if (behavior.output) transferFrom(logisticsContext(), block, behavior.output.dir, behavior.output.item);
  if (behavior.router) runMockRouter(block, behavior.router);
  if (behavior.assembler) runMockAssembler(block, behavior.assembler);
  if (behavior.turret) runMockTurret(block, behavior.turret.priority);
  if (behavior.dronePort) runMockDronePort(droneContext(), block, behavior.dronePort);
}

function runMockDrill(block: Block) {
  if (block.kind !== "drill" || terrainAt(block.pos) !== "ore_patch" || outputBlocked(logisticsContext(), block)) return;
  block.progress += 1;
  if (block.progress >= DRILL_MINE_BASE_TICKS && inventoryCount(block.inventory, "ore") < block.inventory.capacity) {
    block.progress = 0;
    addItem(block.inventory, "ore", 1);
    block.status = "mined ore";
    log("info", block.id, "mined ore");
  }
}

function runMockRouter(block: Block, router: { item: ItemKind | null; dirs: Direction[] }) {
  if (block.kind !== "router") return;
  for (const dir of router.dirs) {
    if (transferFrom(logisticsContext(), block, dir, router.item)) break;
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

function runMockTurret(block: Block, priority: string[]) {
  if (block.kind !== "turret" || inventoryCount(block.inventory, "ammo") <= 0) return;
  const target = chooseTurretTarget(block, priority);
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

function chooseTurretTarget(block: Block, priority: string[]) {
  const targets = visibleTurretTargets(block);
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

function visibleTurretTargets(block: Block) {
  const range = blockAttackRangeTiles("turret") ?? 0;
  const origin = { x: block.pos.x + 0.5, y: block.pos.y + 0.5 };
  return state.enemies
    .filter((enemy) => enemy.hp > 0 && distance(origin, enemy.pos) <= range)
    .sort((a, b) => distance(origin, a.pos) - distance(origin, b.pos));
}

function recordRuntime(
  entity: RuntimeEntity,
  fuelBudget: number,
  fuelSpent: number,
  fuelRemaining: number,
  wasmHash: string,
  overBudget = false,
  runtimeError: string | null = null
) {
  entity.behavior_runtime = {
    last_tick: state.tick,
    run_count: (entity.behavior_runtime?.run_count ?? 0) + 1,
    fuel_budget: fuelBudget,
    fuel_spent: fuelSpent,
    fuel_remaining: fuelRemaining,
    over_budget: overBudget,
    runtime_error: runtimeError,
    wasm_hash: wasmHash
  };
}

function logisticsContext(): MockLogisticsContext {
  return {
    blocks: state.blocks,
    recordItemFlow
  };
}

function combatContext(): MockCombatContext {
  return {
    blocks: state.blocks,
    enemies: state.enemies,
    selectedId: state.selectedId,
    running: state.running,
    log
  };
}

function droneContext(): MockDroneContext {
  return {
    blocks: state.blocks,
    drones: state.drones,
    pendingJobs: state.pendingJobs,
    createId: (kind) => makeId(kind),
    log,
    recordItemFlow
  };
}

function makeId(kind: IdKind) {
  const next = (state.idCounters[kind] ?? 0) + 1;
  state.idCounters[kind] = next;
  return `${kind}_${next}`;
}

function recordItemFlow(fromEntity: string, toEntity: string, item: ItemKind, amount: number, from: Pos, to: Pos) {
  state.itemFlows.push({
    id: makeId("flow"),
    tick: state.tick,
    item,
    amount,
    from_entity: fromEntity,
    to_entity: toEntity,
    from,
    to
  });
  while (state.itemFlows.length > 160) {
    state.itemFlows.shift();
  }
}

function log(level: LogLevel, source: string, message: string) {
  state.logs.push({
    tick: state.tick,
    level,
    source,
    message
  });
  while (state.logs.length > 160) {
    state.logs.shift();
  }
}
