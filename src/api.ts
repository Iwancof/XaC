import { invoke } from "@tauri-apps/api/core";
import type {
  BehaviorSource,
  BlockKind,
  BuildResult,
  Direction,
  GameSnapshot
} from "./types";

export function getSnapshot() {
  return invoke<GameSnapshot>("get_snapshot");
}

export function setRunning(running: boolean) {
  return invoke<GameSnapshot>("set_running", { running });
}

export function stepTicks(count: number) {
  return invoke<GameSnapshot>("step_ticks", { count });
}

export function advance(maxTicks: number) {
  return invoke<GameSnapshot>("advance", { maxTicks });
}

export function placeBlock(kind: BlockKind, x: number, y: number, dir: Direction) {
  return invoke<GameSnapshot>("place_block", { kind, x, y, dir });
}

export function deconstructBlock(blockId: string) {
  return invoke<GameSnapshot>("deconstruct_block", { blockId });
}

export function rotateBlock(blockId: string) {
  return invoke<GameSnapshot>("rotate_block", { blockId });
}

export function selectEntity(id: string | null) {
  return invoke<GameSnapshot>("select_entity", { id });
}

export function openBehavior(behaviorId: string) {
  return invoke<BehaviorSource>("open_behavior", { behaviorId });
}

export function editBuiltinCopy(entityId: string) {
  return invoke<BehaviorSource>("edit_builtin_copy", { blockId: entityId });
}

export function forkBehavior(entityId: string) {
  return invoke<BehaviorSource>("fork_behavior", { blockId: entityId });
}

export function assignBehavior(entityId: string, behaviorId: string) {
  return invoke<GameSnapshot>("assign_behavior", { blockId: entityId, behaviorId });
}

export function saveBehavior(behaviorId: string, source: string) {
  return invoke<BehaviorSource>("save_behavior", { behaviorId, source });
}

export function buildBehavior(behaviorId: string) {
  return invoke<BuildResult>("build_behavior", { behaviorId });
}

export function saveWorld(slot = "quick") {
  return invoke<GameSnapshot>("save_world", { slot });
}

export function loadWorld(slot = "quick") {
  return invoke<GameSnapshot>("load_world", { slot });
}
