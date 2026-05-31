export type Direction = "north" | "east" | "south" | "west";
export type BlockKind =
  | "core"
  | "wire"
  | "cpu_node"
  | "drill"
  | "conveyor"
  | "router"
  | "storage"
  | "assembler"
  | "turret"
  | "drone_port";

export type EnemyKind = "grunt" | "runner" | "armored" | "wire_cutter";
export type ItemKind = "ore" | "plate" | "ammo" | "cpu_part" | "drone_part";
export type TerrainKind = "ground" | "ore_patch" | "rock";
export type LogLevel = "info" | "warn" | "error";

export interface Pos {
  x: number;
  y: number;
}

export interface Inventory {
  items: Partial<Record<ItemKind, number>>;
  capacity: number;
}

export interface Tile {
  pos: Pos;
  terrain: TerrainKind;
  buildable: boolean;
  enemy_passable: boolean;
  block_id: string | null;
}

export interface Block {
  id: string;
  kind: BlockKind;
  pos: Pos;
  dir: Direction;
  hp: number;
  inventory: Inventory;
  behavior_ref: string | null;
  tags: string[];
  active: boolean;
  network_id: number | null;
  effective_cpu_rate: number;
  progress: number;
  status: string;
}

export interface Enemy {
  id: string;
  kind: EnemyKind;
  pos: Pos;
  hp: number;
  max_hp: number;
  speed_ticks: number;
  move_cooldown: number;
  move_speed: number;
  target_id: string | null;
}

export interface DeliveryJob {
  id: string;
  item: ItemKind;
  amount: number;
  pickup: string;
  dropoff: string;
  priority: number;
}

export interface Drone {
  id: string;
  pos: Pos;
  battery: number;
  logic_fuel: number;
  cargo: Inventory;
  state: "docked" | "delivering" | "returning" | "offline";
  job: DeliveryJob | null;
}

export interface Network {
  id: number;
  cpu_pool: number;
  active_devices: number;
  effective_per_device: number;
  block_ids: string[];
  store: Record<string, unknown>;
  read_only_cache: boolean;
}

export interface BehaviorSummary {
  id: string;
  display_name: string;
  base_kind: BlockKind;
  world: string;
  builtin: boolean;
  used_by: number;
  source_path: string;
  build_status: string;
}

export interface BehaviorSource {
  summary: BehaviorSummary;
  source: string;
}

export interface BuildResult {
  behavior_id: string;
  success: boolean;
  message: string;
  wasm_hash: string | null;
}

export interface LogEntry {
  tick: number;
  level: LogLevel;
  source: string;
  message: string;
}

export interface GameSnapshot {
  tick: number;
  running: boolean;
  width: number;
  height: number;
  tiles: Tile[];
  blocks: Block[];
  enemies: Enemy[];
  drones: Drone[];
  networks: Network[];
  logs: LogEntry[];
  selected_id: string | null;
  behaviors: BehaviorSummary[];
  pending_jobs: DeliveryJob[];
}

export interface BuildPaletteItem {
  kind: BlockKind;
  label: string;
  category: string;
  dir?: Direction;
}
