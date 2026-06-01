import { CheckCircle2, CircleDashed, Target } from "lucide-react";
import type { Block, GameSnapshot, ItemFlowEvent } from "./types";

type TutorialStep = {
  id: string;
  label: string;
  detail: string;
  complete: boolean;
};

export function TutorialPanel({ snapshot }: { snapshot: GameSnapshot | null }) {
  const steps = snapshot ? buildTutorialSteps(snapshot) : emptyTutorialSteps();
  const completeCount = steps.filter((step) => step.complete).length;
  const nextStep = steps.find((step) => !step.complete) ?? steps.at(-1);

  return (
    <section className="tutorial-panel" data-testid="tutorial-panel">
      <div className="tutorial-heading">
        <span>
          <Target size={14} />
          Objectives
        </span>
        <strong data-testid="tutorial-progress">
          {completeCount}/{steps.length}
        </strong>
      </div>
      {nextStep && (
        <div className="tutorial-next">
          <span>Next</span>
          <strong>{nextStep.label}</strong>
        </div>
      )}
      <div className="tutorial-list">
        {steps.map((step) => (
          <div
            className={step.complete ? "tutorial-step complete" : "tutorial-step"}
            data-state={step.complete ? "complete" : "pending"}
            data-testid={`tutorial-${step.id}`}
            key={step.id}
          >
            {step.complete ? <CheckCircle2 size={14} /> : <CircleDashed size={14} />}
            <div>
              <strong>{step.label}</strong>
              <span>{step.detail}</span>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function buildTutorialSteps(snapshot: GameSnapshot): TutorialStep[] {
  const blocks = snapshot.blocks;
  const core = blocks.find((block) => block.kind === "core");

  return [
    {
      id: "mining-loop",
      label: "Mine ore to core",
      detail: "Drill, belt, core",
      complete:
        hasBlock(blocks, "drill") &&
        hasBlock(blocks, "conveyor") &&
        ((core?.inventory.items.ore ?? 0) > 40 ||
          hasFlow(snapshot.item_flows, (flow) => flow.item === "ore" && flow.to_entity === core?.id))
    },
    {
      id: "cpu-network",
      label: "Boost with CPU",
      detail: "Wire joins core CPU",
      complete: snapshot.networks.some(
        (network) =>
          network.cpu_pool >= 200 &&
          network.block_ids.some((id) => id.startsWith("core_")) &&
          network.block_ids.some((id) => id.startsWith("cpu_node_"))
      )
    },
    {
      id: "edit-code",
      label: "Edit behavior code",
      detail: "Copy, save, build",
      complete: snapshot.behaviors.some((behavior) => !behavior.builtin)
    },
    {
      id: "ammo-production",
      label: "Produce ammo",
      detail: "Assembler output",
      complete:
        blocks.some((block) => block.kind === "assembler" && (block.inventory.items.ammo ?? 0) > 0) ||
        hasFlow(snapshot.item_flows, (flow) => flow.item === "ammo" && flow.from_entity.startsWith("assembler_"))
    },
    {
      id: "defense",
      label: "Run compiled code",
      detail: "Wasm behavior spends fuel",
      complete:
        blocks.some((block) => (block.behavior_runtime?.run_count ?? 0) > 0) ||
        blocks.some((block) => block.kind === "turret" && block.target_id) ||
        snapshot.logs.some((entry) => entry.source.startsWith("turret_") && entry.message.startsWith("attacking"))
    },
    {
      id: "drone-delivery",
      label: "Drone ammo delivery",
      detail: "Port to frontline",
      complete:
        snapshot.drones.length > 0 &&
        hasFlow(
          snapshot.item_flows,
          (flow) => flow.item === "ammo" && flow.from_entity.startsWith("drone_") && flow.to_entity.startsWith("turret_")
        )
    },
    {
      id: "wire-cutter",
      label: "Watch item flow",
      detail: "Animated belt transfer",
      complete:
        snapshot.item_flows.length > 0 ||
        snapshot.logs.some((entry) => entry.source.startsWith("wire_") && entry.message.includes("destroyed"))
    }
  ];
}

function emptyTutorialSteps() {
  return [
    "mining-loop",
    "cpu-network",
    "edit-code",
    "ammo-production",
    "defense",
    "drone-delivery",
    "wire-cutter"
  ].map((id) => ({ id, label: "Loading", detail: "Waiting for world", complete: false }));
}

function hasBlock(blocks: Block[], kind: Block["kind"]) {
  return blocks.some((block) => block.kind === kind);
}

function hasFlow(flows: ItemFlowEvent[], predicate: (flow: ItemFlowEvent) => boolean) {
  return flows.some(predicate);
}
