import { Box, Code2, Copy, GitBranch, Hammer, Pencil, RotateCw, Save, Trash2 } from "lucide-react";
import { displayItemKind } from "./itemMetadata";
import type {
  BehaviorRuntimeStats,
  BehaviorSource,
  BehaviorSummary,
  Block,
  BuildResult,
  Drone,
  Enemy,
  GameSnapshot,
  Inventory,
  ItemKind
} from "./types";

interface InspectorProps {
  snapshot: GameSnapshot | null;
  selectedBlock: Block | null;
  selectedEnemy: Enemy | null;
  selectedDrone: Drone | null;
  behavior: BehaviorSource | null;
  buildResult: BuildResult | null;
  dirty: boolean;
  compatibleBehaviors: BehaviorSummary[];
  onEditCopy: () => void;
  onFork: () => void;
  onSave: () => void;
  onBuild: () => void;
  onAssignBehavior: (behaviorId: string) => void;
  onOpenBehavior: (behaviorId: string) => void;
  onDeconstruct: () => void;
  onRotate: () => void;
}

export function Inspector({
  snapshot,
  selectedBlock,
  selectedEnemy,
  selectedDrone,
  behavior,
  buildResult,
  dirty,
  compatibleBehaviors,
  onEditCopy,
  onFork,
  onSave,
  onBuild,
  onAssignBehavior,
  onOpenBehavior,
  onDeconstruct,
  onRotate
}: InspectorProps) {
  const selectedNetwork = selectedBlock?.network_id
    ? snapshot?.networks.find((network) => network.id === selectedBlock.network_id)
    : null;
  const droneBehaviorRef = selectedDrone?.behavior_ref ?? null;
  const selectedBehaviorRef = selectedBlock?.behavior_ref ?? droneBehaviorRef;
  const selectedBehaviorSummary =
    compatibleBehaviors.find((item) => item.id === selectedBehaviorRef) ??
    (behavior?.summary.id === selectedBehaviorRef ? behavior.summary : null);
  const selectedBehaviorIsProject = selectedBehaviorSummary?.builtin === false;
  const EditBehaviorIcon = selectedBehaviorIsProject ? Pencil : Copy;
  const editBehaviorLabel = selectedBehaviorIsProject ? "Edit" : "Edit Copy";
  const editBehaviorTitle = selectedBehaviorIsProject ? "Edit project behavior" : "Copy built-in preset and edit";

  return (
    <section className="inspector">
      <div className="panel-heading">
        <Box size={16} />
        <span>Inspector</span>
      </div>
      {selectedBlock ? (
        <>
          <div className="kv">
            <span>ID</span>
            <strong>{selectedBlock.id}</strong>
            <span>Block</span>
            <strong>{selectedBlock.kind}</strong>
            <span>Status</span>
            <strong>{selectedBlock.status}</strong>
            <span>HP</span>
            <strong>{selectedBlock.hp}</strong>
            <span>CPU</span>
            <strong>{selectedBlock.effective_cpu_rate.toFixed(1)} fuel/s</strong>
            <span>Fuel Bank</span>
            <strong>{selectedBlock.fuel_bank.toFixed(1)}</strong>
            {selectedBlock.recipe && (
              <>
                <span>Recipe</span>
                <strong>{selectedBlock.recipe}</strong>
              </>
            )}
            {selectedBlock.target_id && (
              <>
                <span>Target</span>
                <strong>{selectedBlock.target_id}</strong>
              </>
            )}
            <span>Network</span>
            <strong>{selectedBlock.network_id ?? "local"}</strong>
          </div>
          <InventoryRows inventory={selectedBlock.inventory} emptyLabel="empty" />
          {selectedNetwork && (
            <div className="network-card">
              <span>network CPU {selectedNetwork.cpu_pool.toFixed(0)}</span>
              <span>active {selectedNetwork.active_devices}</span>
              <span>share {selectedNetwork.effective_per_device.toFixed(1)}</span>
            </div>
          )}
          {selectedBlock.behavior_runtime && <RuntimeStats stats={selectedBlock.behavior_runtime} />}
          {selectedBlock.behavior_ref && (
            <>
              <label className="behavior-picker">
                <span>Behavior</span>
                <select
                  aria-label="Assign behavior preset"
                  value={selectedBlock.behavior_ref}
                  onChange={(event) => onAssignBehavior(event.target.value)}
                >
                  {compatibleBehaviors.map((item) => (
                    <option key={item.id} value={item.id}>
                      {item.display_name}
                    </option>
                  ))}
                </select>
              </label>
              <div className="behavior-actions">
                <button onClick={() => onOpenBehavior(selectedBlock.behavior_ref!)} title="Open behavior">
                  <Code2 size={16} />
                  Open
                </button>
                <button onClick={onEditCopy} title={editBehaviorTitle}>
                  <EditBehaviorIcon size={16} />
                  {editBehaviorLabel}
                </button>
                <button onClick={onFork} title="Fork behavior for this block">
                  <GitBranch size={16} />
                  Fork
                </button>
              </div>
            </>
          )}
          {selectedBlock.kind !== "core" && (
            <div className="behavior-actions">
              <button onClick={onRotate} title="Rotate selected block clockwise" aria-label="Rotate selected block">
                <RotateCw size={16} />
                Rotate
              </button>
              <button className="danger" onClick={onDeconstruct} title="Deconstruct selected block">
                <Trash2 size={16} />
                Deconstruct
              </button>
            </div>
          )}
        </>
      ) : selectedEnemy ? (
        <>
          <div className="kv">
            <span>ID</span>
            <strong>{selectedEnemy.id}</strong>
            <span>Enemy</span>
            <strong>{selectedEnemy.kind}</strong>
            <span>HP</span>
            <strong>
              {selectedEnemy.hp}/{selectedEnemy.max_hp}
            </strong>
            <span>Position</span>
            <strong>
              {selectedEnemy.pos.x.toFixed(2)}, {selectedEnemy.pos.y.toFixed(2)}
            </strong>
            <span>Speed</span>
            <strong>{selectedEnemy.move_speed.toFixed(2)} tiles/tick</strong>
            <span>Cooldown</span>
            <strong>{selectedEnemy.attack_cooldown}</strong>
            <span>Target</span>
            <strong>{selectedEnemy.target_id ?? "none"}</strong>
          </div>
        </>
      ) : selectedDrone ? (
        <>
          <div className="kv">
            <span>ID</span>
            <strong>{selectedDrone.id}</strong>
            <span>Drone</span>
            <strong>{selectedDrone.state}</strong>
            <span>Home</span>
            <strong>{selectedDrone.home_port}</strong>
            <span>Battery</span>
            <strong>{selectedDrone.battery.toFixed(0)}%</strong>
            <span>Logic</span>
            <strong>{selectedDrone.logic_fuel}</strong>
            <span>Position</span>
            <strong>
              {selectedDrone.pos.x.toFixed(2)}, {selectedDrone.pos.y.toFixed(2)}
            </strong>
            <span>Job</span>
            <strong>
              {selectedDrone.job
                ? `${displayItemKind(selectedDrone.job.item)} ${selectedDrone.job.amount}`
                : "none"}
            </strong>
          </div>
          <InventoryRows inventory={selectedDrone.cargo} emptyLabel="empty cargo" />
          {selectedDrone.behavior_runtime && <RuntimeStats stats={selectedDrone.behavior_runtime} />}
          {droneBehaviorRef && (
            <>
              <label className="behavior-picker">
                <span>Behavior</span>
                <select
                  aria-label="Assign behavior preset"
                  value={droneBehaviorRef}
                  onChange={(event) => onAssignBehavior(event.target.value)}
                >
                  {compatibleBehaviors.map((item) => (
                    <option key={item.id} value={item.id}>
                      {item.display_name}
                    </option>
                  ))}
                </select>
              </label>
              <div className="behavior-actions">
                <button onClick={() => onOpenBehavior(droneBehaviorRef)} title="Open behavior">
                  <Code2 size={16} />
                  Open
                </button>
                <button onClick={onEditCopy} title={editBehaviorTitle}>
                  <EditBehaviorIcon size={16} />
                  {editBehaviorLabel}
                </button>
                <button onClick={onFork} title="Fork behavior for this drone">
                  <GitBranch size={16} />
                  Fork
                </button>
              </div>
            </>
          )}
        </>
      ) : (
        <p className="muted">Select a block, enemy, or drone to inspect its state.</p>
      )}

      {behavior && (
        <div className="behavior-meta">
          <div>
            <strong>{behavior.summary.display_name}</strong>
            <span>{behavior.summary.id}</span>
          </div>
          <div>
            <span>{behavior.summary.world}</span>
            <span>{behavior.summary.source_language}</span>
            <span>{behavior.summary.used_by} placements</span>
            <span>{behavior.summary.builtin ? "read-only preset" : "project behavior"}</span>
            <span>status {behavior.summary.build_status}</span>
            <span title={behavior.summary.source_path}>{behavior.summary.source_path}</span>
          </div>
          <div className="behavior-actions">
            <button onClick={onSave} disabled={!dirty || behavior.summary.builtin} title="Save source">
              <Save size={16} />
              Save
            </button>
            <button onClick={onBuild} disabled={behavior.summary.builtin} title="Build behavior">
              <Hammer size={16} />
              Build
            </button>
          </div>
          {buildResult && (
            <div className={buildResult.success ? "build-ok" : "build-fail"}>
              {buildResult.message}
              {buildResult.wasm_hash ? ` (${buildResult.wasm_hash.slice(0, 10)})` : ""}
            </div>
          )}
        </div>
      )}
    </section>
  );
}

