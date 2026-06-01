import { Application, Container, Graphics, Text } from "pixi.js";
import { useEffect, useRef } from "react";
import { enemyColor } from "./enemyMetadata";
import { blockAttackRangeTiles, blockFootprintSize } from "./gameMetadata";
import { itemColor } from "./itemMetadata";
import type { Overlay } from "./overlays";
import type { Block, BlockKind, Direction, Enemy, GameSnapshot, Pos } from "./types";

interface GridWorldProps {
  snapshot: GameSnapshot | null;
  selectedId: string | null;
  buildKind: BlockKind | null;
  direction: Direction;
  overlay: Overlay;
  onTileClick: (pos: Pos) => void;
  onTilesPaint: (positions: Pos[], direction: Direction) => void;
  onPaintDirectionChange: (direction: Direction) => void;
  onEntityClick: (id: string | null) => void;
}

const TILE = 16;
const COLORS: Record<BlockKind, number> = {
  core: 0xf5c542,
  wire: 0x7ca7ff,
  cpu_node: 0x36d399,
  drill: 0xb67945,
  conveyor: 0x98a2b3,
  router: 0x2dd4bf,
  storage: 0xf59e0b,
  assembler: 0xa78bfa,
  turret: 0xf43f5e,
  drone_port: 0x38bdf8
};

