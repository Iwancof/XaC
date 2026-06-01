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
  isNetworkConnector,
  isNetworkNode,
  isProgrammableBlock
} from "../gameMetadata";
import { BUILTIN_BEHAVIOR_PRESETS } from "../builtinBehaviors";
import { detectBehaviorSourceLanguage } from "../behaviorLanguage";
import { enemyAttackCooldownTicks, enemyAttackDamage, enemyMaxHp, enemyMoveSpeed } from "../enemyMetadata";
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
  EnemyKind,
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
const ENEMY_ATTACK_RANGE = 0.2;

type CommandCall = {
  cmd: string;
  args: unknown;
};

type MutableBehavior = BehaviorSource;
type IdKind = BlockKind | "behavior" | "drone" | "enemy" | "flow";
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
      spawnEnemy: (kind: EnemyKind, pos: Pos) => string;
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
  spawnEnemy,
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
        hp: enemyMaxHp("runner"),
        max_hp: enemyMaxHp("runner"),
        move_speed: enemyMoveSpeed("runner"),
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
      core: 1,
      enemy: 1
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
      if (block.kind !== "drill" || terrainAt(block.pos) !== "ore_patch" || outputBlocked(block)) continue;
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
    runEnemies();
    cleanupDestroyed();
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
          source_language: detectBehaviorSourceLanguage(preset.source),
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
  for (const block of blocks) {
    block.network_id = null;
    block.effective_cpu_rate = block.active ? blockLocalCpuRate(block.kind) : 0;
  }

  const connectorPositions = new Set<string>();
  for (const block of blocks) {
    if (!isNetworkConnector(block.kind)) continue;
    for (const pos of footprintPositions(block.kind, block.pos)) {
      connectorPositions.add(posKey(pos));
    }
  }

  const networks: Network[] = [];
  const seen = new Set<string>();
  const starts = [...connectorPositions]
    .map(parsePosKey)
    .sort((a, b) => a.y - b.y || a.x - b.x);

  for (const start of starts) {
    if (seen.has(posKey(start))) continue;
    const component = connectedConnectorComponent(start, connectorPositions, seen);
    const blockIds = networkBlockIds(blocks, component);
    const cpuPool = blockIds.reduce((sum, id) => {
      const block = blocks.find((candidate) => candidate.id === id);
      return sum + (block ? blockNetworkCpuOutput(block.kind) : 0);
    }, 0);
    const activeDevices =
      blockIds.filter((id) => blocks.find((block) => block.id === id)?.active).length +
      dockedDroneCountInNetwork(blockIds);
    const effectivePerDevice = activeDevices ? cpuPool / activeDevices : 0;
    const networkId = networks.length + 1;

    for (const id of blockIds) {
      const block = blocks.find((candidate) => candidate.id === id);
      if (!block) continue;
      block.network_id = networkId;
      if (block.active) {
        block.effective_cpu_rate = blockLocalCpuRate(block.kind) + effectivePerDevice;
      }
    }

    networks.push({
      id: networkId,
      cpu_pool: cpuPool,
      active_devices: activeDevices,
      effective_per_device: effectivePerDevice,
      block_ids: blockIds,
      store: {},
      read_only_cache: !blockIds.some((id) => blocks.find((block) => block.id === id)?.kind === "core")
    });
  }

  return networks;
}

function connectedConnectorComponent(start: Pos, connectorPositions: Set<string>, seen: Set<string>) {
  const queue = [start];
  const component: Pos[] = [];
  seen.add(posKey(start));

  while (queue.length > 0) {
    const current = queue.shift()!;
    component.push(current);
    for (const dir of allDirections()) {
      const next = step(current, dir);
      const key = posKey(next);
      if (connectorPositions.has(key) && !seen.has(key)) {
        seen.add(key);
        queue.push(next);
      }
    }
  }

  return component;
}

function networkBlockIds(blocks: Block[], component: Pos[]) {
  const blockIds = new Set<string>();
  for (const pos of component) {
    const connector = blockAt(blocks, pos);
    if (connector) {
      blockIds.add(connector.id);
    }
    for (const dir of allDirections()) {
      const neighbor = blockAt(blocks, step(pos, dir));
      if (neighbor && isNetworkNode(neighbor.kind)) {
        blockIds.add(neighbor.id);
      }
    }
  }
  return [...blockIds].sort();
}

function dockedDroneCountInNetwork(blockIds: string[]) {
  return state.drones.filter((drone) => drone.state === "docked" && blockIds.includes(drone.home_port)).length;
}

function runEnemies() {
  for (const enemy of state.enemies) {
    if (enemy.hp <= 0) continue;
    const target = nearestEnemyTarget(enemy);
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

function cleanupDestroyed() {
  const destroyedBlockIds = new Set(
    state.blocks.filter((block) => block.kind !== "core" && block.hp <= 0).map((block) => block.id)
  );
  if (destroyedBlockIds.size > 0) {
    for (const id of destroyedBlockIds) {
      log("warn", id, "block destroyed");
    }
    state.blocks = state.blocks.filter((block) => !destroyedBlockIds.has(block.id));
    if (state.selectedId && destroyedBlockIds.has(state.selectedId)) {
      state.selectedId = null;
    }
  }

  const core = state.blocks.find((block) => block.kind === "core");
  if (core && core.hp <= 0) {
    core.hp = 0;
    if (core.status !== "core breached") {
      core.status = "core breached";
      log("error", core.id, "core destroyed; simulation halted");
    }
    state.running = false;
  }
}

function nearestEnemyTarget(enemy: Enemy) {
  const targetKinds =
    enemy.kind === "wire_cutter" ? new Set<BlockKind>(["wire", "cpu_node", "drone_port"]) : new Set<BlockKind>(["core"]);
  return nearestBlockTarget(enemy.pos, (kind) => targetKinds.has(kind)) ?? nearestBlockTarget(enemy.pos, (kind) => kind === "core");
}

function nearestBlockTarget(origin: Pos, predicate: (kind: BlockKind) => boolean) {
  return state.blocks
    .filter((block) => predicate(block.kind))
    .map((block) => ({ block, pos: closestPointOnBlock(origin, block) }))
    .sort((a, b) => distance(origin, a.pos) - distance(origin, b.pos))[0];
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

function outputBlocked(block: Block) {
  const dst = blockAt(state.blocks, step(block.pos, block.dir));
  return !dst || !canAcceptItem(dst.kind, "ore") || inventoryTotal(dst.inventory) >= dst.inventory.capacity;
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

function closestPointOnBlock(origin: Pos, block: Block): Pos {
  const [width, height] = blockFootprintSize(block.kind);
  return {
    x: clamp(origin.x, block.pos.x, block.pos.x + width),
    y: clamp(origin.y, block.pos.y, block.pos.y + height)
  };
}

function distance(a: Pos, b: Pos) {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  return Math.hypot(dx, dy);
}

function moveToward(origin: Pos, target: Pos, maxDistance: number): Pos {
  const dx = target.x - origin.x;
  const dy = target.y - origin.y;
  const currentDistance = Math.hypot(dx, dy);
  if (currentDistance <= maxDistance || currentDistance === 0) {
    return { ...target };
  }
  const scale = maxDistance / currentDistance;
  return {
    x: origin.x + dx * scale,
    y: origin.y + dy * scale
  };
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
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

function allDirections(): Direction[] {
  return ["north", "east", "south", "west"];
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

function posKey(pos: Pos) {
  return `${pos.x},${pos.y}`;
}

function parsePosKey(key: string): Pos {
  const [x, y] = key.split(",").map(Number);
  return { x, y };
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