function InventoryRows({ inventory, emptyLabel }: { inventory: Inventory; emptyLabel: string }) {
  const entries = Object.entries(inventory.items) as Array<[ItemKind, number]>;
  return (
    <div className="inventory">
      {entries.map(([kind, amount]) => (
        <span key={kind}>
          {displayItemKind(kind)}: {amount}
        </span>
      ))}
      {entries.length === 0 && <span>{emptyLabel}</span>}
    </div>
  );
}

function RuntimeStats({ stats }: { stats: BehaviorRuntimeStats }) {
  const hash = stats.wasm_hash ? stats.wasm_hash.slice(0, 10) : "none";
  const runtimeError = stats.runtime_error;
  const stateClass = runtimeError ? "runtime-error" : stats.over_budget ? "over-budget" : "";
  return (
    <div className={["runtime-card", stateClass].filter(Boolean).join(" ")}>
      <span>{runtimeError ? "runtime error" : stats.over_budget ? "over budget" : "budget ok"}</span>
      <span>runtime tick {stats.last_tick ?? "never"}</span>
      <span>runs {stats.run_count}</span>
      <span>
        fuel {stats.fuel_spent}/{stats.fuel_budget}
      </span>
      <span>left {stats.fuel_remaining}</span>
      <span>wasm {hash}</span>
      {runtimeError && <span>{runtimeError}</span>}
    </div>
  );
}