export function GridWorld({
  snapshot,
  selectedId,
  buildKind,
  direction,
  overlay,
  onTileClick,
  onTilesPaint,
  onPaintDirectionChange,
  onEntityClick
}: GridWorldProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const appRef = useRef<Application | null>(null);
  const stageRef = useRef<Container | null>(null);
  const snapshotRef = useRef<GameSnapshot | null>(null);
  const snapshotFrameBaseMsRef = useRef(0);
  const selectedIdRef = useRef<string | null>(selectedId);
  const buildKindRef = useRef<BlockKind | null>(buildKind);
  const directionRef = useRef<Direction>(direction);
  const overlayRef = useRef<Overlay>(overlay);
  const onTileClickRef = useRef(onTileClick);
  const onTilesPaintRef = useRef(onTilesPaint);
  const onPaintDirectionChangeRef = useRef(onPaintDirectionChange);
  const onEntityClickRef = useRef(onEntityClick);
  const paintRef = useRef<PaintState | null>(null);
  const placementPreviewRef = useRef<PlacementPreview | null>(null);

  useEffect(() => {
    selectedIdRef.current = selectedId;
    buildKindRef.current = buildKind;
    directionRef.current = direction;
    overlayRef.current = overlay;
    if (!buildKind) {
      paintRef.current = null;
      placementPreviewRef.current = null;
    }
    onTileClickRef.current = onTileClick;
    onTilesPaintRef.current = onTilesPaint;
    onPaintDirectionChangeRef.current = onPaintDirectionChange;
    onEntityClickRef.current = onEntityClick;
  }, [selectedId, buildKind, direction, overlay, onTileClick, onTilesPaint, onPaintDirectionChange, onEntityClick]);

  useEffect(() => {
    let disposed = false;
    let initialized = false;
    const app = new Application();
    appRef.current = app;

    app
      .init({
        background: "#101417",
        antialias: false,
        resolution: window.devicePixelRatio || 1,
        autoDensity: true,
        resizeTo: hostRef.current ?? undefined
      })
      .then(() => {
        initialized = true;
        if (disposed || !hostRef.current) {
          app.destroy(true, { children: true, texture: true });
          return;
        }
        hostRef.current.appendChild(app.canvas);
        const stage = new Container();
        stageRef.current = stage;
        app.stage.addChild(stage);
        app.stage.eventMode = "static";
        app.stage.hitArea = app.screen;
        app.stage.on("pointerdown", (event) => {
          const current = snapshotRef.current;
          const pointer = pointerWorld(event);
          const pos = tileFromWorld(pointer);
          if (!current || !inSnapshotBounds(current, pos)) {
            onEntityClickRef.current(null);
            return;
          }
          if (buildKindRef.current) {
            paintRef.current = makePaintState(pos);
            placementPreviewRef.current = makePlacementPreview(buildKindRef.current, directionRef.current, [pos], true);
            renderWorld(
              stage,
              snapshotRef.current,
              selectedIdRef.current,
              buildKindRef.current,
              directionRef.current,
              overlayRef.current,
              placementPreviewRef.current,
              performance.now()
            );
            return;
          }
          const worldTile = {
            x: pointer.x / TILE,
            y: pointer.y / TILE
          };
          const tile = current.tiles.find((item) => item.pos.x === pos.x && item.pos.y === pos.y);
          const block = current.blocks.find((item) => item.id === tile?.block_id);
          const enemy = current.enemies.find((item) => distance(item.pos, worldTile) <= 0.55);
          const drone = current.drones.find((item) => distance(item.pos, worldTile) <= 0.6);
          if (enemy) {
            onEntityClickRef.current(enemy.id);
          } else if (drone) {
            onEntityClickRef.current(drone.id);
          } else if (block) {
            onEntityClickRef.current(block.id);
          } else {
            onEntityClickRef.current(null);
          }
        });
        app.stage.on("pointermove", (event) => {
          const paint = paintRef.current;
          const current = snapshotRef.current;
          const currentBuildKind = buildKindRef.current;
          if (!current || !currentBuildKind) return;
          const pos = tileFromWorld(pointerWorld(event));
          if (!inSnapshotBounds(current, pos)) return;
          if (!paint) {
            placementPreviewRef.current = makePlacementPreview(currentBuildKind, directionRef.current, [pos], false);
            renderWorld(
              stage,
              snapshotRef.current,
              selectedIdRef.current,
              currentBuildKind,
              directionRef.current,
              overlayRef.current,
              placementPreviewRef.current,
              performance.now()
            );
            return;
          }
          const nextDirection = addPaintTiles(paint, pos);
          if (nextDirection && nextDirection !== directionRef.current) {
            directionRef.current = nextDirection;
            onPaintDirectionChangeRef.current(nextDirection);
          }
          placementPreviewRef.current = makePlacementPreview(
            currentBuildKind,
            paint.direction ?? directionRef.current,
            paint.positions,
            true
          );
          renderWorld(
            stage,
            snapshotRef.current,
            selectedIdRef.current,
            currentBuildKind,
            directionRef.current,
            overlayRef.current,
            placementPreviewRef.current,
            performance.now()
          );
        });
        const finishPaint = () => {
          const paint = paintRef.current;
          paintRef.current = null;
          if (!paint || !buildKindRef.current) return;
          const paintDirection = paint.direction ?? directionRef.current;
          placementPreviewRef.current = null;
          if (paint.positions.length === 1) {
            onTileClickRef.current(paint.positions[0]);
          } else {
            onTilesPaintRef.current(paint.positions, paintDirection);
          }
        };
        app.stage.on("pointerup", finishPaint);
        app.stage.on("pointerupoutside", finishPaint);
        app.stage.on("pointercancel", finishPaint);
        renderWorld(stage, snapshotRef.current, selectedId, buildKind, direction, overlay, placementPreviewRef.current, performance.now());
      });

    return () => {
      disposed = true;
      stageRef.current = null;
      if (initialized) {
        app.destroy(true, { children: true, texture: true });
      }
    };
  }, []);

  useEffect(() => {
    snapshotRef.current = snapshot;
    snapshotFrameBaseMsRef.current = performance.now();
    if (stageRef.current) {
      renderWorld(stageRef.current, snapshot, selectedId, buildKind, direction, overlay, placementPreviewRef.current, performance.now());
    }
  }, [snapshot, selectedId, buildKind, direction, overlay]);

  useEffect(() => {
    let frame = 0;
    const animate = (now: number) => {
      if (
        stageRef.current &&
        snapshotRef.current &&
        hasAnimatedItemFlows(snapshotRef.current, now - snapshotFrameBaseMsRef.current)
      ) {
        renderWorld(
          stageRef.current,
          snapshotRef.current,
          selectedIdRef.current,
          buildKindRef.current,
          directionRef.current,
          overlayRef.current,
          placementPreviewRef.current,
          now
        );
      }
      frame = window.requestAnimationFrame(animate);
    };
    frame = window.requestAnimationFrame(animate);
    return () => window.cancelAnimationFrame(frame);
  }, []);

  return <div ref={hostRef} className="grid-world" data-testid="grid-world" />;
}

