import type { Block, Direction, ItemKind, TerrainKind } from "../types";
import { mockSourceWithoutComments } from "./mockBehaviorValidator";

export type MockBehaviorResult = {
  mine: boolean;
  output: { item: ItemKind; dir: Direction } | null;
  router: { item: ItemKind | null; dirs: Direction[] } | null;
  assembler: { recipe: ItemKind | null; produce: boolean } | null;
  turret: { priority: string[] } | null;
};

export type MockBehaviorEnvironment = {
  outputBlocked: () => boolean;
  outputAvailable: (dir: Direction, item: ItemKind | null) => boolean;
  terrainAtSelf: () => TerrainKind;
  visibleTurretTargetCount: () => number;
  canAttackTurretIndex: (index: number) => boolean;
};

type MockBehaviorContext = {
  assemblerRecipe: string | null;
};

export function emptyMockBehaviorResult(): MockBehaviorResult {
  return { mine: false, output: null, router: null, assembler: null, turret: null };
}

export function evaluateMockBehaviorScript(
  block: Block,
  source: string,
  environment: MockBehaviorEnvironment
): MockBehaviorResult {
  const result = emptyMockBehaviorResult();
  const context: MockBehaviorContext = { assemblerRecipe: block.recipe };

  for (const line of mockSourceWithoutComments(source).toLowerCase().split(/\n/).filter(Boolean)) {
    const action = line.startsWith("if ") ? activeMockAction(block, line, environment, context) : line;
    if (!action) continue;
    if (action === "return" || action === "stop") break;
    applyMockAction(block, action, result, context);
  }
  return result;
}

function activeMockAction(
  block: Block,
  line: string,
  environment: MockBehaviorEnvironment,
  context: MockBehaviorContext
) {
  const tokens = line.split(/\s+/).filter(Boolean);
  if (mockCondition(block, tokens.slice(1), environment, context)) {
    return tokens.slice(mockConditionTokenLength(block, tokens.slice(1)) + 1).join(" ");
  }
  return null;
}

function mockCondition(
  block: Block,
  tokens: string[],
  environment: MockBehaviorEnvironment,
  context: MockBehaviorContext
) {
  if (block.kind === "drill") {
    if (tokens[0] === "output_blocked") return environment.outputBlocked();
    if (tokens[0] === "ore_kind" && tokens[1] === "==" && tokens[2] === "ore") {
      return environment.terrainAtSelf() === "ore_patch";
    }
  }
  if (block.kind === "router") {
    if (tokens[0] === "output_available" && isItem(tokens[1]) && isDirection(tokens[2])) {
      return environment.outputAvailable(tokens[2], tokens[1]);
    }
    if (tokens[0] === "output_available" && isDirection(tokens[1])) {
      return environment.outputAvailable(tokens[1], null);
    }
  }
  if (block.kind === "assembler") {
    if (tokens[0] === "can_produce") return canMockProduce(block, context.assemblerRecipe);
    if (tokens[0] === "current_recipe" && tokens[1] === "==") return block.recipe === tokens[2];
    if ((tokens[0] === "input_count" || tokens[0] === "output_count") && isItem(tokens[1])) {
      return compareNumber(inventoryCount(block, tokens[1]), tokens[2], Number(tokens[3]));
    }
  }
  if (block.kind === "turret") {
    if (tokens[0] === "ammo_count") return compareNumber(inventoryCount(block, "ammo"), tokens[1], Number(tokens[2]));
    if (tokens[0] === "scan_enemies") {
      return compareNumber(environment.visibleTurretTargetCount(), tokens[1], Number(tokens[2]));
    }
    if (tokens[0] === "can_attack") return environment.canAttackTurretIndex(Number(tokens[1]));
  }
  if (tokens[0] === "inventory_count" && isItem(tokens[1])) {
    return compareNumber(inventoryCount(block, tokens[1]), tokens[2], Number(tokens[3]));
  }
  if (tokens[0] === "inventory_free") {
    return compareNumber(block.inventory.capacity - inventoryTotal(block), tokens[1], Number(tokens[2]));
  }
  if (tokens[0] === "fuel_remaining") {
    return compareNumber(Math.floor(block.fuel_bank), tokens[1], Number(tokens[2]));
  }
  return false;
}

