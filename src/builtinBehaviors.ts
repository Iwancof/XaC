import assemblerBasicSource from "../assets/builtin/assembler/basic.xac?raw";
import carrierDroneBasicSource from "../assets/builtin/carrier_drone/basic.xac?raw";
import drillBasicSource from "../assets/builtin/drill/basic.xac?raw";
import dronePortBasicSource from "../assets/builtin/drone_port/basic.xac?raw";
import routerAmmoEastSource from "../assets/builtin/router/ammo_east.xac?raw";
import routerBasicSource from "../assets/builtin/router/basic.xac?raw";
import turretBasicSource from "../assets/builtin/turret/basic.xac?raw";
import turretPrioritySource from "../assets/builtin/turret/priority.xac?raw";
import type { BehaviorKind } from "./types";

export type BuiltinBehaviorPreset = {
  id: string;
  displayName: string;
  baseKind: BehaviorKind;
  world: string;
  sourcePath: string;
  source: string;
};

export const BUILTIN_BEHAVIOR_PRESETS: BuiltinBehaviorPreset[] = [
  {
    id: "builtin.drill.basic",
    displayName: "Basic Drill",
    baseKind: "drill",
    world: "drill-behavior",
    sourcePath: "assets/builtin/drill/basic.xac",
    source: drillBasicSource
  },
  {
    id: "builtin.router.basic",
    displayName: "Basic Router",
    baseKind: "router",
    world: "router-behavior",
    sourcePath: "assets/builtin/router/basic.xac",
    source: routerBasicSource
  },
  {
    id: "builtin.router.ammo_east",
    displayName: "Ammo East Router",
    baseKind: "router",
    world: "router-behavior",
    sourcePath: "assets/builtin/router/ammo_east.xac",
    source: routerAmmoEastSource
  },
  {
    id: "builtin.assembler.basic",
    displayName: "Basic Assembler",
    baseKind: "assembler",
    world: "assembler-behavior",
    sourcePath: "assets/builtin/assembler/basic.xac",
    source: assemblerBasicSource
  },
  {
    id: "builtin.turret.basic",
    displayName: "Basic Turret",
    baseKind: "turret",
    world: "turret-behavior",
    sourcePath: "assets/builtin/turret/basic.xac",
    source: turretBasicSource
  },
  {
    id: "builtin.turret.priority",
    displayName: "Priority Turret",
    baseKind: "turret",
    world: "turret-behavior",
    sourcePath: "assets/builtin/turret/priority.xac",
    source: turretPrioritySource
  },
  {
    id: "builtin.drone_port.basic",
    displayName: "Basic Drone Port",
    baseKind: "drone_port",
    world: "drone-port-behavior",
    sourcePath: "assets/builtin/drone_port/basic.xac",
    source: dronePortBasicSource
  },
  {
    id: "builtin.carrier_drone.basic",
    displayName: "Basic Carrier Drone",
    baseKind: "carrier_drone",
    world: "carrier-drone-behavior",
    sourcePath: "assets/builtin/carrier_drone/basic.xac",
    source: carrierDroneBasicSource
  }
];
