import type { BlockKind, ItemKind } from "./types";

type BlockMetadata = {
  maxHp: number;
  inventoryCapacity: number;
  footprint: readonly [number, number];
  localCpuRate: number;
  networkCpuOutput: number;
  programmable: boolean;
  networkNode: boolean;
  defaultBehaviorId: string | null;
  accepts: readonly ItemKind[] | "any" | "none";
};

export const DRILL_MINE_BASE_TICKS = 30;

export const BLOCK_METADATA: Record<BlockKind, BlockMetadata> = {
  core: {
    maxHp: 500,
    inventoryCapacity: 1000,
    footprint: [4, 4],
    localCpuRate: 0,
    networkCpuOutput: 120,
    programmable: false,
    networkNode: true,
    defaultBehaviorId: null,
    accepts: "any"
  },
  wire: {
    maxHp: 15,
    inventoryCapacity: 0,
    footprint: [1, 1],
    localCpuRate: 0,
    networkCpuOutput: 0,
    programmable: false,
    networkNode: true,
    defaultBehaviorId: null,
    accepts: "none"
  },
  cpu_node: {
    maxHp: 90,
    inventoryCapacity: 0,
    footprint: [1, 1],
    localCpuRate: 0,
    networkCpuOutput: 80,
    programmable: false,
    networkNode: true,
    defaultBehaviorId: null,
    accepts: "none"
  },
  drill: {
    maxHp: 90,
    inventoryCapacity: 10,
    footprint: [1, 1],
    localCpuRate: 1,
    networkCpuOutput: 0,
    programmable: true,
    networkNode: true,
    defaultBehaviorId: "builtin.drill.basic",
    accepts: "none"
  },
  conveyor: {
    maxHp: 90,
    inventoryCapacity: 1,
    footprint: [1, 1],
    localCpuRate: 0,
    networkCpuOutput: 0,
    programmable: false,
    networkNode: false,
    defaultBehaviorId: null,
    accepts: "any"
  },
  router: {
    maxHp: 90,
    inventoryCapacity: 1,
    footprint: [1, 1],
    localCpuRate: 1,
    networkCpuOutput: 0,
    programmable: true,
    networkNode: true,
    defaultBehaviorId: "builtin.router.basic",
    accepts: "any"
  },
  storage: {
    maxHp: 90,
    inventoryCapacity: 300,
    footprint: [1, 1],
    localCpuRate: 0,
    networkCpuOutput: 0,
    programmable: false,
    networkNode: true,
    defaultBehaviorId: null,
    accepts: "any"
  },
  assembler: {
    maxHp: 90,
    inventoryCapacity: 100,
    footprint: [1, 1],
    localCpuRate: 2,
    networkCpuOutput: 0,
    programmable: true,
    networkNode: true,
    defaultBehaviorId: "builtin.assembler.basic",
    accepts: ["ore", "plate"]
  },
  turret: {
    maxHp: 90,
    inventoryCapacity: 80,
    footprint: [1, 1],
    localCpuRate: 3,
    networkCpuOutput: 0,
    programmable: true,
    networkNode: true,
    defaultBehaviorId: "builtin.turret.basic",
    accepts: ["ammo"]
  },
  drone_port: {
    maxHp: 90,
    inventoryCapacity: 120,
    footprint: [1, 1],
    localCpuRate: 3,
    networkCpuOutput: 20,
    programmable: true,
    networkNode: true,
    defaultBehaviorId: "builtin.drone_port.basic",
    accepts: "any"
  }
};

export function blockMaxHp(kind: BlockKind) {
  return BLOCK_METADATA[kind].maxHp;
}

export function blockInventoryCapacity(kind: BlockKind) {
  return BLOCK_METADATA[kind].inventoryCapacity;
}

export function blockFootprintSize(kind: BlockKind): [number, number] {
  return [...BLOCK_METADATA[kind].footprint];
}

export function blockDefaultBehaviorId(kind: BlockKind) {
  return BLOCK_METADATA[kind].defaultBehaviorId;
}

export function isProgrammableBlock(kind: BlockKind) {
  return BLOCK_METADATA[kind].programmable;
}

export function isNetworkNode(kind: BlockKind) {
  return BLOCK_METADATA[kind].networkNode;
}

export function blockLocalCpuRate(kind: BlockKind) {
  return BLOCK_METADATA[kind].localCpuRate;
}

export function blockNetworkCpuOutput(kind: BlockKind) {
  return BLOCK_METADATA[kind].networkCpuOutput;
}

export function canAcceptItem(kind: BlockKind, item: ItemKind) {
  const accepts = BLOCK_METADATA[kind].accepts;
  if (accepts === "any") return true;
  if (accepts === "none") return false;
  return accepts.includes(item);
}

export function displayBlockKind(kind: BlockKind) {
  return kind
    .split("_")
    .map((part) => part[0].toUpperCase() + part.slice(1))
    .join("");
}
