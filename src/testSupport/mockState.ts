import {
  blockDefaultBehaviorId,
  blockInventoryCapacity,
  blockLocalCpuRate,
  blockMaxHp,
  isNetworkNode,
  isProgrammableBlock
} from "../gameMetadata";
import { BUILTIN_BEHAVIOR_PRESETS } from "../builtinBehaviors";
import { detectBehaviorSourceLanguage } from "../behaviorLanguage";
import type {
  BehaviorRuntimeStats,
  BehaviorSource,
  BehaviorSummary,
  Block,
  BlockKind,
  DeliveryJob,
  Direction,
  Drone,
  Enemy,
  ItemFlowEvent,
  LogEntry,
  Pos
} from "../types";

export type CommandCall = {
  cmd: string;
  args: unknown;
};

export type MutableBehavior = BehaviorSource;
export type IdKind = BlockKind | "behavior" | "drone" | "enemy" | "flow" | "job";
export type RuntimeEntity = { behavior_runtime: BehaviorRuntimeStats | null };
export type BehaviorOwner =
  | { kind: "block"; entity: Block; behaviorKind: BehaviorSummary["base_kind"] }
  | { kind: "drone"; entity: Drone; behaviorKind: "carrier_drone" };

export interface MockState {
  tick: number;
  running: boolean;
  blocks: Block[];
  enemies: Enemy[];
  drones: Drone[];
  pendingJobs: DeliveryJob[];
  itemFlows: ItemFlowEvent[];
  logs: LogEntry[];
  selectedId: string | null;
  behaviors: Record<string, MutableBehavior>;
  idCounters: Partial<Record<IdKind, number>>;
  calls: CommandCall[];
  saves: Record<string, SavedMockState>;
}

export type SavedMockState = Omit<MockState, "calls" | "saves">;

export function createInitialMockState(): MockState {
  const core = createMockBlock("core", { x: 30, y: 30 }, "east", "core_1");
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
    enemies: [],
    drones: [],
    pendingJobs: [],
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
    behaviors: builtinMockBehaviors(),
    idCounters: {
      core: 1
    },
    calls: [],
    saves: {}
  };
}

export function createMockBlock(kind: BlockKind, pos: Pos, dir: Direction, id: string): Block {
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

function builtinMockBehaviors(): Record<string, MutableBehavior> {
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

export function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}
