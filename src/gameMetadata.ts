import type { BlockKind, ItemKind } from "./types";
import blockMetadata from "../assets/block_metadata.json";

type BlockMetadata = {
  maxHp: number;
  inventoryCapacity: number;
  footprint: readonly [number, number];
  localCpuRate: number;
  networkCpuOutput: number;
  programmable: boolean;
  networkNode: boolean;
  networkConnector: boolean;
  defaultBehaviorId: string | null;
  accepts: readonly ItemKind[] | "any" | "none";
};

export const DRILL_MINE_BASE_TICKS = 30;

export const BLOCK_METADATA = blockMetadata as unknown as Record<BlockKind, BlockMetadata>;

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