function mockConditionTokenLength(block: Block, tokens: string[]) {
  if (block.kind === "drill" && tokens[0] === "output_blocked") return 1;
  if (block.kind === "drill" && tokens[0] === "ore_kind") return 3;
  if (block.kind === "router" && tokens[0] === "output_available" && isItem(tokens[1])) return 3;
  if (block.kind === "router" && tokens[0] === "output_available") return 2;
  if (block.kind === "assembler" && tokens[0] === "can_produce") return 1;
  if (block.kind === "assembler" && tokens[0] === "current_recipe") return 3;
  if (block.kind === "assembler" && (tokens[0] === "input_count" || tokens[0] === "output_count")) return 4;
  if (block.kind === "turret" && (tokens[0] === "ammo_count" || tokens[0] === "scan_enemies")) return 3;
  if (block.kind === "turret" && tokens[0] === "can_attack") return 2;
  if (tokens[0] === "inventory_count") return 4;
  if (tokens[0] === "inventory_free" || tokens[0] === "fuel_remaining") return 3;
  return tokens.length;
}

function applyMockAction(
  block: Block,
  action: string,
  result: MockBehaviorResult,
  context: MockBehaviorContext
) {
  const tokens = action.split(/\s+/).filter(Boolean);
  if (tokens[0] === "mine" && block.kind === "drill") {
    result.mine = true;
  } else if (tokens[0] === "output" && block.kind === "drill" && isItem(tokens[1])) {
    result.output = { item: tokens[1], dir: block.dir };
  } else if ((tokens[0] === "push_any" || action === "push any") && block.kind === "router") {
    result.router = { item: null, dirs: allDirections() };
  } else if (tokens[0] === "push" && block.kind === "router" && isDirection(tokens[1])) {
    result.router = { item: null, dirs: [tokens[1]] };
  } else if (tokens[0] === "push" && block.kind === "router" && isItem(tokens[1]) && isDirection(tokens[2])) {
    result.router = { item: tokens[1], dirs: [tokens[2]] };
  } else if (tokens[0] === "set_recipe" && block.kind === "assembler" && isRecipe(tokens[1])) {
    context.assemblerRecipe = tokens[1];
    result.assembler = { recipe: tokens[1], produce: result.assembler?.produce ?? false };
  } else if (tokens[0] === "produce" && block.kind === "assembler") {
    result.assembler = { recipe: result.assembler?.recipe ?? null, produce: true };
  } else if (tokens[0] === "attack_nearest" && block.kind === "turret") {
    result.turret = { priority: ["nearest"] };
  } else if (tokens[0] === "attack_best" && block.kind === "turret") {
    result.turret = { priority: tokens.slice(1) };
  } else if (tokens[0] === "attack" && block.kind === "turret") {
    result.turret = { priority: [`index:${tokens[1]}`] };
  }
}

function canMockProduce(block: Block, recipe: string | null) {
  if (recipe === "plate") {
    return inventoryCount(block, "ore") >= 2 && inventoryTotal(block) < block.inventory.capacity;
  }
  if (recipe === "ammo") {
    return inventoryCount(block, "plate") >= 1 && inventoryTotal(block) + 1 < block.inventory.capacity;
  }
  return false;
}

function inventoryCount(block: Block, item: ItemKind) {
  return block.inventory.items[item] ?? 0;
}

function inventoryTotal(block: Block) {
  return Object.values(block.inventory.items).reduce((sum, amount) => sum + (amount ?? 0), 0);
}

function compareNumber(left: number, operator: string | undefined, right: number) {
  if (!Number.isFinite(right)) return false;
  if (operator === "<") return left < right;
  if (operator === "<=") return left <= right;
  if (operator === "==") return left === right;
  if (operator === ">=") return left >= right;
  if (operator === ">") return left > right;
  return false;
}

function allDirections(): Direction[] {
  return ["north", "east", "south", "west"];
}

function isItem(value: string | undefined): value is ItemKind {
  return value === "ore" || value === "plate" || value === "ammo" || value === "cpu_part" || value === "drone_part";
}

function isRecipe(value: string | undefined): value is ItemKind {
  return value === "plate" || value === "ammo";
}

function isDirection(value: string | undefined): value is Direction {
  return value === "north" || value === "east" || value === "south" || value === "west";
}
