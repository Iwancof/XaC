import { Activity, Cpu, Layers, Radar, Route, type LucideIcon } from "lucide-react";

export type Overlay = "none" | "network" | "cpu" | "logistics" | "attack";

export const OVERLAYS: Array<{ id: Overlay; label: string; icon: LucideIcon }> = [
  { id: "none", label: "None", icon: Layers },
  { id: "network", label: "Network", icon: Activity },
  { id: "cpu", label: "CPU", icon: Cpu },
  { id: "logistics", label: "Logistics", icon: Route },
  { id: "attack", label: "Attack", icon: Radar }
];

export function overlayLabel(overlay: Overlay) {
  return OVERLAYS.find((item) => item.id === overlay)?.label ?? overlay;
}
