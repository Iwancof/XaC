import {
  Activity,
  Cpu,
  FastForward,
  Layers,
  Pause,
  Play,
  Radar,
  RotateCw,
  Route,
  StepForward,
  X
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  advance,
  assignBehavior,
  buildBehavior,
  deconstructBlock,
  editBuiltinCopy,
  forkBehavior,
  getSnapshot,
  openBehavior,
  placeBlock,
  rotateBlock,
  saveBehavior,
  selectEntity,
  setRunning,
  stepTicks
} from "./api";
import { CodeEditor } from "./CodeEditor";
import { GridWorld } from "./GridWorld";
import { Inspector } from "./Inspector";
import { PALETTE } from "./palette";
import type {
  BehaviorSource,
  Block,
  BlockKind,
  BuildResult,
  Direction,
  Drone,
  Enemy,
  GameSnapshot,
  ItemKind,
  Pos
} from "./types";

type Overlay = "none" | "network" | "cpu" | "logistics" | "attack";

const OVERLAYS: Array<{ id: Overlay; label: string; icon: typeof Layers }> = [
  { id: "none", label: "None", icon: Layers },
  { id: "network", label: "Network", icon: Activity },
  { id: "cpu", label: "CPU", icon: Cpu },
  { id: "logistics", label: "Logistics", icon: Route },
  { id: "attack", label: "Attack", icon: Radar }
];

const NEXT_DIRECTION: Record<Direction, Direction> = {
  north: "east",
  east: "south",
  south: "west",
  west: "north"
};

