import { Route } from "lucide-react";
import type { GameSnapshot, ItemFlowEvent } from "./types";

interface LogisticsPanelProps {
  snapshot: GameSnapshot | null;
}

export function LogisticsPanel({ snapshot }: LogisticsPanelProps) {
  const jobs = snapshot?.pending_jobs.slice(0, 3) ?? [];
  const flows = snapshot?.item_flows.slice(-5).reverse() ?? [];

  return (
    <section className="logistics-panel" aria-label="Logistics">
      <div className="logistics-heading">
        <span>
          <Route size={15} />
          Logistics
        </span>
        <strong>
          {snapshot?.pending_jobs.length ?? 0} jobs / {snapshot?.item_flows.length ?? 0} flows
        </strong>
      </div>
      <div className="logistics-list">
        {jobs.map((job) => (
          <div className="logistics-row job" key={job.id}>
            <span>{job.id}</span>
            <strong>
              {job.item} x{job.amount}
            </strong>
            <small>
              {job.pickup}
              {" -> "}
              {job.dropoff}
            </small>
          </div>
        ))}
        {flows.map((flow) => (
          <div className="logistics-row flow" key={flow.id}>
            <span>{flow.tick}</span>
            <strong>{flowLabel(flow)}</strong>
            <small>
              {flow.from_entity}
              {" -> "}
              {flow.to_entity}
            </small>
          </div>
        ))}
        {jobs.length === 0 && flows.length === 0 && <div className="logistics-empty">No active logistics</div>}
      </div>
    </section>
  );
}

function flowLabel(flow: ItemFlowEvent) {
  return `${flow.item} x${flow.amount}`;
}
