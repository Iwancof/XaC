import {
  Archive,
  Blocks,
  Cpu,
  Crosshair,
  Factory,
  MoveRight,
  Network,
  Pickaxe,
  RadioTower,
  RotateCw,
  X,
  type LucideIcon
} from "lucide-react";
import { useMemo, useState } from "react";
import {
  blockFootprintSize,
  blockInventoryCapacity,
  blockLocalCpuRate,
  blockMaxHp,
  blockNetworkCpuOutput,
  isNetworkConnector,
  isProgrammableBlock
} from "./gameMetadata";
import { PALETTE } from "./palette";
import type { BlockKind, Direction, GameSnapshot } from "./types";

type BuildCategoryId = "factory" | "production" | "distribution" | "logic" | "crafting" | "turret" | "units";

type BuildCategory = {
  id: BuildCategoryId;
  label: string;
  icon: LucideIcon;
};

const CATEGORIES: BuildCategory[] = [
  { id: "factory", label: "Factory", icon: Blocks },
  { id: "production", label: "Production", icon: Pickaxe },
  { id: "distribution", label: "Distribution", icon: MoveRight },
  { id: "logic", label: "Logic", icon: Network },
  { id: "crafting", label: "Crafting", icon: Factory },
  { id: "turret", label: "Turret", icon: Crosshair },
  { id: "units", label: "Units", icon: RadioTower }
];

const BLOCK_ICONS: Record<BlockKind, LucideIcon> = {
  core: Blocks,
  drill: Pickaxe,
  conveyor: MoveRight,
  wire: Network,
  cpu_node: Cpu,
  router: MoveRight,
  storage: Archive,
  assembler: Factory,
  turret: Crosshair,
  drone_port: RadioTower
};

const CATEGORY_BY_KIND: Partial<Record<BlockKind, BuildCategoryId>> = {
  drill: "production",
  conveyor: "distribution",
  router: "distribution",
  storage: "distribution",
  wire: "logic",
  cpu_node: "logic",
  assembler: "crafting",
  turret: "turret",
  drone_port: "units"
};

interface BuildPaletteProps {
  snapshot: GameSnapshot | null;
  buildKind: BlockKind | null;
  direction: Direction;
  onSelectBlock: (kind: BlockKind | null, defaultDirection?: Direction) => void;
  onDirectionChange: (direction: Direction) => void;
  onRotateDirection: () => void;
  onCancelPlacement: () => void;
}

export function BuildPalette({
  snapshot,
  buildKind,
  direction,
  onSelectBlock,
  onDirectionChange,
  onRotateDirection,
  onCancelPlacement
}: BuildPaletteProps) {
  const [category, setCategory] = useState<BuildCategoryId>("factory");
  const selectedItem = PALETTE.find((item) => item.kind === buildKind) ?? null;
  const selectedCategory = selectedItem ? CATEGORY_BY_KIND[selectedItem.kind] ?? "factory" : category;
  const visibleItems = useMemo(() => {
    if (category === "factory") return PALETTE;
    return PALETTE.filter((item) => CATEGORY_BY_KIND[item.kind] === category);
  }, [category]);
  const SelectedIcon = selectedItem ? BLOCK_ICONS[selectedItem.kind] : Blocks;
  const core = snapshot?.blocks.find((block) => block.kind === "core");
  const coreOre = core?.inventory.items.ore ?? 0;

  return (
    <section className="build-fragment" data-testid="build-fragment">
      <div className="build-info">
        <div className={`build-preview ${selectedItem ? `kind-${selectedItem.kind}` : ""}`}>
          <SelectedIcon size={24} />
        </div>
        <div className="build-info-copy">
          <div className="build-kicker">
            <span>Blocks</span>
            <span>{categoryLabel(selectedCategory)}</span>
          </div>
          <strong>{selectedItem?.label ?? "Select to place"}</strong>
          <small>
            {selectedItem
              ? `Placing ${selectedItem.kind.replaceAll("_", " ")} ${directionGlyph(direction)} / ${selectedStats(selectedItem.kind)}`
              : "Factory blocks stay selected for rapid line placement."}
          </small>
        </div>
        <button className="square-button" onClick={onRotateDirection} disabled={!buildKind} title="Rotate placement">
          <RotateCw size={16} />
        </button>
        <button className="square-button" onClick={onCancelPlacement} disabled={!buildKind} title="Cancel placement">
          <X size={16} />
        </button>
      </div>

      <div className="build-body">
        <div className="build-grid" role="group" aria-label="Build blocks">
          {visibleItems.map((item, index) => {
            const Icon = BLOCK_ICONS[item.kind];
            return (
              <button
                key={item.kind}
                className={buildKind === item.kind ? `build-tile selected kind-${item.kind}` : `build-tile kind-${item.kind}`}
                data-testid={`build-tile-${item.kind}`}
                onClick={() => onSelectBlock(buildKind === item.kind ? null : item.kind, item.dir)}
                title={`${item.label} / ${categoryLabel(CATEGORY_BY_KIND[item.kind] ?? "factory")}`}
              >
                <span className="tile-index">{index + 1}</span>
                <span className="tile-icon">
                  <Icon size={18} />
                </span>
                <strong>{item.label}</strong>
              </button>
            );
          })}
        </div>

        <div className="category-rail" role="group" aria-label="Build categories">
          {CATEGORIES.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              className={category === id ? "category-button selected" : "category-button"}
              data-testid={`build-category-${id}`}
              onClick={() => setCategory(id)}
              title={label}
              aria-label={`Category ${label}`}
            >
              <Icon size={17} />
            </button>
          ))}
        </div>
      </div>

      <div className="build-footer">
        <div className="direction-pad" role="group" aria-label="Placement direction">
          {(["north", "east", "south", "west"] as Direction[]).map((dir) => (
            <button
              key={dir}
              className={direction === dir ? "selected" : ""}
              onClick={() => onDirectionChange(dir)}
              title={`Face ${dir}`}
              aria-label={`Face ${dir}`}
            >
              {directionGlyph(dir)}
            </button>
          ))}
        </div>
        <div className="build-readout">
          <span>core ore {coreOre}</span>
          <span>{directionGlyph(direction)} {direction}</span>
        </div>
      </div>
    </section>
  );
}

function selectedStats(kind: BlockKind) {
  const [width, height] = blockFootprintSize(kind);
  const stats = [`${width}x${height}`, `${blockMaxHp(kind)} HP`];
  const capacity = blockInventoryCapacity(kind);
  const localCpu = blockLocalCpuRate(kind);
  const networkCpu = blockNetworkCpuOutput(kind);
  if (capacity > 0) stats.push(`${capacity} store`);
  if (networkCpu > 0) stats.push(`+${networkCpu} CPU`);
  if (localCpu > 0) stats.push(`${localCpu} fuel/s`);
  if (isProgrammableBlock(kind)) stats.push("WASM");
  if (isNetworkConnector(kind)) stats.push("net");
  return stats.join(" / ");
}

function categoryLabel(id: BuildCategoryId) {
  return CATEGORIES.find((category) => category.id === id)?.label ?? "Factory";
}

function directionGlyph(direction: Direction) {
  return { north: "^", east: ">", south: "v", west: "<" }[direction];
}