export function App() {
  const [snapshot, setSnapshot] = useState<GameSnapshot | null>(null);
  const [buildKind, setBuildKind] = useState<BlockKind | null>(null);
  const [direction, setDirection] = useState<Direction>("east");
  const [overlay, setOverlay] = useState<Overlay>("network");
  const [behavior, setBehavior] = useState<BehaviorSource | null>(null);
  const [editorValue, setEditorValue] = useState("");
  const [savedValue, setSavedValue] = useState("");
  const [buildResult, setBuildResult] = useState<BuildResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setSnapshot(await getSnapshot());
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const cancelPlacement = useCallback(() => {
    setBuildKind(null);
  }, []);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        cancelPlacement();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [cancelPlacement]);

  useEffect(() => {
    const handle = window.setInterval(async () => {
      try {
        setSnapshot(await advance(2));
      } catch (err) {
        setError(String(err));
      }
    }, 100);
    return () => window.clearInterval(handle);
  }, []);

  const selectedBlock = useMemo<Block | null>(() => {
    if (!snapshot?.selected_id) return null;
    return snapshot.blocks.find((block) => block.id === snapshot.selected_id) ?? null;
  }, [snapshot]);
  const selectedEnemy = useMemo<Enemy | null>(() => {
    if (!snapshot?.selected_id) return null;
    return snapshot.enemies.find((enemy) => enemy.id === snapshot.selected_id) ?? null;
  }, [snapshot]);
  const selectedDrone = useMemo<Drone | null>(() => {
    if (!snapshot?.selected_id) return null;
    return snapshot.drones.find((drone) => drone.id === snapshot.selected_id) ?? null;
  }, [snapshot]);
  const compatibleBehaviors = useMemo(() => {
    const behaviorKind = selectedBlock?.kind ?? (selectedDrone ? "carrier_drone" : null);
    if (!behaviorKind) return [];
    return snapshot?.behaviors.filter((item) => item.base_kind === behaviorKind) ?? [];
  }, [selectedBlock, selectedDrone, snapshot]);

  const dirty = behavior ? editorValue !== savedValue : false;

  const runCommand = async (task: () => Promise<GameSnapshot>) => {
    try {
      setSnapshot(await task());
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  };

  const loadBehavior = async (behaviorId: string) => {
    try {
      const next = await openBehavior(behaviorId);
      setBehavior(next);
      setEditorValue(next.source);
      setSavedValue(next.source);
      setBuildResult(null);
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  };

  const handleTileClick = (pos: Pos) => {
    if (!buildKind) return;
    runCommand(() => placeBlock(buildKind, pos.x, pos.y, direction));
  };

  const handleEntityClick = (id: string | null) => {
    runCommand(() => selectEntity(id));
  };

  const handleEditCopy = async () => {
    const entityId = selectedBlock?.id ?? selectedDrone?.id;
    if (!entityId) return;
    try {
      const next = await editBuiltinCopy(entityId);
      setBehavior(next);
      setEditorValue(next.source);
      setSavedValue(next.source);
      setBuildResult(null);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleFork = async () => {
    const entityId = selectedBlock?.id ?? selectedDrone?.id;
    if (!entityId) return;
    try {
      const next = await forkBehavior(entityId);
      setBehavior(next);
      setEditorValue(next.source);
      setSavedValue(next.source);
      setBuildResult(null);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleAssignBehavior = async (behaviorId: string) => {
    const entityId = selectedBlock?.id ?? selectedDrone?.id;
    if (!entityId) return;
    try {
      setSnapshot(await assignBehavior(entityId, behaviorId));
      setError(null);
      if (behavior && !dirty) {
        await loadBehavior(behaviorId);
      }
    } catch (err) {
      setError(String(err));
    }
  };

  const handleSave = async () => {
    if (!behavior) return;
    try {
      const next = await saveBehavior(behavior.summary.id, editorValue);
      setBehavior(next);
      setSavedValue(next.source);
      setError(null);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleBuild = async () => {
    if (!behavior) return;
    try {
      if (dirty) {
        await saveBehavior(behavior.summary.id, editorValue);
      }
      const result = await buildBehavior(behavior.summary.id);
      const next = await openBehavior(behavior.summary.id);
      setBehavior(next);
      setEditorValue(next.source);
      setBuildResult(result);
      setSavedValue(next.source);
      setError(null);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleDeconstruct = async () => {
    if (!selectedBlock) return;
    try {
      setSnapshot(await deconstructBlock(selectedBlock.id));
      setBehavior(null);
      setEditorValue("");
      setSavedValue("");
      setBuildResult(null);
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  };

  const handleRotateSelected = async () => {
    if (!selectedBlock) return;
    try {
      setSnapshot(await rotateBlock(selectedBlock.id));
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  };

  const rotatePlacementDirection = () => {
    setDirection((current) => NEXT_DIRECTION[current]);
  };

  const core = snapshot?.blocks.find((block) => block.kind === "core");
  const coreItemCount = (item: ItemKind) => core?.inventory.items[item] ?? 0;
  const selectedBehaviorId = selectedBlock?.behavior_ref ?? selectedDrone?.behavior_ref ?? null;

  return (
    <main className="app-shell">
      <header className="topbar">
        <div className="brand">
          <span>XaC</span>
          <strong>RTS as Code MVP</strong>
        </div>
        <div className="topbar-controls">
          <button onClick={() => runCommand(() => setRunning(!(snapshot?.running ?? false)))}>
            {snapshot?.running ? <Pause size={16} /> : <Play size={16} />}
            {snapshot?.running ? "Pause" : "Run"}
          </button>
          <button onClick={() => runCommand(() => stepTicks(1))}>
            <StepForward size={16} />
            Tick
          </button>
          <button onClick={() => runCommand(() => stepTicks(40))}>
            <FastForward size={16} />
            +40
          </button>
        </div>
        <div className="metrics">
          <span>tick {snapshot?.tick ?? 0}</span>
          <span>wave {snapshot?.status.wave ?? 1}</span>
          <span>next {snapshot?.status.next_wave_in ?? 0}</span>
          <span>
            core HP {snapshot?.status.core_hp ?? 0}/{snapshot?.status.core_max_hp ?? 0}
          </span>
          {snapshot?.status.defeated && <span>DEFEATED</span>}
          <span>blocks {snapshot?.blocks.length ?? 0}</span>
          <span>enemies {snapshot?.enemies.length ?? 0}</span>
          <span>wire {snapshot?.status.wire_threats ?? 0}</span>
          <span>damage {snapshot?.status.damaged_wires ?? 0}</span>
          <span>net CPU {snapshot?.status.network_cpu.toFixed(0) ?? "0"}</span>
          <span>core ore {coreItemCount("ore")}</span>
          <span>core plate {coreItemCount("plate")}</span>
          <span>core ammo {coreItemCount("ammo")}</span>
        </div>
      </header>

      <section className="workspace">
        <div className="world-pane">
          <GridWorld
            snapshot={snapshot}
            selectedId={snapshot?.selected_id ?? null}
            buildKind={buildKind}
            direction={direction}
            overlay={overlay}
            onTileClick={handleTileClick}
            onEntityClick={handleEntityClick}
          />
        </div>

        <aside className="right-pane">
          <section className="block-list-panel">
            <div className="panel-heading">
              <span>Blocks</span>
              <strong>{buildKind ? `Placing ${buildKind.replaceAll("_", " ")}` : "Select to place"}</strong>
            </div>
            <div className="block-list">
              {PALETTE.map((item) => (
                <button
                  key={item.kind}
                  className={buildKind === item.kind ? "block-item selected" : "block-item"}
                  onClick={() => {
                    setBuildKind(buildKind === item.kind ? null : item.kind);
                    if (item.dir) setDirection(item.dir);
                  }}
                  title={`${item.category}: ${item.label}`}
                >
                  <span>{item.label}</span>
                  <small>{item.category}</small>
                </button>
              ))}
            </div>
            <div className="placement-controls">
              <span>Direction</span>
              <select value={direction} onChange={(event) => setDirection(event.target.value as Direction)}>
                <option value="north">North</option>
                <option value="east">East</option>
                <option value="south">South</option>
                <option value="west">West</option>
              </select>
              <button
                onClick={rotatePlacementDirection}
                title="Rotate placement direction clockwise"
                aria-label="Rotate placement direction"
              >
                <RotateCw size={16} />
                Rotate
              </button>
              <button
                onClick={cancelPlacement}
                disabled={!buildKind}
                title="Cancel placement mode"
                aria-label="Cancel placement"
              >
                <X size={16} />
                Cancel
              </button>
            </div>
          </section>

          <Inspector
            snapshot={snapshot}
            selectedBlock={selectedBlock}
            selectedEnemy={selectedEnemy}
            selectedDrone={selectedDrone}
            behavior={behavior}
            buildResult={buildResult}
            dirty={dirty}
            compatibleBehaviors={compatibleBehaviors}
            onEditCopy={handleEditCopy}
            onFork={handleFork}
            onSave={handleSave}
            onBuild={handleBuild}
            onAssignBehavior={handleAssignBehavior}
            onOpenBehavior={loadBehavior}
            onDeconstruct={handleDeconstruct}
            onRotate={handleRotateSelected}
          />
          <section className="editor-panel">
            <div className="panel-heading">
              <span>Code Editor</span>
              {selectedBehaviorId && !behavior && (
                <button onClick={() => loadBehavior(selectedBehaviorId)}>Open Selected Behavior</button>
              )}
            </div>
            {behavior ? (
              <CodeEditor value={editorValue} onChange={setEditorValue} />
            ) : (
              <div className="empty-editor">Open a behavior from the inspector.</div>
            )}
          </section>
        </aside>
      </section>

      <footer className="bottom-bar">
        <section className="overlay-controls">
          {OVERLAYS.map(({ id, label, icon: Icon }) => (
            <button key={id} className={overlay === id ? "selected" : ""} onClick={() => setOverlay(id)}>
              <Icon size={15} />
              {label}
            </button>
          ))}
        </section>

        <section className="log-panel">
          {error && <div className="log error">UI: {error}</div>}
          {snapshot?.logs
            .slice(-6)
            .reverse()
            .map((entry, index) => (
              <div className={`log ${entry.level}`} key={`${entry.tick}-${entry.source}-${index}`}>
                <span>{entry.tick}</span>
                <strong>{entry.source}</strong>
                {entry.message}
              </div>
            ))}
        </section>
      </footer>
    </main>
  );
}
