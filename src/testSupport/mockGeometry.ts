import { blockFootprintSize } from "../gameMetadata";
import type { Block, BlockKind, Direction, Pos } from "../types";

export function blockAt(blocks: Block[], pos: Pos) {
  return blocks.find((block) => footprintPositions(block.kind, block.pos).some((tile) => tile.x === pos.x && tile.y === pos.y));
}

export function blockCenter(block: Block): Pos {
  const [width, height] = blockFootprintSize(block.kind);
  return {
    x: block.pos.x + width / 2,
    y: block.pos.y + height / 2
  };
}

export function closestPointOnBlock(origin: Pos, block: Block): Pos {
  const [width, height] = blockFootprintSize(block.kind);
  return {
    x: clamp(origin.x, block.pos.x, block.pos.x + width),
    y: clamp(origin.y, block.pos.y, block.pos.y + height)
  };
}

export function distance(a: Pos, b: Pos) {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  return Math.hypot(dx, dy);
}

export function moveToward(origin: Pos, target: Pos, maxDistance: number): Pos {
  const dx = target.x - origin.x;
  const dy = target.y - origin.y;
  const currentDistance = Math.hypot(dx, dy);
  if (currentDistance <= maxDistance || currentDistance === 0) {
    return { ...target };
  }
  const scale = maxDistance / currentDistance;
  return {
    x: origin.x + dx * scale,
    y: origin.y + dy * scale
  };
}

export function footprintPositions(kind: BlockKind, pos: Pos) {
  const [width, height] = blockFootprintSize(kind);
  const positions: Pos[] = [];
  for (let y = pos.y; y < pos.y + height; y += 1) {
    for (let x = pos.x; x < pos.x + width; x += 1) {
      positions.push({ x, y });
    }
  }
  return positions;
}

export function allDirections(): Direction[] {
  return ["north", "east", "south", "west"];
}

export function step(pos: Pos, dir: Direction): Pos {
  const delta: Record<Direction, Pos> = {
    north: { x: 0, y: -1 },
    east: { x: 1, y: 0 },
    south: { x: 0, y: 1 },
    west: { x: -1, y: 0 }
  };
  return { x: pos.x + delta[dir].x, y: pos.y + delta[dir].y };
}

export function posKey(pos: Pos) {
  return `${pos.x},${pos.y}`;
}

export function parsePosKey(key: string): Pos {
  const [x, y] = key.split(",").map(Number);
  return { x, y };
}

export function rotateDirection(dir: Direction): Direction {
  const next: Record<Direction, Direction> = {
    north: "east",
    east: "south",
    south: "west",
    west: "north"
  };
  return next[dir];
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}
