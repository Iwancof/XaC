import type { BuildPaletteItem } from "./types";

export const PALETTE: BuildPaletteItem[] = [
  { kind: "core", label: "Core", category: "Base" },
  { kind: "drill", label: "Ore Drill", category: "Mining", dir: "east" },
  { kind: "conveyor", label: "Belt Conveyor", category: "Logistics", dir: "east" },
  { kind: "wire", label: "Wire", category: "Network" },
  { kind: "cpu_node", label: "CPU Node", category: "Network" },
  { kind: "router", label: "Router", category: "Logistics" },
  { kind: "storage", label: "Storage", category: "Logistics" },
  { kind: "assembler", label: "Assembler", category: "Production", dir: "east" },
  { kind: "turret", label: "Turret", category: "Defense" },
  { kind: "drone_port", label: "Drone Port", category: "Drones" }
];
