import { canAcceptItem } from "../gameMetadata";
import type { Block, Direction, ItemKind, Pos } from "../types";
import { blockAt, blockCenter, step } from "./mockGeometry";
import { addItem, inventoryCount, inventoryTotal } from "./mockInventory";

export type MockLogisticsContext = {
  blocks: Block[];
  recordItemFlow: (fromEntity: string, toEntity: string, item: ItemKind, amount: number, from: Pos, to: Pos) => void;
};

export function transferFrom(
  context: MockLogisticsContext,
  block: Block,
  dir = block.dir,
  itemFilter: ItemKind | null = null
) {
  const item = Object.entries(block.inventory.items).find(
    ([kind, amount]) => (amount ?? 0) > 0 && (!itemFilter || kind === itemFilter)
  );
  if (!item) return false;
  const [kind, amount] = item as [ItemKind, number];
  const dst = blockAt(context.blocks, step(block.pos, dir));
  if (!dst || !canAcceptItem(dst.kind, kind) || inventoryTotal(dst.inventory) >= dst.inventory.capacity) return false;
  block.inventory.items[kind] = Math.max(0, amount - 1);
  if (block.inventory.items[kind] === 0) delete block.inventory.items[kind];
  addItem(dst.inventory, kind, 1);
  block.status = `sent ${kind}`;
  dst.status = `received ${kind}`;
  context.recordItemFlow(block.id, dst.id, kind, 1, blockCenter(block), blockCenter(dst));
  return true;
}

export function outputBlocked(context: MockLogisticsContext, block: Block) {
  const dst = blockAt(context.blocks, step(block.pos, block.dir));
  return !dst || !canAcceptItem(dst.kind, "ore") || inventoryTotal(dst.inventory) >= dst.inventory.capacity;
}

export function outputAvailable(
  context: MockLogisticsContext,
  block: Block,
  dir: Direction,
  itemFilter: ItemKind | null = null
) {
  const item = Object.entries(block.inventory.items).find(
    ([kind, amount]) => (amount ?? 0) > 0 && (!itemFilter || kind === itemFilter)
  )?.[0] as ItemKind | undefined;
  if (!item) return false;
  const dst = blockAt(context.blocks, step(block.pos, dir));
  return Boolean(dst && canAcceptItem(dst.kind, item) && inventoryTotal(dst.inventory) < dst.inventory.capacity);
}

export function networkStockCount(blocks: Block[], block: Block, item: ItemKind) {
  return networkBlocks(blocks, block).reduce((sum, candidate) => sum + inventoryCount(candidate.inventory, item), 0);
}

export function networkBlocks(blocks: Block[], block: Block) {
  if (block.network_id === null) return [block];
  return blocks.filter((candidate) => candidate.network_id === block.network_id);
}
