import {
  FastForward,
  FolderOpen,
  Pause,
  Play,
  Save,
  StepForward
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  advance,
  assignBehavior,
  buildBehavior,
  deconstructBlock,
  editBuiltinCopy,
  forkBehavior,
  getCommonTemplates,
  getSnapshot,
  loadWorld,
  openBehavior,
  placeBlock,
  placeBlocks,
  rotateBlock,
  saveBehavior,
  saveWorld,
  selectEntity,
  setRunning,
  stepTicks
} from "./api";
import { BuildPalette } from "./BuildPalette";
import { CodeEditor } from "./CodeEditor";
import { GridWorld } from "./GridWorld";
import { Inspector } from "./Inspector";
import { LogisticsPanel } from "./LogisticsPanel";
import { OverlayDetails } from "./OverlayDetails";
import { OVERLAYS, type Overlay } from "./overlays";
import { TutorialPanel } from "./TutorialPanel";
import type {
  BehaviorSource,
  Block,
  BlockKind,
  BuildResult,
  CommonTemplate,
  Direction,
  Drone,
  Enemy,
  GameSnapshot,
  ItemKind,
  Pos
} from "./types";

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
  const [templates, setTemplates] = useState<CommonTemplate[]>([]);
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
    getCommonTemplates()
      .then(setTemplates)
      .catch((err) => setError(String(err)));
  }, []);

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

  const handleTilesPaint = (positions: Pos[], paintDirection: Direction) => {
    if (!buildKind || positions.length === 0) return;
    setDirection(paintDirection);
    if (positions.length === 1) {
      runCommand(() => placeBlock(buildKind, positions[0].x, positions[0].y, paintDirection));
      return;
    }
    runCommand(() => placeBlocks(buildKind, positions, paintDirection));
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

  const handleSaveWorld = async () => {
    await runCommand(() => saveWorld("quick"));
  };

  const handleLoadWorld = async () => {
    try {
      setSnapshot(await loadWorld("quick"));
      setBehavior(null);
      setEditorValue("");
      setSavedValue("");
      setBuildResult(null);
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
          <button onClick={handleSaveWorld}>
            <Save size={16} />
            Save World
          </button>
          <button onClick={handleLoadWorld}>
            <FolderOpen size={16} />
            Load World
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
          <span>jobs {snapshot?.pending_jobs.length ?? 0}</span>
          <span>flows {snapshot?.item_flows.length ?? 0}</span>
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
            onTilesPaint={handleTilesPaint}
            onPaintDirectionChange={setDirection}
            onEntityClick={handleEntityClick}
          />
        </div>

        <aside className="right-pane">
          <BuildPalette
            snapshot={snapshot}
            buildKind={buildKind}
            direction={direction}
            onSelectBlock={(kind, defaultDirection) => {
              setBuildKind(kind);
              if (defaultDirection) setDirection(defaultDirection);
            }}
            onDirectionChange={setDirection}
            onRotateDirection={rotatePlacementDirection}
            onCancelPlacement={cancelPlacement}
          />

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
              <strong>{templates.length} templates</strong>
              {selectedBehaviorId && !behavior && (
                <button onClick={() => loadBehavior(selectedBehaviorId)}>Open Selected Behavior</button>
              )}
            </div>
            {behavior ? (
              <CodeEditor value={editorValue} onChange={setEditorValue} />
            ) : (
              <div className="empty-editor">
                <div className="template-list" data-testid="template-list">
                  {templates.map((template) => (
                    <div className="template-row" key={template.id}>
                      <strong>{template.display_name}</strong>
                      <span>{template.language}</span>
                      <small>{template.source_path}</small>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </section>
        </aside>
      </section>

      <footer className="bottom-bar">
        <div className="bottom-tools">
          <section className="overlay-controls">
            {OVERLAYS.map(({ id, label, icon: Icon }) => (
              <button key={id} className={overlay === id ? "selected" : ""} onClick={() => setOverlay(id)}>
                <Icon size={15} />
                {label}
              </button>
            ))}
          </section>
          <OverlayDetails snapshot={snapshot} overlay={overlay} />
          <TutorialPanel snapshot={snapshot} />
        </div>

        <LogisticsPanel snapshot={snapshot} />

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
