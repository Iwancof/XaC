import { mockIPC } from "@tauri-apps/api/mocks";
import type {
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

const DRILL_SOURCE = `(module
  (func $spin (param $n i32)
    (local $i i32)
    (local.set $i (i32.const 0))
    (block $exit
      (loop $loop
        (br_if $exit (i32.ge_s (local.get $i) (local.get $n)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop))))
  (func (export "tick") (result i32)
    (i32.const 1)))
`;

type CommandCall = {
  cmd: string;
  args: unknown;
};

type MutableBehavior = BehaviorSource;

interface MockState {
  tick: number;
  running: boolean;
  blocks: Block[];
  logs: LogEntry[];
  selectedId: string | null;
  behaviors: Record<string, MutableBehavior>;
  idCounters: Partial<Record<BlockKind | "behavior", number>>;
  calls: CommandCall[];
}

declare global {
  interface Window {
    __XAC_TEST_STATE__?: {
      calls: CommandCall[];
      reset: () => void;
      snapshot: () => GameSnapshot;
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
      state.running = Boolean((args as { running?: boolean }).running);
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
    case "select_entity":
      state.selectedId = (args as { id?: string | null }).id ?? null;
      return snapshot();
    case "open_behavior":
      return openBehavior((args as { behaviorId?: string }).behaviorId ?? "");
    case "edit_builtin_copy":
      return copyBehavior((args as { blockId?: string }).blockId ?? "", false);
    case "fork_behavior":
      return copyBehavior((args as { blockId?: string }).blockId ?? "", true);
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
  snapshot
};

function createInitialState(): MockState {
  const core = makeBlock("core", { x: 31, y: 32 }, "east", "core_1");
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

  return clone({
    tick: state.tick,
    running: state.running,
    width: MAP_WIDTH,
    height: MAP_HEIGHT,
    tiles,
    blocks,
    enemies: [],
    drones: [],
    networks,
    logs: state.logs.slice(-160),
    selected_id: state.selectedId,
    behaviors: behaviorSummaries(),
    pending_jobs: []
  });
}

function placeBlock({ kind, x, y, dir }: { kind: BlockKind; x: number; y: number; dir: Direction }) {
  const pos = { x, y };
  if (!inBounds(pos)) {
    throw new Error("position is outside the map");
  }
  if (state.blocks.some((block) => block.pos.x === x && block.pos.y === y)) {
    throw new Error("tile is not buildable or is already occupied");
  }

  const block = makeBlock(kind, pos, dir);
  state.blocks.push(block);
  state.selectedId = block.id;
  log("info", block.id, `placed ${displayKind(kind)} at ${x},${y}`);
  return snapshot();
}

function runTicks(count: number) {
  for (let i = 0; i < Math.max(0, Math.min(500, count)); i += 1) {
    state.tick += 1;

    for (const block of state.blocks) {
      if (block.kind !== "drill" || terrainAt(block.pos) !== "ore_patch") continue;
      block.progress += 1;
      if (block.progress >= 30 && inventoryCount(block.inventory, "ore") < block.inventory.capacity) {
        block.progress = 0;
        addItem(block.inventory, "ore", 1);
        block.status = "mined ore";
        log("info", block.id, "mined ore");
      }
    }
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

function copyBehavior(blockId: string, fork: boolean): BehaviorSource {
  const block = state.blocks.find((item) => item.id === blockId);
  if (!block?.behavior_ref) {
    throw new Error("selected block has no behavior");
  }

  const original = openBehavior(block.behavior_ref);
  if (!original.summary.builtin && !fork) {
    return original;
  }

  const id = makeId("behavior");
  const sourcePath = `projects/default_project/blocks/${id}/src/behavior.wat`;
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
  block.behavior_ref = id;
  log("info", block.id, `${fork ? "forked" : "created editable copy"} ${id}`);
  return openBehavior(id);
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
    hp: kind === "core" ? 500 : kind === "wire" ? 15 : 90,
    inventory: { items: {}, capacity: capacityFor(kind) },
    behavior_ref: defaultBehaviorFor(kind),
    tags: kind === "turret" ? ["frontline"] : [],
    active: isProgrammable(kind),
    network_id: isNetworkNode(kind) ? 1 : null,
    effective_cpu_rate: localCpuRate(kind),
    progress: 0,
    status: "idle"
  };
}

function builtinBehaviors(): Record<string, MutableBehavior> {
  return {
    "builtin.drill.basic": {
      summary: {
        id: "builtin.drill.basic",
        display_name: "Basic Drill",
        base_kind: "drill",
        world: "drill-behavior",
        builtin: true,
        used_by: 0,
        source_path: "assets/builtin/drill/basic.wat",
        build_status: "builtin"
      },
      source: DRILL_SOURCE
    }
  };
}

function behaviorSummaries(): BehaviorSummary[] {
  return Object.values(state.behaviors).map((behavior) => ({
    ...behavior.summary,
    used_by: usedBy(behavior.summary.id)
  }));
}

function recomputeNetworks(blocks: Block[]): Network[] {
  const blockIds = blocks.filter((block) => isNetworkNode(block.kind)).map((block) => block.id);
  const activeDevices = blocks.filter((block) => block.active).length;
  const cpuPool = blocks
    .filter((block) => isNetworkNode(block.kind))
    .reduce((sum, block) => sum + networkCpu(block.kind), 0);
  const effectivePerDevice = activeDevices ? cpuPool / activeDevices : 0;
  for (const block of blocks) {
    block.network_id = isNetworkNode(block.kind) ? 1 : null;
    block.effective_cpu_rate = block.active ? localCpuRate(block.kind) + effectivePerDevice : 0;
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
        block_id: blocks.find((block) => block.pos.x === x && block.pos.y === y)?.id ?? null
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

function defaultBehaviorFor(kind: BlockKind) {
  return kind === "drill" ? "builtin.drill.basic" : null;
}

function isProgrammable(kind: BlockKind) {
  return ["drill", "router", "assembler", "turret", "drone_port"].includes(kind);
}

function isNetworkNode(kind: BlockKind) {
  return kind !== "conveyor";
}

function localCpuRate(kind: BlockKind) {
  if (kind === "drill" || kind === "router") return 1;
  if (kind === "assembler") return 2;
  if (kind === "turret" || kind === "drone_port") return 3;
  return 0;
}

function networkCpu(kind: BlockKind) {
  if (kind === "core") return 120;
  if (kind === "cpu_node") return 80;
  if (kind === "drone_port") return 20;
  return 0;
}

function capacityFor(kind: BlockKind) {
  const capacities: Partial<Record<BlockKind, number>> = {
    core: 1000,
    storage: 300,
    conveyor: 1,
    router: 1,
    turret: 80,
    assembler: 100,
    drill: 10,
    drone_port: 120
  };
  return capacities[kind] ?? 0;
}

function displayKind(kind: BlockKind) {
  return kind
    .split("_")
    .map((part) => part[0].toUpperCase() + part.slice(1))
    .join("");
}

function usedBy(behaviorId: string) {
  return state.blocks.filter((block) => block.behavior_ref === behaviorId).length;
}

function makeId(kind: BlockKind | "behavior") {
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
