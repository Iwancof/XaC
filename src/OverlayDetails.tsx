import { Activity, Cpu, Radar, Route } from "lucide-react";
import { displayItemKind } from "./itemMetadata";
import { overlayLabel, type Overlay } from "./overlays";
import type { Block, GameSnapshot } from "./types";

interface OverlayDetailsProps {
  snapshot: GameSnapshot | null;
  overlay: Overlay;
}

export function OverlayDetails({ snapshot, overlay }: OverlayDetailsProps) {
  const Icon = overlayIcon(overlay);
  return (
    <section className="overlay-details" data-testid="overlay-details" aria-label={`${overlayLabel(overlay)} overlay details`}>
      <div className="overlay-detail-heading">
        <span>
          <Icon size={13} />
          {overlayLabel(overlay)}
        </span>
        <strong>{snapshot ? overlaySummary(snapshot, overlay) : "waiting"}</strong>
      </div>
      {snapshot ? overlayRows(snapshot, overlay) : <div className="overlay-empty">No world snapshot</div>}
    </section>
  );
}

function overlayRows(snapshot: GameSnapshot, overlay: Overlay) {
  if (overlay === "network") {
    if (snapshot.networks.length === 0) {
      return <div className="overlay-empty">No networks</div>;
    }
    return (
      <div className="overlay-detail-list">
        {snapshot.networks.map((network) => (
          <div className="overlay-detail-row" key={network.id}>
            <strong>net {network.id}</strong>
            <span>CPU {network.cpu_pool.toFixed(0)}</span>
            <span>active {network.active_devices}</span>
            <small>
              {network.read_only_cache ? "cache" : "core"} / {network.block_ids.length} blocks
            </small>
          </div>
        ))}
      </div>
    );
  }

  if (overlay === "cpu") {
    const cpuBlocks = snapshot.blocks
      .filter((block) => block.active && (block.behavior_ref || block.effective_cpu_rate > 0))
      .sort(compareCpuBlocks);
    if (cpuBlocks.length === 0) {
      return <div className="overlay-empty">No active CPU devices</div>;
    }
    return (
      <div className="overlay-detail-list">
        {cpuBlocks.slice(0, 5).map((block) => (
          <div className="overlay-detail-row" key={block.id}>
            <strong>{block.id}</strong>
            <span>{block.effective_cpu_rate.toFixed(1)} fuel/s</span>
            <small>network {block.network_id ?? "local"}</small>
          </div>
        ))}
      </div>
    );
  }

  if (overlay === "logistics") {
    const activeDrones = snapshot.drones.filter((drone) => drone.job);
    if (snapshot.pending_jobs.length === 0 && activeDrones.length === 0) {
      return <div className="overlay-empty">No active deliveries</div>;
    }
    return (
      <div className="overlay-detail-list">
        {snapshot.pending_jobs.slice(0, 3).map((job) => (
          <div className="overlay-detail-row" key={job.id}>
            <strong>{job.id}</strong>
            <span>
              {displayItemKind(job.item)} x{job.amount}
            </span>
            <small>
              {job.pickup}
              {" -> "}
              {job.dropoff}
            </small>
          </div>
        ))}
        {activeDrones.slice(0, 3).map((drone) => (
          <div className="overlay-detail-row" key={drone.id}>
            <strong>{drone.id}</strong>
            <span>{drone.state}</span>
            <small>{drone.job ? `${drone.job.pickup} -> ${drone.job.dropoff}` : "idle"}</small>
          </div>
        ))}
      </div>
    );
  }

  if (overlay === "attack") {
    const turrets = snapshot.blocks.filter((block) => block.kind === "turret");
    if (turrets.length === 0) {
      return <div className="overlay-empty">No turrets</div>;
    }
    return (
      <div className="overlay-detail-list">
        {turrets.slice(0, 5).map((turret) => (
          <div className="overlay-detail-row" key={turret.id}>
            <strong>{turret.id}</strong>
            <span>{turret.target_id ? `target ${turret.target_id}` : "no target"}</span>
            <small>{ammoSummary(turret)}</small>
          </div>
        ))}
      </div>
    );
  }

  return <div className="overlay-empty">Overlay off</div>;
}

function overlaySummary(snapshot: GameSnapshot, overlay: Overlay) {
  if (overlay === "network") {
    return `${snapshot.networks.length} networks`;
  }
  if (overlay === "cpu") {
    const activeCpu = snapshot.blocks.filter((block) => block.active && block.effective_cpu_rate > 0);
    return `${activeCpu.length} devices`;
  }
  if (overlay === "logistics") {
    return `${snapshot.pending_jobs.length} jobs / ${snapshot.drones.length} drones`;
  }
  if (overlay === "attack") {
    const armed = snapshot.blocks.filter((block) => block.kind === "turret");
    return `${armed.length} turrets`;
  }
  return "off";
}

function compareCpuBlocks(a: Block, b: Block) {
  if (b.effective_cpu_rate !== a.effective_cpu_rate) {
    return b.effective_cpu_rate - a.effective_cpu_rate;
  }
  return a.id.localeCompare(b.id);
}

function ammoSummary(block: Block) {
  const ammo = block.inventory.items.ammo ?? 0;
  return `${ammo} ammo`;
}

function overlayIcon(overlay: Overlay) {
  if (overlay === "network") return Activity;
  if (overlay === "cpu") return Cpu;
  if (overlay === "logistics") return Route;
  if (overlay === "attack") return Radar;
  return Activity;
}
