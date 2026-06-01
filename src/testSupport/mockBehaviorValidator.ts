import type { BehaviorKind, BehaviorSource } from "../types";

const SCRIPT_ITEMS = new Set(["ore", "plate", "ammo", "cpu_part", "cpu-part", "drone_part", "drone-part"]);
const SCRIPT_RECIPES = new Set(["plate", "ammo"]);
const SCRIPT_DIRECTIONS = new Set(["north", "east", "south", "west"]);
const SCRIPT_COMPARISONS = new Set(["<", "<=", "==", ">=", ">"]);
const SCRIPT_ENEMIES = new Set(["grunt", "runner", "armored", "wire_cutter", "wire-cutter"]);
const SCRIPT_ATTACK_POLICIES = new Set(["nearest", "lowest_hp", "weakest", "runner", "wire_cutter", "wire-cutter", "armored", "grunt"]);
const SCRIPT_DROPOFF_TAGS = new Set(["frontline"]);

export function validateMockBehaviorBuild(behavior: BehaviorSource) {
  const kind = behavior.summary.base_kind;
  const source = mockSourceWithoutComments(behavior.source).toLowerCase();
  if (source.startsWith("(module")) {
    return source.includes('"tick"') ? null : "mock build failed: WAT behavior must export tick";
  }
  if (isTinyBehaviorSource(source)) {
    return balancedDelimiters(source) ? null : "mock build failed: Tiny behavior has unbalanced delimiters";
  }

  const lines = source.split(/\n/).filter(Boolean);
  if (lines.length === 0) {
    return `mock build failed: ${kind} behavior is empty`;
  }
  for (const [index, line] of lines.entries()) {
    if (!isSupportedScriptLine(kind, line)) {
      return `mock build failed: unsupported ${kind} line ${index + 1}: ${line}`;
    }
  }
  return null;
}

