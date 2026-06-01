import { Box, Code2, Copy, GitBranch, Hammer, RotateCw, Save, Trash2 } from "lucide-react";
import type { BehaviorSource, BehaviorSummary, Block, BuildResult, Drone, Enemy, GameSnapshot } from "./types";

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
            {selectedBlock.recipe && (
              <>
                <span>Recipe</span>
                <strong>{selectedBlock.recipe}</strong>
              </>
            )}
            <span>Network</span>
            <strong>{selectedBlock.network_id ?? "local"}</strong>
          </div>
          <div className="inventory">
            {Object.entries(selectedBlock.inventory.items).map(([kind, amount]) => (
              <span key={kind}>
                {kind}: {amount}
              </span>
            ))}
            {Object.keys(selectedBlock.inventory.items).length === 0 && <span>empty</span>}
          </div>
          {selectedNetwork && (
            <div className="network-card">
              <span>network CPU {selectedNetwork.cpu_pool.toFixed(0)}</span>
              <span>active {selectedNetwork.active_devices}</span>
              <span>share {selectedNetwork.effective_per_device.toFixed(1)}</span>
            </div>
          )}
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
                <button onClick={onEditCopy} title="Copy built-in preset and edit">
                  <Copy size={16} />
                  Edit Copy
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
            <strong>{selectedDrone.job ? `${selectedDrone.job.item} ${selectedDrone.job.amount}` : "none"}</strong>
          </div>
          <div className="inventory">
            {Object.entries(selectedDrone.cargo.items).map(([kind, amount]) => (
              <span key={kind}>
                {kind}: {amount}
              </span>
            ))}
            {Object.keys(selectedDrone.cargo.items).length === 0 && <span>empty cargo</span>}
          </div>
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
                <button onClick={onEditCopy} title="Copy built-in preset and edit">
                  <Copy size={16} />
                  Edit Copy
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
            <span>{behavior.summary.used_by} placements</span>
            <span>{behavior.summary.builtin ? "read-only preset" : "project behavior"}</span>
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