function hasAnimatedItemFlows(snapshot: GameSnapshot, snapshotAgeMs: number) {
  return snapshotAgeMs < 2400 && snapshot.item_flows.some((flow) => snapshot.tick - flow.tick <= 48);
}

interface PaintState {
  positions: Pos[];
  seen: Set<string>;
  last: Pos;
  direction: Direction | null;
}

interface PlacementPreview {
  kind: BlockKind;
  direction: Direction;
  positions: Pos[];
  active: boolean;
}

function pointerWorld(event: { global: { x: number; y: number } }) {
  return event.global;
}

function tileFromWorld(world: Pos) {
  return {
    x: Math.floor(world.x / TILE),
    y: Math.floor(world.y / TILE)
  };
}

function inSnapshotBounds(snapshot: GameSnapshot, pos: Pos) {
  return pos.x >= 0 && pos.y >= 0 && pos.x < snapshot.width && pos.y < snapshot.height;
}

function makePaintState(pos: Pos): PaintState {
  return {
    positions: [pos],
    seen: new Set([posKey(pos)]),
    last: pos,
    direction: null
  };
}

function makePlacementPreview(kind: BlockKind, direction: Direction, positions: Pos[], active: boolean): PlacementPreview {
  return {
    kind,
    direction,
    positions: [...positions],
    active
  };
}

function addPaintTiles(paint: PaintState, pos: Pos) {
  const nextDirection = directionFromDelta(pos.x - paint.last.x, pos.y - paint.last.y);
  for (const tile of tileLine(paint.last, pos)) {
    const key = posKey(tile);
    if (!paint.seen.has(key)) {
      paint.positions.push(tile);
      paint.seen.add(key);
    }
  }
  paint.last = pos;
  if (nextDirection) paint.direction = nextDirection;
  return nextDirection;
}

function directionFromDelta(dx: number, dy: number): Direction | null {
  if (dx === 0 && dy === 0) return null;
  if (Math.abs(dx) >= Math.abs(dy)) {
    return dx >= 0 ? "east" : "west";
  }
  return dy >= 0 ? "south" : "north";
}

function tileLine(from: Pos, to: Pos) {
  const steps = Math.max(Math.abs(to.x - from.x), Math.abs(to.y - from.y));
  if (steps === 0) return [to];
  const tiles: Pos[] = [];
  for (let i = 1; i <= steps; i += 1) {
    tiles.push({
      x: Math.round(from.x + ((to.x - from.x) * i) / steps),
      y: Math.round(from.y + ((to.y - from.y) * i) / steps)
    });
  }
  return tiles;
}

function posKey(pos: Pos) {
  return `${pos.x},${pos.y}`;
}

function renderWorld(
  stage: Container,
  snapshot: GameSnapshot | null,
  selectedId: string | null,
  buildKind: BlockKind | null,
  direction: Direction,
  overlay: Overlay,
  placementPreview: PlacementPreview | null,
  renderTimeMs: number
) {
  stage.removeChildren();
  if (!snapshot) return;

  const graphics = new Graphics();
  drawTiles(graphics, snapshot);
  drawOverlays(graphics, snapshot, overlay);
  drawBlocks(graphics, snapshot.blocks, selectedId, renderTimeMs);
  drawItemFlows(graphics, snapshot, renderTimeMs);
  drawPlacementPreview(graphics, snapshot, placementPreview);
  drawEnemies(graphics, snapshot.enemies, selectedId);
  drawDrones(graphics, snapshot, selectedId);
  stage.addChild(graphics);
  addOverlayLabels(stage, snapshot, overlay);

  if (buildKind) {
    const ghost = new Text({
      text: `Placing ${label(buildKind)} ${arrow(direction)}`,
      style: {
        fill: "#d5e2e8",
        fontFamily: "Inter, system-ui, sans-serif",
        fontSize: 13
      }
    });
    ghost.x = 12;
    ghost.y = 10;
    stage.addChild(ghost);
  }
}

function addOverlayLabels(stage: Container, snapshot: GameSnapshot, overlay: Overlay) {
  if (overlay !== "cpu") return;
  for (const block of snapshot.blocks) {
    if (!block.active || block.effective_cpu_rate <= 0) continue;
    const [width] = blockFootprintSize(block.kind);
    const text = new Text({
      text: block.effective_cpu_rate.toFixed(0),
      style: {
        fill: "#dfffee",
        fontFamily: "Inter, system-ui, sans-serif",
        fontSize: 10,
        stroke: { color: "#0e1214", width: 3 }
      }
    });
    text.x = block.pos.x * TILE + Math.max(2, (width * TILE - text.width) / 2);
    text.y = block.pos.y * TILE + 2;
    stage.addChild(text);
  }
}

