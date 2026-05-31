import { Application, Container, Graphics, Text } from "pixi.js";
import { useEffect, useRef } from "react";
import type { Block, BlockKind, Direction, Enemy, GameSnapshot, Pos, Tile } from "./types";

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
          const current = snapshotRef.current;
          if (!current || pos.x < 0 || pos.y < 0 || pos.x >= current.width || pos.y >= current.height) {
            onEntityClickRef.current(null);
            return;
          }
          const block = current.blocks.find((item) => item.pos.x === pos.x && item.pos.y === pos.y);
          const enemy = current.enemies.find((item) => item.pos.x === pos.x && item.pos.y === pos.y);
          if (buildKindRef.current) {
            onTileClickRef.current(pos);
          } else if (block) {
            onEntityClickRef.current(block.id);
          } else if (enemy) {
            onEntityClickRef.current(enemy.id);
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

  return <div ref={hostRef} className="grid-world" />;
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
  drawEnemies(graphics, snapshot.enemies, selectedId);
  drawDrones(graphics, snapshot);
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
        g.rect(block.pos.x * TILE + 1, block.pos.y * TILE + 1, TILE - 2, TILE - 2).fill({
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
      g.rect(block.pos.x * TILE + 1, block.pos.y * TILE + 1, TILE - 2, TILE - 2).fill({
        color: 0x36d399,
        alpha
      });
    }
  }

  if (overlay === "attack") {
    for (const block of snapshot.blocks.filter((item) => item.kind === "turret")) {
      g.circle(block.pos.x * TILE + 8, block.pos.y * TILE + 8, TILE * 8).stroke({
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
      g.moveTo(drone.pos.x * TILE + 8, drone.pos.y * TILE + 8);
      g.lineTo(dropoff.pos.x * TILE + 8, dropoff.pos.y * TILE + 8);
      g.stroke({ width: 2, color: 0x38bdf8, alpha: 0.55 });
    }
  }
}

function drawBlocks(g: Graphics, blocks: Block[], selectedId: string | null) {
  for (const block of blocks) {
    const x = block.pos.x * TILE;
    const y = block.pos.y * TILE;
    const color = COLORS[block.kind];
    g.roundRect(x + 2, y + 2, TILE - 4, TILE - 4, 3).fill(color);
    if (block.kind === "wire") {
      g.moveTo(x + 2, y + 8);
      g.lineTo(x + 14, y + 8);
      g.moveTo(x + 8, y + 2);
      g.lineTo(x + 8, y + 14);
      g.stroke({ width: 2, color: 0xd8e7ff, alpha: 0.65 });
    }
    if (block.kind === "conveyor" || block.kind === "drill" || block.kind === "assembler") {
      drawArrow(g, x + 8, y + 8, block.dir, 0x101417);
    }
    if (block.status === "over_budget") {
      g.rect(x + 3, y + 3, TILE - 6, TILE - 6).stroke({ width: 2, color: 0xfacc15 });
    }
    if (selectedId === block.id) {
      g.rect(x + 1, y + 1, TILE - 2, TILE - 2).stroke({ width: 2, color: 0xffffff });
    }
  }
}

function drawEnemies(g: Graphics, enemies: Enemy[], selectedId: string | null) {
  for (const enemy of enemies) {
    const x = enemy.pos.x * TILE + 8;
    const y = enemy.pos.y * TILE + 8;
    g.circle(x, y, 6).fill(ENEMY_COLORS[enemy.kind]);
    g.rect(x - 6, y - 9, 12, 2).fill(0x1f2937);
    g.rect(x - 6, y - 9, Math.max(1, 12 * (enemy.hp / enemy.max_hp)), 2).fill(0x22c55e);
    if (selectedId === enemy.id) {
      g.circle(x, y, 8).stroke({ width: 2, color: 0xffffff });
    }
  }
}

function drawDrones(g: Graphics, snapshot: GameSnapshot) {
  for (const drone of snapshot.drones) {
    const x = drone.pos.x * TILE + 8;
    const y = drone.pos.y * TILE + 8;
    g.moveTo(x, y - 6);
    g.lineTo(x + 7, y + 5);
    g.lineTo(x - 7, y + 5);
    g.closePath().fill(0x38bdf8);
  }
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