export function mockSourceWithoutComments(source: string) {
  return source
    .split(/\r?\n/)
    .map((line) => line.replace(/#.*/, "").replace(/\/\/.*/, "").trim())
    .filter(Boolean)
    .join("\n");
}

function isTinyBehaviorSource(source: string) {
  return /^(?:export\s+|pub\s+)?fn\s+tick\b/.test(source) || /^void\s+tick\b/.test(source);
}

function isSupportedScriptLine(kind: BehaviorKind, line: string) {
  const tokens = line.split(/\s+/).filter(Boolean);
  if (tokens[0] === "if") {
    const actionStart = mockConditionLength(kind, tokens);
    return actionStart !== null && isSupportedScriptAction(kind, tokens.slice(actionStart));
  }
  return isSupportedScriptAction(kind, tokens);
}

function mockConditionLength(kind: BehaviorKind, tokens: string[]) {
  if (tokens[0] !== "if") return null;

  if (kind === "drill") {
    if (tokens[1] === "output_blocked") return 2;
    if (tokens[1] === "ore_kind" && tokens[2] === "==" && isScriptItem(tokens[3])) return 4;
  }
  if (kind === "router") {
    if (tokens[1] === "output_available" && isScriptItem(tokens[2]) && isDirection(tokens[3])) return 4;
    if (tokens[1] === "output_available" && isDirection(tokens[2])) return 3;
  }
  if (kind === "assembler") {
    if (tokens[1] === "can_produce") return 2;
    if (tokens[1] === "current_recipe" && tokens[2] === "==" && isRecipe(tokens[3])) return 4;
    if (["input_count", "output_count"].includes(tokens[1] ?? "")) {
      return isScriptItem(tokens[2]) && isComparison(tokens[3]) && isInteger(tokens[4]) ? 5 : null;
    }
  }
  if (kind === "turret") {
    if (tokens[1] === "ammo_count" && tokens[2] === ">" && tokens[3] === "0") return 4;
    if (tokens[1] === "scan_enemies" && isComparison(tokens[2]) && isInteger(tokens[3])) return 4;
    if (tokens[1] === "enemy_kind" && isInteger(tokens[2]) && tokens[3] === "==" && isEnemyKind(tokens[4])) return 5;
    if (tokens[1] === "enemy_hp" && isInteger(tokens[2]) && isComparison(tokens[3]) && isInteger(tokens[4])) return 5;
    if (tokens[1] === "enemy_distance" && isInteger(tokens[2]) && isComparison(tokens[3]) && isFiniteNumber(tokens[4])) return 5;
    if (tokens[1] === "can_attack" && isInteger(tokens[2])) return 3;
  }
  if (kind === "drone_port") {
    if (["docked_drone_count", "pending_job_count"].includes(tokens[1] ?? "")) {
      return isComparison(tokens[2]) && isInteger(tokens[3]) ? 4 : null;
    }
  }
  if (kind === "carrier_drone") {
    if (tokens[1] === "battery_percent" && tokens[2] === "<" && isInteger(tokens[3])) return 4;
    if (tokens[1] === "battery_ratio" && tokens[2] === "<" && isFiniteNumber(tokens[3])) return 4;
    if (tokens[1] === "logic_fuel_remaining" && tokens[2] === "<" && isInteger(tokens[3])) return 4;
    if (tokens[1] === "has_job" || tokens[1] === "has_pending_job") return 2;
    if (tokens[1] === "cargo_count" && isScriptItem(tokens[2]) && isComparison(tokens[3]) && isInteger(tokens[4])) return 5;
  }

  if (tokens[1] === "inventory_count" && isScriptItem(tokens[2]) && isComparison(tokens[3]) && isInteger(tokens[4])) return 5;
  if (tokens[1] === "inventory_free" && isComparison(tokens[2]) && isInteger(tokens[3])) return 4;
  if (["stock_count", "stock_capacity"].includes(tokens[1] ?? "") && isScriptItem(tokens[2]) && isComparison(tokens[3]) && isInteger(tokens[4])) return 5;
  if (tokens[1] === "has_space" && isScriptItem(tokens[2]) && isInteger(tokens[3])) return 4;
  if (tokens[1] === "fuel_remaining" && tokens[2] === ">" && isInteger(tokens[3])) return 4;
  if (tokens[1] === "net" && isInteger(tokens[2]) && [">", "=="].includes(tokens[3] ?? "") && isInteger(tokens[4])) return 5;

  return null;
}

function isSupportedScriptAction(kind: BehaviorKind, tokens: string[]) {
  if (tokens.length === 0) return true;
  if (["return", "stop", "noop"].includes(tokens[0] ?? "")) return tokens.length === 1;
  if (tokens[0] === "log") return tokens.length > 1;
  if (tokens[0] === "net_set") return isInteger(tokens[1]) && isInteger(tokens[2]) && tokens.length === 3;
  if (["net_delete", "net_del"].includes(tokens[0] ?? "")) return isInteger(tokens[1]) && tokens.length === 2;

  if (kind === "drill") {
    return (tokens[0] === "mine" && tokens.length === 1) || (tokens[0] === "output" && isScriptItem(tokens[1]) && tokens.length === 2);
  }
  if (kind === "router") {
    if (tokens[0] === "push_any") return tokens.length === 1;
    if (tokens[0] !== "push") return false;
    if (tokens[1] === "any") return tokens.length === 2;
    if (isDirection(tokens[1])) return tokens.length === 2;
    return isScriptItem(tokens[1]) && isDirection(tokens[2]) && tokens.length === 3;
  }
  if (kind === "assembler") {
    return (tokens[0] === "set_recipe" && isRecipe(tokens[1]) && tokens.length === 2) || (tokens[0] === "produce" && tokens.length === 1);
  }
  if (kind === "turret") {
    if (tokens[0] === "attack_nearest") return tokens.length === 1;
    if (tokens[0] === "attack") return isInteger(tokens[1]) && tokens.length === 2;
    return tokens[0] === "attack_best" && tokens.length > 1 && tokens.slice(1).every(isAttackPolicyToken);
  }
  if (kind === "drone_port") {
    if (["dispatch", "charge_docked_drones", "dispatch_idle_drones"].includes(tokens[0] ?? "")) return tokens.length === 1;
    return (
      tokens[0] === "create_delivery_job" &&
      isScriptItem(tokens[1]) &&
      isInteger(tokens[2]) &&
      SCRIPT_DROPOFF_TAGS.has(tokens[3] ?? "") &&
      tokens.length === 4
    );
  }
  if (kind === "carrier_drone") {
    if (["return_to_port", "claim_delivery_job", "deliver", "idle"].includes(tokens[0] ?? "")) return tokens.length === 1;
    if (tokens[0] === "move_to") return isInteger(tokens[1]) && isInteger(tokens[2]) && tokens.length === 3;
    return ["load", "unload"].includes(tokens[0] ?? "") && isScriptItem(tokens[1]) && isInteger(tokens[2]) && tokens.length === 3;
  }
  return false;
}

function isScriptItem(value: string | undefined) {
  return value !== undefined && SCRIPT_ITEMS.has(value);
}

function isRecipe(value: string | undefined) {
  return value !== undefined && SCRIPT_RECIPES.has(value);
}

function isDirection(value: string | undefined) {
  return value !== undefined && SCRIPT_DIRECTIONS.has(value);
}

function isComparison(value: string | undefined) {
  return value !== undefined && SCRIPT_COMPARISONS.has(value);
}

function isEnemyKind(value: string | undefined) {
  return value !== undefined && SCRIPT_ENEMIES.has(value);
}

function isAttackPolicyToken(value: string) {
  return value
    .split(",")
    .map((policy) => policy.trim())
    .filter(Boolean)
    .every((policy) => SCRIPT_ATTACK_POLICIES.has(policy));
}

function isInteger(value: string | undefined) {
  return value !== undefined && /^-?\d+$/.test(value);
}

function isFiniteNumber(value: string | undefined) {
  return value !== undefined && Number.isFinite(Number(value));
}

function balancedDelimiters(source: string) {
  const stack: string[] = [];
  const pairs: Record<string, string> = { ")": "(", "}": "{" };
  for (const char of source) {
    if (char === "(" || char === "{") {
      stack.push(char);
    } else if (char === ")" || char === "}") {
      if (stack.pop() !== pairs[char]) return false;
    }
  }
  return stack.length === 0;
}