function drawTiles(g: Graphics, snapshot: GameSnapshot) {
  for (const tile of snapshot.tiles) {
    const x = tile.pos.x * TILE;
    const y = tile.pos.y * TILE;
    const fill = tile.terrain === "ore_patch" ? 0x40351e : 0x151b1f;
    g.rect(x, y, TILE, TILE).fill(fill);
    if (tile.terrain === "ore_patch") {
      g.circle(x + 8, y + 8, 2).fill(0xd8a94a);
    }
  }
  g.stroke({ width: 1, color: 0x253038, alpha: 0.45 });
  for (let x = 0; x <= snapshot.width; x++) {
    g.moveTo(x * TILE, 0);
    g.lineTo(x * TILE, snapshot.height * TILE);
  }
  for (let y = 0; y <= snapshot.height; y++) {
    g.moveTo(0, y * TILE);
    g.lineTo(snapshot.width * TILE, y * TILE);
  }
}

function drawOverlays(g: Graphics, snapshot: GameSnapshot, overlay: Overlay) {
  if (overlay === "network") {
    for (const network of snapshot.networks) {
      const color = network.read_only_cache ? 0xf97316 : networkColor(network.id);
      for (const id of network.block_ids) {
        const block = snapshot.blocks.find((item) => item.id === id);
        if (!block) continue;
        const [width, height] = blockFootprintSize(block.kind);
        g.rect(block.pos.x * TILE + 1, block.pos.y * TILE + 1, width * TILE - 2, height * TILE - 2).fill({
          color,
          alpha: 0.22
        });
      }
    }
  }

  if (overlay === "cpu") {
    for (const block of snapshot.blocks) {
      if (!block.active) continue;
      const alpha = Math.min(0.55, 0.08 + block.effective_cpu_rate / 180);
      const [width, height] = blockFootprintSize(block.kind);
      g.rect(block.pos.x * TILE + 1, block.pos.y * TILE + 1, width * TILE - 2, height * TILE - 2).fill({
        color: 0x36d399,
        alpha
      });
    }
  }

  if (overlay === "attack") {
    for (const block of snapshot.blocks.filter((item) => item.kind === "turret")) {
      const center = blockCenter(block);
      const range = blockAttackRangeTiles(block.kind);
      if (range === null) continue;
      g.circle(center.x, center.y, TILE * range).stroke({
        width: 1,
        color: 0xf43f5e,
        alpha: 0.18
      });
      const target = snapshot.enemies.find((enemy) => enemy.id === block.target_id);
      if (target) {
        g.moveTo(center.x, center.y);
        g.lineTo(target.pos.x * TILE, target.pos.y * TILE);
        g.stroke({ width: 2, color: 0xf43f5e, alpha: 0.78 });
        g.circle(target.pos.x * TILE, target.pos.y * TILE, 9).stroke({
          width: 2,
          color: 0xf43f5e,
          alpha: 0.82
        });
      }
    }
  }

  if (overlay === "logistics") {
    for (const drone of snapshot.drones) {
      if (!drone.job) continue;
      const dropoff = snapshot.blocks.find((block) => block.id === drone.job?.dropoff);
      if (!dropoff) continue;
      const dropoffCenter = blockCenter(dropoff);
      g.moveTo(drone.pos.x * TILE, drone.pos.y * TILE);
      g.lineTo(dropoffCenter.x, dropoffCenter.y);
      g.stroke({ width: 2, color: 0x38bdf8, alpha: 0.55 });
    }
  }
}

