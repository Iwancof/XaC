import { canAcceptItem } from "../gameMetadata";
import type { Block, DeliveryJob, Drone, ItemKind, LogLevel, Pos } from "../types";
import type { MockBehaviorResult } from "./mockBehaviorRuntime";
import { blockCenter, distance, moveToward } from "./mockGeometry";
import { addItem, inventoryCount, inventoryFree, removeItem } from "./mockInventory";
import { networkBlocks } from "./mockLogistics";

export interface MockDroneContext {
  blocks: Block[];
  drones: Drone[];
  pendingJobs: DeliveryJob[];
  createId: (kind: "drone" | "job") => string;
  log: (level: LogLevel, source: string, message: string) => void;
  recordItemFlow: (fromEntity: string, toEntity: string, item: ItemKind, amount: number, from: Pos, to: Pos) => void;
}

export function spawnCarrierDrone(context: MockDroneContext, homePortId?: string) {
  const port = homePortId
    ? context.blocks.find((block) => block.id === homePortId)
    : context.blocks.find((block) => block.kind === "drone_port");
  if (!port) {
    throw new Error(`unknown drone port: ${homePortId ?? "first drone_port"}`);
  }
  if (port.kind !== "drone_port") {
    throw new Error(`block ${port.id} is not a drone port`);
  }

  const id = createCarrierDrone(context, port);
  context.log("info", id, `carrier drone docked at ${port.id}`);
  return id;
}

export function runMockDronePort(
  context: MockDroneContext,
  block: Block,
  command: NonNullable<MockBehaviorResult["dronePort"]>
) {
  if (block.kind !== "drone_port") return;
  if (command.charge) {
    for (const drone of context.drones.filter((candidate) => candidate.home_port === block.id && droneAtHome(context, candidate))) {
      drone.battery = Math.min(100, drone.battery + 2);
      drone.logic_fuel = Math.min(1000, drone.logic_fuel + 10);
      drone.state = "docked";
    }
  }
  for (const job of command.createJobs) {
    createMockDeliveryJob(context, block, job.item, job.amount, job.dropoffTag);
  }
  if (command.dispatch) {
    dispatchMockIdleDrones(context, block);
  }
}

export function runMockDrones(context: MockDroneContext) {
  for (const drone of context.drones) {
    if (drone.job) {
      continueMockDroneDelivery(context, drone);
    } else if (!droneAtHome(context, drone)) {
      moveDroneTowardHome(context, drone);
    } else {
      drone.state = "docked";
    }
  }
}

export function dockedDroneCountAtPort(context: MockDroneContext, portId: string) {
  return context.drones.filter((drone) => drone.home_port === portId && droneAtHome(context, drone)).length;
}

function createCarrierDrone(context: MockDroneContext, port: Block) {
  const existing = context.drones.find((drone) => drone.home_port === port.id);
  if (existing) return existing.id;

  const id = context.createId("drone");
  context.drones.push({
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
  return id;
}

function createMockDeliveryJob(
  context: MockDroneContext,
  port: Block,
  item: ItemKind,
  amount: number,
  dropoffTag: string
) {
  const dropoff = context.blocks.find(
    (block) =>
      block.tags.includes(dropoffTag) &&
      canAcceptItem(block.kind, item) &&
      inventoryCount(block.inventory, item) < amount &&
      inventoryFree(block.inventory) >= amount
  );
  if (!dropoff) return false;
  if (context.pendingJobs.some((job) => job.dropoff === dropoff.id && job.item === item)) return false;
  if (context.drones.some((drone) => drone.job?.dropoff === dropoff.id && drone.job.item === item)) return false;

  const pickup = networkBlocks(context.blocks, port).find(
    (block) =>
      ["core", "storage", "assembler", "drone_port"].includes(block.kind) &&
      inventoryCount(block.inventory, item) >= amount
  );
  if (!pickup) return false;

  context.pendingJobs.push({
    id: context.createId("job"),
    item,
    amount,
    pickup: pickup.id,
    dropoff: dropoff.id,
    priority: 50
  });
  port.status = `queued ${item} delivery`;
  return true;
}

function dispatchMockIdleDrones(context: MockDroneContext, port: Block) {
  createCarrierDrone(context, port);
  for (const drone of context.drones.filter((candidate) => candidate.home_port === port.id && !candidate.job && droneAtHome(context, candidate))) {
    const job = context.pendingJobs.shift();
    if (!job) return;
    drone.job = job;
    drone.state = "delivering";
    drone.behavior_ref ??= "builtin.carrier_drone.basic";
    context.log("info", drone.id, `claimed ${job.item} delivery`);
  }
}

function continueMockDroneDelivery(context: MockDroneContext, drone: Drone) {
  const job = drone.job;
  if (!job) return;
  const carrying = inventoryCount(drone.cargo, job.item);
  if (carrying === 0) {
    const pickup = context.blocks.find((block) => block.id === job.pickup);
    if (!pickup) {
      drone.job = null;
      drone.state = "returning";
      return;
    }
    if (moveDroneTowardBlock(drone, pickup)) {
      const loaded = Math.min(job.amount, inventoryCount(pickup.inventory, job.item), inventoryFree(drone.cargo));
      if (loaded > 0) {
        removeItem(pickup.inventory, job.item, loaded);
        addItem(drone.cargo, job.item, loaded);
        pickup.status = `loaded ${job.item}`;
      }
    }
    return;
  }

  const dropoff = context.blocks.find((block) => block.id === job.dropoff);
  if (!dropoff) {
    drone.job = null;
    drone.state = "returning";
    return;
  }
  if (moveDroneTowardBlock(drone, dropoff)) {
    const unloaded = Math.min(carrying, inventoryFree(dropoff.inventory));
    if (unloaded > 0) {
      removeItem(drone.cargo, job.item, unloaded);
      addItem(dropoff.inventory, job.item, unloaded);
      context.recordItemFlow(drone.id, dropoff.id, job.item, unloaded, { ...drone.pos }, blockCenter(dropoff));
      dropoff.status = `received ${job.item}`;
    }
    if (inventoryCount(drone.cargo, job.item) === 0) {
      drone.job = null;
      drone.state = "returning";
    }
  }
}

function moveDroneTowardBlock(drone: Drone, block: Block) {
  const target = blockCenter(block);
  drone.state = "delivering";
  drone.pos = moveToward(drone.pos, target, 0.18);
  drone.battery = Math.max(0, drone.battery - 0.02);
  return distance(drone.pos, target) <= 0.2;
}

function moveDroneTowardHome(context: MockDroneContext, drone: Drone) {
  const home = context.blocks.find((block) => block.id === drone.home_port);
  if (!home) {
    drone.state = "offline";
    return;
  }
  drone.state = "returning";
  drone.pos = moveToward(drone.pos, blockCenter(home), 0.18);
  if (droneAtHome(context, drone)) {
    drone.state = "docked";
  }
}

function droneAtHome(context: MockDroneContext, drone: Drone) {
  const home = context.blocks.find((block) => block.id === drone.home_port);
  return Boolean(home && distance(drone.pos, blockCenter(home)) <= 0.2);
}
