import type { Direction, EnemyKind, ItemKind } from "./types";

const SCRIPT_CONTROL_KEYWORDS = ["if", "return", "stop", "noop", "log"] as const;

const SCRIPT_CONDITION_KEYWORDS = [
  "output_blocked",
  "ore_kind",
  "output_available",
  "can_produce",
  "current_recipe",
  "input_count",
  "output_count",
  "ammo_count",
  "scan_enemies",
  "enemy_kind",
  "enemy_hp",
  "enemy_distance",
  "can_attack",
  "inventory_count",
  "inventory_free",
  "stock_count",
  "stock_capacity",
  "has_space",
  "docked_drone_count",
  "pending_job_count",
  "battery_ratio",
  "battery_percent",
  "logic_fuel_remaining",
  "has_job",
  "has_pending_job",
  "cargo_count",
  "fuel_remaining",
  "net"
] as const;

const SCRIPT_ACTION_KEYWORDS = [
  "mine",
  "output",
  "push",
  "push_any",
  "set_recipe",
  "produce",
  "attack",
  "attack_nearest",
  "attack_best",
  "dispatch",
  "charge_docked_drones",
  "create_delivery_job",
  "dispatch_idle_drones",
  "return_to_port",
  "claim_delivery_job",
  "deliver",
  "move_to",
  "load",
  "unload",
  "idle",
  "net_set",
  "net_delete",
  "net_del"
] as const;

const HOST_IMPORT_KEYWORDS = [
  "store_get_i32",
  "store_set_i32",
  "store_delete_i32",
  "push_dir",
  "push_item_dir",
  "output_item_available"
] as const;

const ITEM_KEYWORDS = [
  "ore",
  "plate",
  "ammo",
  "cpu_part",
  "drone_part"
] as const satisfies readonly ItemKind[];
const ITEM_ALIAS_KEYWORDS = ["cpu-part", "drone-part"] as const;
const DIRECTION_KEYWORDS = ["north", "east", "south", "west"] as const satisfies readonly Direction[];
const ROUTER_TARGET_KEYWORDS = ["any"] as const;
const ATTACK_POLICY_KEYWORDS = ["nearest", "lowest_hp", "weakest"] as const;
const ENEMY_KIND_KEYWORDS = [
  "grunt",
  "runner",
  "armored",
  "wire_cutter"
] as const satisfies readonly EnemyKind[];
const ENEMY_KIND_ALIAS_KEYWORDS = ["wire-cutter"] as const;
const DROPOFF_TAG_KEYWORDS = ["frontline"] as const;

const WAT_KEYWORDS = [
  "module",
  "import",
  "func",
  "export",
  "param",
  "result",
  "i32",
  "i64",
  "f32",
  "local",
  "local.get",
  "local.set",
  "local.tee",
  "memory",
  "data",
  "global",
  "global.get",
  "block",
  "loop",
  "then",
  "else",
  "end",
  "drop",
  "call",
  "br",
  "br_if",
  "select",
  "i32.const",
  "i32.add",
  "i32.sub",
  "i32.mul",
  "i32.div_s",
  "i32.rem_s",
  "i32.eqz",
  "i32.eq",
  "i32.ne",
  "i32.lt_s",
  "i32.le_s",
  "i32.gt_s",
  "i32.ge_s",
  "i64.const",
  "i64.lt_s",
  "i64.gt_s",
  "f32.const",
  "f32.lt",
  "f32.le",
  "f32.gt",
  "f32.ge"
] as const;

function uniqueKeywords(keywords: readonly string[]) {
  return Array.from(new Set(keywords));
}

export const BEHAVIOR_LANGUAGE_KEYWORDS = uniqueKeywords([
  ...SCRIPT_CONTROL_KEYWORDS,
  ...SCRIPT_CONDITION_KEYWORDS,
  ...SCRIPT_ACTION_KEYWORDS,
  ...HOST_IMPORT_KEYWORDS,
  ...ITEM_KEYWORDS,
  ...ITEM_ALIAS_KEYWORDS,
  ...DIRECTION_KEYWORDS,
  ...ROUTER_TARGET_KEYWORDS,
  ...ATTACK_POLICY_KEYWORDS,
  ...ENEMY_KIND_KEYWORDS,
  ...ENEMY_KIND_ALIAS_KEYWORDS,
  ...DROPOFF_TAG_KEYWORDS,
  ...WAT_KEYWORDS
]);
