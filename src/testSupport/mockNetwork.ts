import { blockLocalCpuRate, blockNetworkCpuOutput, isNetworkConnector, isNetworkNode } from "../gameMetadata";
import type { Block, Drone, Network, Pos } from "../types";
import { allDirections, blockAt, footprintPositions, parsePosKey, posKey, step } from "./mockGeometry";

export function recomputeNetworks(blocks: Block[], drones: Drone[]): Network[] {
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
      dockedDroneCountInNetwork(drones, blockIds);
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

function dockedDroneCountInNetwork(drones: Drone[], blockIds: string[]) {
  return drones.filter((drone) => drone.state === "docked" && blockIds.includes(drone.home_port)).length;
}
