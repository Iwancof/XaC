import type { BuildPaletteItem } from "./types";

export const PALETTE: BuildPaletteItem[] = [
  { kind: "wire", label: "Wire", category: "Network" },
  { kind: "cpu_node", label: "CPU Node", category: "Network" },
  { kind: "drill", label: "Drill", category: "Mining", dir: "east" },
  { kind: "conveyor", label: "Conveyor", category: "Logistics", dir: "east" },
  { kind: "router", label: "Router", category: "Logistics" },
  { kind: "storage", label: "Storage", category: "Logistics" },
  { kind: "assembler", label: "Assembler", category: "Production", dir: "east" },
  { kind: "turret", label: "Turret", category: "Defense" },
  { kind: "drone_port", label: "Drone Port", category: "Drones" }
];
