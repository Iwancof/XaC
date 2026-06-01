import resourcesSource from "../assets/resources.toml?raw";
import type { ItemKind } from "./types";

export interface ItemMetadata {
  id: ItemKind;
  displayName: string;
  stackSize: number;
  color: number;
}

const ITEM_ORDER: readonly ItemKind[] = ["ore", "plate", "ammo", "cpu_part", "drone_part"];

const ITEM_COLORS: Record<ItemKind, number> = {
  ore: 0xd8a94a,
  plate: 0xb9c2cf,
  ammo: 0xf43f5e,
  cpu_part: 0x36d399,
  drone_part: 0x38bdf8
};

export const ITEM_METADATA: Record<ItemKind, ItemMetadata> = parseResources(resourcesSource);

export function displayItemKind(kind: ItemKind) {
  return ITEM_METADATA[kind].displayName;
}

export function itemStackSize(kind: ItemKind) {
  return ITEM_METADATA[kind].stackSize;
}

export function itemColor(kind: ItemKind) {
  return ITEM_METADATA[kind].color;
}

function parseResources(source: string): Record<ItemKind, ItemMetadata> {
  const entries: Partial<Record<ItemKind, ItemMetadata>> = {};
  let current: Partial<Omit<ItemMetadata, "color">> | null = null;

  const finishCurrent = () => {
    if (!current) return;
    if (!current.id || !current.displayName || !current.stackSize) {
      throw new Error("resources.toml contains an incomplete item entry");
    }
    entries[current.id] = {
      id: current.id,
      displayName: current.displayName,
      stackSize: current.stackSize,
      color: ITEM_COLORS[current.id]
    };
  };

  for (const rawLine of source.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    if (line === "[[item]]") {
      finishCurrent();
      current = {};
      continue;
    }

    const match = /^([a-z_]+)\s*=\s*(.+)$/.exec(line);
    if (!match || !current) continue;
    const [, key, rawValue] = match;
    if (key === "id") {
      const id = parseTomlString(rawValue);
      if (!isItemKind(id)) throw new Error(`resources.toml has unknown item id ${id}`);
      current.id = id;
    } else if (key === "display_name") {
      current.displayName = parseTomlString(rawValue);
    } else if (key === "stack_size") {
      current.stackSize = Number.parseInt(rawValue, 10);
    }
  }
  finishCurrent();

  const metadata = {} as Record<ItemKind, ItemMetadata>;
  for (const id of ITEM_ORDER) {
    const entry = entries[id];
    if (!entry) throw new Error(`resources.toml is missing item ${id}`);
    metadata[id] = entry;
  }
  return metadata;
}

function parseTomlString(value: string) {
  const trimmed = value.trim();
  const match = /^"([^"]*)"$/.exec(trimmed);
  if (!match) throw new Error(`expected TOML string, got ${trimmed}`);
  return match[1];
}

function isItemKind(value: string): value is ItemKind {
  return ITEM_ORDER.includes(value as ItemKind);
}
