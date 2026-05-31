import { Box, Code2, Copy, GitBranch, Hammer, Save } from "lucide-react";
import type { BehaviorSource, Block, BuildResult, GameSnapshot } from "./types";

interface InspectorProps {
  snapshot: GameSnapshot | null;
  selectedBlock: Block | null;
  behavior: BehaviorSource | null;
  buildResult: BuildResult | null;
  dirty: boolean;
  onEditCopy: () => void;
  onFork: () => void;
  onSave: () => void;
  onBuild: () => void;
  onOpenBehavior: (behaviorId: string) => void;
}

export function Inspector({
  snapshot,
  selectedBlock,
  behavior,
  buildResult,
  dirty,
  onEditCopy,
  onFork,
  onSave,
  onBuild,
  onOpenBehavior
}: InspectorProps) {
  const selectedNetwork = selectedBlock?.network_id
    ? snapshot?.networks.find((network) => network.id === selectedBlock.network_id)
    : null;

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
          )}
        </>
      ) : (
        <p className="muted">Select a block to inspect code, CPU, inventory, and network state.</p>
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
