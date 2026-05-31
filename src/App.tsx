import {
  Activity,
  Cpu,
  FastForward,
  Layers,
  Pause,
  Play,
  Radar,
  Route,
  StepForward
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  advance,
  buildBehavior,
  deconstructBlock,
  editBuiltinCopy,
  forkBehavior,
  getSnapshot,
  openBehavior,
  placeBlock,
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
  GameSnapshot,
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
    if (!selectedBlock) return;
    try {
      const next = await editBuiltinCopy(selectedBlock.id);
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
    if (!selectedBlock) return;
    try {
      const next = await forkBehavior(selectedBlock.id);
      setBehavior(next);
      setEditorValue(next.source);
      setSavedValue(next.source);
      setBuildResult(null);
      await refresh();
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
      setBuildResult(result);
      setSavedValue(editorValue);
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

  const core = snapshot?.blocks.find((block) => block.kind === "core");
  const selectedBehaviorId = selectedBlock?.behavior_ref ?? null;

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
          <span>blocks {snapshot?.blocks.length ?? 0}</span>
          <span>enemies {snapshot?.enemies.length ?? 0}</span>
          <span>wire {snapshot?.status.wire_threats ?? 0}</span>
          <span>damage {snapshot?.status.damaged_wires ?? 0}</span>
          <span>net CPU {snapshot?.status.network_cpu.toFixed(0) ?? "0"}</span>
          <span>core ammo {core?.inventory.items.ammo ?? 0}</span>
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
            </div>
          </section>

          <Inspector
            snapshot={snapshot}
            selectedBlock={selectedBlock}
            behavior={behavior}
            buildResult={buildResult}
            dirty={dirty}
            onEditCopy={handleEditCopy}
            onFork={handleFork}
            onSave={handleSave}
            onBuild={handleBuild}
            onOpenBehavior={loadBehavior}
            onDeconstruct={handleDeconstruct}
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