function drawBlocks(g: Graphics, blocks: Block[], selectedId: string | null, renderTimeMs: number) {
  for (const block of blocks) {
    const x = block.pos.x * TILE;
    const y = block.pos.y * TILE;
    const [width, height] = blockFootprintSize(block.kind);
    const pixelWidth = width * TILE;
    const pixelHeight = height * TILE;
    const color = COLORS[block.kind];
    g.roundRect(x + 2, y + 2, pixelWidth - 4, pixelHeight - 4, 3).fill(color);
    if (block.kind === "conveyor") {
      drawConveyorMotion(g, x, y, block.dir, renderTimeMs);
    }
    if (block.kind === "drill" && block.active && block.progress > 0) {
      drawMiningPulse(g, x + pixelWidth / 2, y + pixelHeight / 2, renderTimeMs);
    }
    if (block.kind === "wire") {
      g.moveTo(x + 2, y + 8);
      g.lineTo(x + 14, y + 8);
      g.moveTo(x + 8, y + 2);
      g.lineTo(x + 8, y + 14);
      g.stroke({ width: 2, color: 0xd8e7ff, alpha: 0.65 });
    }
    if (block.kind === "conveyor" || block.kind === "drill" || block.kind === "assembler") {
      drawArrow(g, x + pixelWidth / 2, y + pixelHeight / 2, block.dir, 0x101417);
    }
    if (block.status === "over_budget") {
      g.rect(x + 3, y + 3, pixelWidth - 6, pixelHeight - 6).stroke({ width: 2, color: 0xfacc15 });
    } else if (block.status === "runtime error") {
      g.rect(x + 3, y + 3, pixelWidth - 6, pixelHeight - 6).stroke({ width: 2, color: 0xf87171 });
    }
    if (selectedId === block.id) {
      g.rect(x + 1, y + 1, pixelWidth - 2, pixelHeight - 2).stroke({ width: 2, color: 0xffffff });
    }
  }
}

function drawConveyorMotion(g: Graphics, x: number, y: number, direction: Direction, renderTimeMs: number) {
  const axis = directionVector(direction);
  const phase = ((renderTimeMs / 170) % 1) * 5;
  for (let i = -1; i <= 2; i += 1) {
    const travel = i * 5 + phase - 3;
    const px = x + TILE / 2 + axis.x * travel;
    const py = y + TILE / 2 + axis.y * travel;
    g.circle(px, py, 1.5).fill({ color: 0xf3f7f9, alpha: 0.42 });
  }
}

function drawMiningPulse(g: Graphics, x: number, y: number, renderTimeMs: number) {
  const pulse = 0.5 + Math.sin(renderTimeMs / 130) * 0.5;
  g.circle(x, y, 5 + pulse * 2).stroke({ width: 1.5, color: 0xf5c542, alpha: 0.35 + pulse * 0.35 });
  g.circle(x - 3, y + 2, 1.8).fill({ color: 0xd8a94a, alpha: 0.6 });
}

function drawPlacementPreview(g: Graphics, snapshot: GameSnapshot, preview: PlacementPreview | null) {
  if (!preview) return;
  const [width, height] = blockFootprintSize(preview.kind);
  for (const pos of preview.positions) {
    const valid = canPreviewPlace(snapshot, preview.kind, pos);
    const color = valid ? COLORS[preview.kind] : 0xf87171;
    const x = pos.x * TILE;
    const y = pos.y * TILE;
    const pixelWidth = width * TILE;
    const pixelHeight = height * TILE;
    g.roundRect(x + 1, y + 1, pixelWidth - 2, pixelHeight - 2, 3).fill({
      color,
      alpha: preview.active ? 0.28 : 0.18
    });
    g.roundRect(x + 1, y + 1, pixelWidth - 2, pixelHeight - 2, 3).stroke({
      width: 1.5,
      color: valid ? 0x38bdf8 : 0xf87171,
      alpha: preview.active ? 0.9 : 0.65
    });
    if (preview.kind === "conveyor" || preview.kind === "drill" || preview.kind === "assembler") {
      drawArrow(g, x + pixelWidth / 2, y + pixelHeight / 2, preview.direction, valid ? 0xe8f7ff : 0xfecaca);
    }
  }
}

function canPreviewPlace(snapshot: GameSnapshot, kind: BlockKind, pos: Pos) {
  const [width, height] = blockFootprintSize(kind);
  for (let x = pos.x; x < pos.x + width; x += 1) {
    for (let y = pos.y; y < pos.y + height; y += 1) {
      const tile = snapshot.tiles.find((item) => item.pos.x === x && item.pos.y === y);
      if (!tile?.buildable || tile.block_id) return false;
    }
  }
  return true;
}

