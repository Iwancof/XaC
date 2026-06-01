import { mockIPC } from "@tauri-apps/api/mocks";
import {
  blockDefaultBehaviorId,
  blockFootprintSize,
  blockInventoryCapacity,
  blockLocalCpuRate,
  blockMaxHp,
  blockNetworkCpuOutput,
  canAcceptItem,
  displayBlockKind,
  DRILL_MINE_BASE_TICKS,
  isNetworkNode,
  isProgrammableBlock
} from "../gameMetadata";
import { BUILTIN_BEHAVIOR_PRESETS } from "../builtinBehaviors";
import type {
  BehaviorRuntimeStats,
  BehaviorSource,
  BehaviorSummary,
  Block,
  BlockKind,
  BuildResult,
  DeliveryJob,
  Direction,
  Drone,
  Enemy,
  GameSnapshot,
  Inventory,
  ItemFlowEvent,
  ItemKind,
  LogEntry,
  LogLevel,
  Network,
  Pos,
  TerrainKind,
  Tile
} from "../types";

const MAP_WIDTH = 64;
const MAP_HEIGHT = 64;

type CommandCall = {
  cmd: string;
  args: unknown;
};

type MutableBehavior = BehaviorSource;
type IdKind = BlockKind | "behavior" | "drone" | "flow";
type RuntimeEntity = { behavior_runtime: BehaviorRuntimeStats | null };
type BehaviorOwner =
  | { kind: "block"; entity: Block; behaviorKind: BehaviorSummary["base_kind"] }
  | { kind: "drone"; entity: Drone; behaviorKind: "carrier_drone" };

interface MockState {
  tick: number;
  running: boolean;
  blocks: Block[];
  enemies: Enemy[];
  drones: Drone[];
  itemFlows: ItemFlowEvent[];
  logs: LogEntry[];
  selectedId: string | null;
  behaviors: Record<string, MutableBehavior>;
  idCounters: Partial<Record<IdKind, number>>;
  calls: CommandCall[];
}

declare global {
  interface Window {
    __XAC_TEST_STATE__?: {
      calls: CommandCall[];
      reset: () => void;
      snapshot: () => GameSnapshot;
      spawnCarrierDrone: (homePortId?: string) => string;
      forceOverBudget: (entityId: string) => void;
    };
  }
}

let state = createInitialState();

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
    default:
      throw new Error(`Unhandled mock IPC command: ${cmd}`);
  }
});

window.__XAC_TEST_STATE__ = {
  calls: state.calls,
  reset: () => {
    state = createInitialState();
    window.__XAC_TEST_STATE__!.calls = state.calls;
  },
  snapshot,
  spawnCarrierDrone,
  forceOverBudget
};

function createInitialState(): MockState {
  const core = makeBlock("core", { x: 30, y: 30 }, "east", "core_1");
  core.inventory.items = {
    ore: 40,
    plate: 20,
    ammo: 60
  };
  core.status = "core online";
  core.network_id = 1;

  return {
    tick: 0,
    running: false,
    blocks: [core],
    enemies: [
      {
        id: "enemy_1",
        kind: "runner",
        pos: { x: 28.5, y: 28.5 },
        hp: 20,
        max_hp: 20,
        move_speed: 0.14,
        attack_cooldown: 0,
        target_id: core.id
      }
    ],
    drones: [],
    itemFlows: [],
    logs: [
      {
        tick: 0,
        level: "info",
        source: "system",
        message: "XaC MVP world initialized"
      }
    ],
    selectedId: core.id,
    behaviors: builtinBehaviors(),
    idCounters: {
      core: 1
    },
    calls: []
  };
}

function snapshot(): GameSnapshot {
  const blocks = state.blocks.map((block) => ({ ...block, inventory: clone(block.inventory) }));
  const networks = recomputeNetworks(blocks);
  const tiles = buildTiles(blocks);
  const enemies = state.enemies.map((enemy) => ({ ...enemy, pos: { ...enemy.pos } }));
  const drones = state.drones.map((drone) => ({
    ...drone,
    pos: { ...drone.pos },
    cargo: clone(drone.cargo),
    job: drone.job ? clone(drone.job) : null
  }));
  const pendingJobs: DeliveryJob[] = [];

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
    pending_jobs: pendingJobs,
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
  if (kind === "core") {
    throw new Error("core is the initial 4x4 objective and cannot be placed");
  }
  const pos = { x, y };
  const footprint = footprintPositions(kind, pos);
  if (footprint.some((tile) => !inBounds(tile))) {
    throw new Error("position is outside the map");
  }
  if (footprint.some((tile) => blockAt(state.blocks, tile))) {
    throw new Error("tile is not buildable or is already occupied");
  }

  const block = makeBlock(kind, pos, dir);
  state.blocks.push(block);
  state.selectedId = block.id;
  log("info", block.id, `placed ${displayBlockKind(kind)} at ${x},${y}`);
  return snapshot();
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
    recomputeNetworks(state.blocks);

    for (const block of state.blocks) {
      if (block.active && block.behavior_ref) {
        runMockBehavior(block);
      }
      if (block.kind !== "drill" || terrainAt(block.pos) !== "ore_patch") continue;
      block.progress += 1;
      if (block.progress >= DRILL_MINE_BASE_TICKS && inventoryCount(block.inventory, "ore") < block.inventory.capacity) {
        block.progress = 0;
        addItem(block.inventory, "ore", 1);
        block.status = "mined ore";
        log("info", block.id, "mined ore");
      }
    }
    for (const block of state.blocks) {
      if (block.kind === "drill" || block.kind === "conveyor" || block.kind === "assembler") {
        transferFrom(block);
      }
    }
  }
}

