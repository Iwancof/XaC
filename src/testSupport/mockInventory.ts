import type { Inventory, ItemKind } from "../types";

export function inventoryCount(inventory: Inventory, item: ItemKind) {
  return inventory.items[item] ?? 0;
}

export function inventoryTotal(inventory: Inventory) {
  return Object.values(inventory.items).reduce((sum, amount) => sum + (amount ?? 0), 0);
}

export function inventoryFree(inventory: Inventory) {
  return inventory.capacity - inventoryTotal(inventory);
}

export function addItem(inventory: Inventory, item: ItemKind, amount: number) {
  inventory.items[item] = inventoryCount(inventory, item) + amount;
}

export function removeItem(inventory: Inventory, item: ItemKind, amount: number) {
  const next = Math.max(0, inventoryCount(inventory, item) - amount);
  if (next === 0) {
    delete inventory.items[item];
  } else {
    inventory.items[item] = next;
  }
}