function drawItemFlows(g: Graphics, snapshot: GameSnapshot, renderTimeMs: number) {
  const tickFraction = ((renderTimeMs % 100) / 100) * 2;
  for (const flow of snapshot.item_flows) {
    const age = Math.max(0, snapshot.tick - flow.tick);
    if (age > 48) continue;
    const progress = Math.min(1, (age + tickFraction) / 14);
    const x = lerp(flow.from.x, flow.to.x, progress) * TILE;
    const y = lerp(flow.from.y, flow.to.y, progress) * TILE;
    const color = itemColor(flow.item);
    const alpha = Math.max(0, 0.95 - progress * 0.4 - Math.max(0, age - 18) * 0.035);
    const radius = Math.min(6, 3.25 + flow.amount * 0.45);
    g.moveTo(flow.from.x * TILE, flow.from.y * TILE);
    g.lineTo(flow.to.x * TILE, flow.to.y * TILE);
    g.stroke({ width: 2, color, alpha: alpha * 0.22 });
    g.circle(x, y, radius + 4).fill({ color, alpha: alpha * 0.16 });
    g.circle(x, y, radius).fill({ color, alpha });
    g.circle(x, y, radius + 1.5).stroke({ width: 1.5, color: 0x0e1214, alpha: 0.75 });
    const trail = Math.max(0, progress - 0.16);
    const tx = lerp(flow.from.x, flow.to.x, trail) * TILE;
    const ty = lerp(flow.from.y, flow.to.y, trail) * TILE;
    g.circle(tx, ty, Math.max(1.5, radius * 0.45)).fill({ color, alpha: alpha * 0.55 });
  }
}

function drawEnemies(g: Graphics, enemies: Enemy[], selectedId: string | null) {
  for (const enemy of enemies) {
    const x = enemy.pos.x * TILE;
    const y = enemy.pos.y * TILE;
    g.circle(x, y, 6).fill(enemyColor(enemy.kind));
    g.rect(x - 6, y - 9, 12, 2).fill(0x1f2937);
    g.rect(x - 6, y - 9, Math.max(1, 12 * (enemy.hp / enemy.max_hp)), 2).fill(0x22c55e);
    if (selectedId === enemy.id) {
      g.circle(x, y, 8).stroke({ width: 2, color: 0xffffff });
    }
  }
}

function drawDrones(g: Graphics, snapshot: GameSnapshot, selectedId: string | null) {
  for (const drone of snapshot.drones) {
    const x = drone.pos.x * TILE;
    const y = drone.pos.y * TILE;
    g.moveTo(x, y - 6);
    g.lineTo(x + 7, y + 5);
    g.lineTo(x - 7, y + 5);
    g.closePath().fill(0x38bdf8);
    if (selectedId === drone.id) {
      g.circle(x, y, 9).stroke({ width: 2, color: 0xffffff });
    }
  }
}

function distance(a: Pos, b: Pos) {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  return Math.sqrt(dx * dx + dy * dy);
}

function lerp(a: number, b: number, t: number) {
  return a + (b - a) * t;
}

function directionVector(direction: Direction) {
  return {
    north: { x: 0, y: -1 },
    east: { x: 1, y: 0 },
    south: { x: 0, y: 1 },
    west: { x: -1, y: 0 }
  }[direction];
}

function drawArrow(g: Graphics, x: number, y: number, dir: Direction, color: number) {
  const points: Record<Direction, number[]> = {
    north: [x, y - 5, x + 4, y + 3, x - 4, y + 3],
    east: [x + 5, y, x - 3, y + 4, x - 3, y - 4],
    south: [x, y + 5, x + 4, y - 3, x - 4, y - 3],
    west: [x - 5, y, x + 3, y + 4, x + 3, y - 4]
  };
  g.poly(points[dir]).fill(color);
}

function networkColor(id: number) {
  const colors = [0x38bdf8, 0x36d399, 0xa78bfa, 0xf59e0b, 0xf43f5e];
  return colors[id % colors.length];
}

function label(kind: BlockKind) {
  return kind.replaceAll("_", " ");
}

function arrow(dir: Direction) {
  return { north: "^", east: ">", south: "v", west: "<" }[dir];
}

function blockCenter(block: Block) {
  const [width, height] = blockFootprintSize(block.kind);
  return {
    x: (block.pos.x + width / 2) * TILE,
    y: (block.pos.y + height / 2) * TILE
  };
}