function currentCoreHp(blocks: Block[]) {
  const core = blocks.find((block) => block.kind === "core");
  return Math.max(0, core?.hp ?? 0);
}

function coreDefeated(blocks: Block[]) {
  return currentCoreHp(blocks) <= 0;
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
  behavior.summary.build_status = "saved";
  log("info", behaviorId, "source saved");
  return openBehavior(behaviorId);
}

function buildBehavior(behaviorId: string): BuildResult {
  const behavior = state.behaviors[behaviorId];
  if (!behavior) {
    throw new Error(`unknown behavior: ${behaviorId}`);
  }
  behavior.summary.build_status = "built";
  return {
    behavior_id: behaviorId,
    success: true,
    message: "mock build succeeded",
    wasm_hash: "mocked-wasm-hash"
  };
}

function makeBlock(kind: BlockKind, pos: Pos, dir: Direction, id = makeId(kind)): Block {
  return {
    id,
    kind,
    pos,
    dir,
    hp: blockMaxHp(kind),
    inventory: { items: {}, capacity: blockInventoryCapacity(kind) },
    recipe: null,
    behavior_ref: blockDefaultBehaviorId(kind),
    tags: kind === "turret" ? ["frontline"] : [],
    active: isProgrammableBlock(kind),
    network_id: isNetworkNode(kind) ? 1 : null,
    effective_cpu_rate: blockLocalCpuRate(kind),
    fuel_bank: 0,
    behavior_runtime: null,
    progress: 0,
    target_id: null,
    status: "idle"
  };
}

function builtinBehaviors(): Record<string, MutableBehavior> {
  return Object.fromEntries(
    BUILTIN_BEHAVIOR_PRESETS.map((preset) => [
      preset.id,
      {
        summary: {
          id: preset.id,
          display_name: preset.displayName,
          base_kind: preset.baseKind,
          world: preset.world,
          builtin: true,
          used_by: 0,
          source_path: preset.sourcePath,
          build_status: "builtin"
        },
        source: preset.source
      }
    ])
  );
}

function behaviorSummaries(): BehaviorSummary[] {
  return Object.values(state.behaviors).map((behavior) => ({
    ...behavior.summary,
    used_by: usedBy(behavior.summary.id)
  }));
}

