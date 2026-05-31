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

export function selectEntity(id: string | null) {
  return invoke<GameSnapshot>("select_entity", { id });
}

export function openBehavior(behaviorId: string) {
  return invoke<BehaviorSource>("open_behavior", { behaviorId });
}

export function editBuiltinCopy(blockId: string) {
  return invoke<BehaviorSource>("edit_builtin_copy", { blockId });
}

export function forkBehavior(blockId: string) {
  return invoke<BehaviorSource>("fork_behavior", { blockId });
}

export function saveBehavior(behaviorId: string, source: string) {
  return invoke<BehaviorSource>("save_behavior", { behaviorId, source });
}

export function buildBehavior(behaviorId: string) {
  return invoke<BuildResult>("build_behavior", { behaviorId });
}
