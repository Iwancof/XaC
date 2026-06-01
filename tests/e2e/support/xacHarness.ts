import type { Locator, Page } from "@playwright/test";

export type IpcCall = {
  cmd: string;
  args: Record<string, unknown>;
};

type TestEnemyKind = "grunt" | "runner" | "armored" | "wire_cutter";

declare global {
  interface Window {
    __XAC_TEST_STATE__?: {
      calls: IpcCall[];
      snapshot: () => {
        behaviors: Array<{ id: string; source_path: string }>;
        blocks: Array<{
          id: string;
          kind: string;
          behavior_ref: string | null;
          hp: number;
          inventory: { items: Partial<Record<string, number>> };
          network_id: number | null;
          effective_cpu_rate: number;
          target_id: string | null;
        }>;
        drones: Array<{ id: string; behavior_ref: string | null; pos: { x: number; y: number } }>;
        enemies: Array<{ id: string; hp: number; pos: { x: number; y: number }; target_id: string | null }>;
        item_flows: Array<{ item: string; amount: number; from_entity: string; to_entity: string }>;
        pending_jobs: Array<{ id: string; item: string; amount: number; pickup: string; dropoff: string }>;
        networks: Array<{
          cpu_pool: number;
          active_devices: number;
          effective_per_device: number;
          block_ids: string[];
          read_only_cache: boolean;
        }>;
        status: {
          wire_threats: number;
          damaged_wires: number;
          network_cpu: number;
        };
        selected_id: string | null;
      };
      spawnCarrierDrone: (homePortId?: string) => string;
      spawnEnemy: (kind: TestEnemyKind, pos: { x: number; y: number }) => string;
      setBlockInventory: (blockId: string, items: Partial<Record<string, number>>) => void;
      forceOverBudget: (entityId: string) => void;
      forceRuntimeError: (entityId: string, message?: string) => void;
    };
    __XAC_EDITOR__?: {
      getValue: () => string;
      setValue: (value: string) => void;
    };
  }
}

export const tileCenter = (x: number, y: number) => ({
  x: x * 16 + 8,
  y: y * 16 + 8
});

export async function dragTiles(
  page: Page,
  canvas: Locator,
  from: { x: number; y: number },
  to: { x: number; y: number }
) {
  const box = await canvas.boundingBox();
  if (!box) {
    throw new Error("grid canvas should have a bounding box");
  }
  await page.mouse.move(box.x + from.x, box.y + from.y);
  await page.mouse.down();
  await page.mouse.move(box.x + to.x, box.y + to.y, { steps: 2 });
  await page.mouse.up();
}