function spawnCarrierDrone(homePortId?: string) {
  const port = homePortId
    ? state.blocks.find((block) => block.id === homePortId)
    : state.blocks.find((block) => block.kind === "drone_port");
  if (!port) {
    throw new Error(`unknown drone port: ${homePortId ?? "first drone_port"}`);
  }
  if (port.kind !== "drone_port") {
    throw new Error(`block ${port.id} is not a drone port`);
  }

  const id = makeId("drone");
  state.drones.push({
    id,
    home_port: port.id,
    behavior_ref: "builtin.carrier_drone.basic",
    pos: { x: port.pos.x + 0.5, y: port.pos.y + 0.5 },
    battery: 100,
    logic_fuel: 1000,
    behavior_runtime: null,
    cargo: { items: {}, capacity: 20 },
    state: "docked",
    job: null
  });
  state.selectedId = id;
  log("info", id, `carrier drone docked at ${port.id}`);
  return id;
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

function recomputeNetworks(blocks: Block[]): Network[] {
  const blockIds = blocks.filter((block) => isNetworkNode(block.kind)).map((block) => block.id);
  const activeDevices = blocks.filter((block) => block.active).length;
  const cpuPool = blocks
    .filter((block) => isNetworkNode(block.kind))
    .reduce((sum, block) => sum + blockNetworkCpuOutput(block.kind), 0);
  const effectivePerDevice = activeDevices ? cpuPool / activeDevices : 0;
  for (const block of blocks) {
    block.network_id = isNetworkNode(block.kind) ? 1 : null;
    block.effective_cpu_rate = block.active ? blockLocalCpuRate(block.kind) + effectivePerDevice : 0;
  }
  return [
    {
      id: 1,
      cpu_pool: cpuPool,
      active_devices: activeDevices,
      effective_per_device: effectivePerDevice,
      block_ids: blockIds,
      store: {},
      read_only_cache: false
    }
  ];
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

function terrainAt(pos: Pos): TerrainKind {
  const ore =
    (pos.x - 20) ** 2 + (pos.y - 30) ** 2 < 42 ||
    (pos.x - 42) ** 2 + (pos.y - 25) ** 2 < 30 ||
    (pos.x - 30) ** 2 + (pos.y - 44) ** 2 < 28;
  return ore ? "ore_patch" : "ground";
}

function inBounds(pos: Pos) {
  return pos.x >= 0 && pos.y >= 0 && pos.x < MAP_WIDTH && pos.y < MAP_HEIGHT;
}

function transferFrom(block: Block) {
  const item = Object.entries(block.inventory.items).find(([, amount]) => (amount ?? 0) > 0);
  if (!item) return false;
  const [kind, amount] = item as [ItemKind, number];
  const dst = blockAt(state.blocks, step(block.pos, block.dir));
  if (!dst || !canAcceptItem(dst.kind, kind) || inventoryTotal(dst.inventory) >= dst.inventory.capacity) return false;
  block.inventory.items[kind] = Math.max(0, amount - 1);
  if (block.inventory.items[kind] === 0) delete block.inventory.items[kind];
  addItem(dst.inventory, kind, 1);
  block.status = `sent ${kind}`;
  dst.status = `received ${kind}`;
  recordItemFlow(block.id, dst.id, kind, 1, blockCenter(block), blockCenter(dst));
  return true;
}

function inventoryTotal(inventory: Inventory) {
  return Object.values(inventory.items).reduce((sum, amount) => sum + (amount ?? 0), 0);
}

function blockAt(blocks: Block[], pos: Pos) {
  return blocks.find((block) => footprintPositions(block.kind, block.pos).some((tile) => tile.x === pos.x && tile.y === pos.y));
}

function blockCenter(block: Block): Pos {
  const [width, height] = blockFootprintSize(block.kind);
  return {
    x: block.pos.x + width / 2,
    y: block.pos.y + height / 2
  };
}

function footprintPositions(kind: BlockKind, pos: Pos) {
  const [width, height] = blockFootprintSize(kind);
  const positions: Pos[] = [];
  for (let y = pos.y; y < pos.y + height; y += 1) {
    for (let x = pos.x; x < pos.x + width; x += 1) {
      positions.push({ x, y });
    }
  }
  return positions;
}

function step(pos: Pos, dir: Direction): Pos {
  const delta: Record<Direction, Pos> = {
    north: { x: 0, y: -1 },
    east: { x: 1, y: 0 },
    south: { x: 0, y: 1 },
    west: { x: -1, y: 0 }
  };
  return { x: pos.x + delta[dir].x, y: pos.y + delta[dir].y };
}

function rotateDirection(dir: Direction): Direction {
  const next: Record<Direction, Direction> = {
    north: "east",
    east: "south",
    south: "west",
    west: "north"
  };
  return next[dir];
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
  if (block.fuel_bank < minInvocationFuel) return;

  const fuelBudget = Math.floor(block.fuel_bank);
  const fuelSpent = Math.min(8, fuelBudget);
  block.fuel_bank = Math.max(0, block.fuel_bank - fuelSpent);
  recordRuntime(block, fuelBudget, fuelSpent, fuelBudget - fuelSpent, "mocked-wasm-hash");
}

function recordRuntime(
  entity: RuntimeEntity,
  fuelBudget: number,
  fuelSpent: number,
  fuelRemaining: number,
  wasmHash: string,
  overBudget = false
) {
  entity.behavior_runtime = {
    last_tick: state.tick,
    run_count: (entity.behavior_runtime?.run_count ?? 0) + 1,
    fuel_budget: fuelBudget,
    fuel_spent: fuelSpent,
    fuel_remaining: fuelRemaining,
    over_budget: overBudget,
    wasm_hash: wasmHash
  };
}

function makeId(kind: IdKind) {
  const next = (state.idCounters[kind] ?? 0) + 1;
  state.idCounters[kind] = next;
  return `${kind}_${next}`;
}

function inventoryCount(inventory: Inventory, item: ItemKind) {
  return inventory.items[item] ?? 0;
}

function addItem(inventory: Inventory, item: ItemKind, amount: number) {
  inventory.items[item] = inventoryCount(inventory, item) + amount;
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

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}
