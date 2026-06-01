import { Application, Container, Graphics, Text } from "pixi.js";
import { useEffect, useRef } from "react";
import { blockFootprintSize } from "./gameMetadata";
import type { Block, BlockKind, Direction, Enemy, GameSnapshot, ItemFlowEvent, ItemKind, Pos, Tile } from "./types";

type Overlay = "none" | "network" | "cpu" | "logistics" | "attack";

interface GridWorldProps {
  snapshot: GameSnapshot | null;
  selectedId: string | null;
  buildKind: BlockKind | null;
  direction: Direction;
  overlay: Overlay;
  onTileClick: (pos: Pos) => void;
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

const ENEMY_COLORS = {
  grunt: 0xef4444,
  runner: 0xf97316,
  armored: 0x7f1d1d,
  wire_cutter: 0xeab308
};

const ITEM_COLORS: Record<ItemKind, number> = {
  ore: 0xd8a94a,
  plate: 0xb9c2cf,
  ammo: 0xf43f5e,
  cpu_part: 0x36d399,
  drone_part: 0x38bdf8
};

export function GridWorld({
  snapshot,
  selectedId,
  buildKind,
  direction,
  overlay,
  onTileClick,
  onEntityClick
}: GridWorldProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const appRef = useRef<Application | null>(null);
  const stageRef = useRef<Container | null>(null);
  const snapshotRef = useRef<GameSnapshot | null>(null);
  const buildKindRef = useRef<BlockKind | null>(buildKind);
  const onTileClickRef = useRef(onTileClick);
  const onEntityClickRef = useRef(onEntityClick);

  useEffect(() => {
    buildKindRef.current = buildKind;
    onTileClickRef.current = onTileClick;
    onEntityClickRef.current = onEntityClick;
  }, [buildKind, onTileClick, onEntityClick]);

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
          const world = event.global;
          const pos = {
            x: Math.floor(world.x / TILE),
            y: Math.floor(world.y / TILE)
          };
          const worldTile = {
            x: world.x / TILE,
            y: world.y / TILE
          };
          const current = snapshotRef.current;
          if (!current || pos.x < 0 || pos.y < 0 || pos.x >= current.width || pos.y >= current.height) {
            onEntityClickRef.current(null);
            return;
          }
          const tile = current.tiles.find((item) => item.pos.x === pos.x && item.pos.y === pos.y);
          const block = current.blocks.find((item) => item.id === tile?.block_id);
          const enemy = current.enemies.find((item) => distance(item.pos, worldTile) <= 0.55);
          const drone = current.drones.find((item) => distance(item.pos, worldTile) <= 0.6);
          if (buildKindRef.current) {
            onTileClickRef.current(pos);
          } else if (enemy) {
            onEntityClickRef.current(enemy.id);
          } else if (drone) {
            onEntityClickRef.current(drone.id);
          } else if (block) {
            onEntityClickRef.current(block.id);
          } else {
            onEntityClickRef.current(null);
          }
        });
        renderWorld(stage, snapshotRef.current, selectedId, buildKind, direction, overlay);
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
    if (stageRef.current) {
      renderWorld(stageRef.current, snapshot, selectedId, buildKind, direction, overlay);
    }
  }, [snapshot, selectedId, buildKind, direction, overlay]);

  return <div ref={hostRef} className="grid-world" data-testid="grid-world" />;
}

function renderWorld(
  stage: Container,
  snapshot: GameSnapshot | null,
  selectedId: string | null,
  buildKind: BlockKind | null,
  direction: Direction,
  overlay: Overlay
) {
  stage.removeChildren();
  if (!snapshot) return;

  const graphics = new Graphics();
  drawTiles(graphics, snapshot.tiles);
  drawOverlays(graphics, snapshot, overlay);
  drawBlocks(graphics, snapshot.blocks, selectedId);
  drawItemFlows(graphics, snapshot);
  drawEnemies(graphics, snapshot.enemies, selectedId);
  drawDrones(graphics, snapshot, selectedId);
  stage.addChild(graphics);

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

function drawTiles(g: Graphics, tiles: Tile[]) {
  for (const tile of tiles) {
    const x = tile.pos.x * TILE;
    const y = tile.pos.y * TILE;
    const fill = tile.terrain === "ore_patch" ? 0x40351e : 0x151b1f;
    g.rect(x, y, TILE, TILE).fill(fill);
    if (tile.terrain === "ore_patch") {
      g.circle(x + 8, y + 8, 2).fill(0xd8a94a);
    }
  }
  g.stroke({ width: 1, color: 0x253038, alpha: 0.45 });
  for (let x = 0; x <= 64; x++) {
    g.moveTo(x * TILE, 0);
    g.lineTo(x * TILE, 64 * TILE);
  }
  for (let y = 0; y <= 64; y++) {
    g.moveTo(0, y * TILE);
    g.lineTo(64 * TILE, y * TILE);
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
      g.circle(center.x, center.y, TILE * 8).stroke({
        width: 1,
        color: 0xf43f5e,
        alpha: 0.18
      });
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

function drawBlocks(g: Graphics, blocks: Block[], selectedId: string | null) {
  for (const block of blocks) {
    const x = block.pos.x * TILE;
    const y = block.pos.y * TILE;
    const [width, height] = blockFootprintSize(block.kind);
    const pixelWidth = width * TILE;
    const pixelHeight = height * TILE;
    const color = COLORS[block.kind];
    g.roundRect(x + 2, y + 2, pixelWidth - 4, pixelHeight - 4, 3).fill(color);
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
    }
    if (selectedId === block.id) {
      g.rect(x + 1, y + 1, pixelWidth - 2, pixelHeight - 2).stroke({ width: 2, color: 0xffffff });
    }
  }
}

function drawItemFlows(g: Graphics, snapshot: GameSnapshot) {
  for (const flow of snapshot.item_flows) {
    const age = Math.max(0, snapshot.tick - flow.tick);
    if (age > 30) continue;
    const progress = Math.min(1, age / 12);
    const x = lerp(flow.from.x, flow.to.x, progress) * TILE;
    const y = lerp(flow.from.y, flow.to.y, progress) * TILE;
    const alpha = 0.9 - progress * 0.45;
    const radius = Math.min(5, 2.5 + flow.amount * 0.35);
    g.circle(x, y, radius).fill({ color: itemColor(flow), alpha });
    g.circle(x, y, radius + 1).stroke({ width: 1, color: 0x0e1214, alpha: 0.55 });
  }
}

function itemColor(flow: ItemFlowEvent) {
  return ITEM_COLORS[flow.item];
}

function drawEnemies(g: Graphics, enemies: Enemy[], selectedId: string | null) {
  for (const enemy of enemies) {
    const x = enemy.pos.x * TILE;
    const y = enemy.pos.y * TILE;
    g.circle(x, y, 6).fill(ENEMY_COLORS[enemy.kind]);
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
