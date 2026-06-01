import { expect, test } from "@playwright/test";

type IpcCall = {
  cmd: string;
  args: Record<string, unknown>;
};

type TestEnemyKind = "grunt" | "runner" | "armored" | "wire_cutter";

declare global {
  interface Window {
    __XAC_TEST_STATE__?: {
      calls: IpcCall[];
      snapshot: () => {
        behaviors: Array<{ id: string; source_path: string }>;
        blocks: Array<{ id: string; kind: string; hp: number; network_id: number | null; effective_cpu_rate: number }>;
        drones: Array<{ id: string; behavior_ref: string | null }>;
        enemies: Array<{ id: string; pos: { x: number; y: number }; target_id: string | null }>;
        item_flows: Array<{ item: string; amount: number; from_entity: string; to_entity: string }>;
        networks: Array<{
          cpu_pool: number;
          active_devices: number;
          effective_per_device: number;
          block_ids: string[];
          read_only_cache: boolean;
        }>;
        status: {
          wire_threats: number;
          damaged_wires: number;
          network_cpu: number;
        };
        selected_id: string | null;
      };
      spawnCarrierDrone: (homePortId?: string) => string;
      spawnEnemy: (kind: TestEnemyKind, pos: { x: number; y: number }) => string;
      forceOverBudget: (entityId: string) => void;
    };
    __XAC_EDITOR__?: {
      getValue: () => string;
      setValue: (value: string) => void;
    };
  }
}

const tileCenter = (x: number, y: number) => ({
  x: x * 16 + 8,
  y: y * 16 + 8
});

test("places minimum devices from the right block list and opens drill behavior", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByText("Blocks", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: /Core/ })).toHaveCount(0);
  await expect(page.getByRole("button", { name: /Ore Drill/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Belt Conveyor/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Storage/ })).toBeVisible();
  const builtinBehaviorIds = await page.evaluate(() =>
    window.__XAC_TEST_STATE__?.snapshot().behaviors.map((behavior) => behavior.id)
  );
  expect(builtinBehaviorIds).toEqual(
    expect.arrayContaining([
      "builtin.drill.basic",
      "builtin.router.basic",
      "builtin.router.ammo_east",
      "builtin.assembler.basic",
      "builtin.turret.basic",
      "builtin.turret.priority",
      "builtin.drone_port.basic",
      "builtin.carrier_drone.basic"
    ])
  );

  const canvas = page.getByTestId("grid-world").locator("canvas");
  await expect(canvas).toBeVisible();
  await expect(page.locator(".metrics")).toContainText("blocks 1");
  await expect(page.locator(".metrics")).toContainText("wave 1");
  await expect(page.locator(".metrics")).toContainText("net CPU 120");
  await expect(page.locator(".metrics")).toContainText("core HP 500/500");
  await expect(page.locator(".metrics")).toContainText("core ore 40");
  await expect(page.locator(".metrics")).toContainText("core plate 20");
  await expect(page.locator(".metrics")).toContainText("core ammo 60");
  await expect(page.locator(".metrics")).toContainText("enemies 1");
  await expect(page.locator(".inspector")).toContainText("core");
  await canvas.click({ position: tileCenter(28, 28) });
  await expect(page.locator(".inspector")).toContainText("runner");
  await expect(page.locator(".inspector")).toContainText("28.50, 28.50");
  await expect(page.locator(".inspector")).toContainText("core_1");
  await page.getByRole("button", { name: /Tick/ }).click();
  await expect(page.locator(".inspector")).toContainText("28.60, 28.60");
  const movedEnemy = await page.evaluate(() => window.__XAC_TEST_STATE__?.snapshot().enemies[0]);
  expect(movedEnemy).toEqual(
    expect.objectContaining({
      id: "enemy_1",
      target_id: "core_1",
      pos: expect.objectContaining({
        x: expect.closeTo(28.6, 0.02),
        y: expect.closeTo(28.6, 0.02)
      })
    })
  );
  await canvas.click({ position: tileCenter(32, 32) });
  await expect(page.locator(".inspector")).toContainText("core");

  await page.getByRole("button", { name: /Ore Drill/ }).click();
  await expect(page.getByText("Placing drill")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByText("Select to place")).toBeVisible();
  await page.getByRole("button", { name: /Ore Drill/ }).click();
  await expect(page.getByText("Placing drill")).toBeVisible();
  await page.getByRole("button", { name: "Cancel placement", exact: true }).click();
  await expect(page.getByText("Select to place")).toBeVisible();
  await canvas.click({ position: tileCenter(18, 30) });
  await expect(page.locator(".metrics")).toContainText("blocks 1");

  await page.getByRole("button", { name: /CPU Node/ }).click();
  await expect(page.getByText("Placing cpu node")).toBeVisible();
  await canvas.click({ position: tileCenter(19, 29) });
  await expect(page.locator(".metrics")).toContainText("blocks 2");
  await expect(page.locator(".inspector")).toContainText("cpu_node");
  const isolatedNetworks = await page.evaluate(() => window.__XAC_TEST_STATE__?.snapshot().networks ?? []);
  expect(isolatedNetworks).toEqual(
    expect.arrayContaining([
      expect.objectContaining({ cpu_pool: 80, block_ids: ["cpu_node_1"], read_only_cache: true }),
      expect.objectContaining({ cpu_pool: 120, block_ids: ["core_1"], read_only_cache: false })
    ])
  );

  await page.getByRole("button", { name: /Wire/ }).click();
  await expect(page.getByText("Placing wire")).toBeVisible();
  for (let x = 20; x <= 30; x += 1) {
    await canvas.click({ position: tileCenter(x, 29) });
  }
  await expect(page.locator(".metrics")).toContainText("blocks 13");
  const connectedNetworks = await page.evaluate(() => window.__XAC_TEST_STATE__?.snapshot().networks ?? []);
  expect(connectedNetworks).toEqual([
    expect.objectContaining({
      cpu_pool: 200,
      active_devices: 0,
      block_ids: expect.arrayContaining(["core_1", "cpu_node_1", "wire_1", "wire_11"]),
      read_only_cache: false
    })
  ]);

  await page.getByRole("button", { name: /Belt Conveyor/ }).click();
  await expect(page.getByText("Placing conveyor")).toBeVisible();
  for (let x = 21; x < 30; x += 1) {
    await canvas.click({ position: tileCenter(x, 30) });
  }
  await expect(page.locator(".metrics")).toContainText("blocks 22");
  await expect(page.locator(".inspector")).toContainText("conveyor");
  const rotateSelectedBlock = page.getByRole("button", { name: "Rotate selected block", exact: true });
  await rotateSelectedBlock.click();
  await expect(page.locator(".inspector")).toContainText("facing south");
  await expect(page.locator(".log-panel")).toContainText("rotated Conveyor to south");
  await rotateSelectedBlock.click();
  await rotateSelectedBlock.click();
  await rotateSelectedBlock.click();
  await expect(page.locator(".inspector")).toContainText("facing east");

  await page.getByRole("button", { name: /Ore Drill/ }).click();
  await expect(page.getByText("Placing drill")).toBeVisible();
  await canvas.click({ position: tileCenter(20, 30) });
  await expect(page.locator(".metrics")).toContainText("blocks 23");
  await expect(page.locator(".metrics")).toContainText("net CPU 200");
  await expect(page.locator(".inspector")).toContainText("drill");
  await expect(page.locator(".inspector")).toContainText("network CPU 200");
  await expect(page.locator(".inspector")).toContainText("active 1");
  await expect(page.locator(".inspector")).toContainText("share 200.0");
  await expect(page.locator(".inspector")).toContainText("Fuel Bank");
  await expect(page.locator(".log-panel")).toContainText("placed Drill at 20,30");
  const drillNetwork = await page.evaluate(() =>
    window.__XAC_TEST_STATE__?.snapshot().blocks.find((block) => block.id === "drill_1")
  );
  expect(drillNetwork).toEqual(expect.objectContaining({ network_id: 1, effective_cpu_rate: 201 }));

  await page.getByRole("button", { name: /\+40/ }).click();
  await expect(page.locator(".metrics")).toContainText("core ore 41");
  await expect(page.locator(".metrics")).toContainText("flows");
  await expect(page.locator(".inspector")).toContainText("runtime tick");
  await expect(page.locator(".inspector")).toContainText("wasm mocked-was");
  await expect(page.getByLabel("Logistics")).toContainText("Ore x1");
  await expect(page.getByLabel("Logistics")).toContainText("conveyor_9 -> core_1");
  const oreFlows = await page.evaluate(() => window.__XAC_TEST_STATE__?.snapshot().item_flows ?? []);
  expect(oreFlows).toEqual(
    expect.arrayContaining([
      expect.objectContaining({ item: "ore", amount: 1, from_entity: "conveyor_9", to_entity: "core_1" })
    ])
  );
  await page.evaluate(() => window.__XAC_TEST_STATE__!.forceOverBudget("drill_1"));
  await page.keyboard.press("Escape");
  await canvas.click({ position: tileCenter(20, 30) });
  await expect(page.locator(".inspector")).toContainText("over budget");
  await expect(page.locator(".inspector")).toContainText("fuel 40/40");
  await expect(page.locator(".log-panel")).toContainText("over_budget with 40 fuel");
  await canvas.click({ position: tileCenter(32, 32) });
  await expect(page.locator(".inspector")).toContainText("core");
  await expect(page.locator(".inspector")).toContainText("Ore: 41");
  await expect(page.locator(".inspector")).toContainText("under attack by enemy_1");
  const defendedCore = await page.evaluate(() =>
    window.__XAC_TEST_STATE__?.snapshot().blocks.find((block) => block.id === "core_1")
  );
  expect(defendedCore?.hp).toBeLessThan(500);

  await canvas.click({ position: tileCenter(20, 30) });
  await expect(page.locator(".inspector")).toContainText("drill");

  await page.getByRole("button", { name: "Open", exact: true }).click();
  await expect(page.locator(".behavior-meta")).toContainText("Basic Drill");
  await expect(page.locator(".behavior-meta")).toContainText("builtin.drill.basic");
  await expect(page.locator(".behavior-meta")).toContainText("XaC Script");
  await expect(page.locator(".behavior-meta")).toContainText("read-only preset");
  await expect(page.locator(".behavior-meta")).toContainText("status builtin");
  await expect(page.locator(".behavior-meta")).toContainText("assets/builtin/drill/basic.xac");
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /if output_blocked return/);
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /mine/);

  await page.getByRole("button", { name: "Edit Copy", exact: true }).click();
  await expect(page.locator(".behavior-meta")).toContainText("Basic Drill Copy");
  await expect(page.locator(".behavior-meta")).toContainText("project behavior");
  await page.waitForFunction(() => Boolean(window.__XAC_EDITOR__));

  const editedSource = `if output_blocked return
# edited in UI E2E
mine
`;
  await page.evaluate((source) => window.__XAC_EDITOR__!.setValue(source), editedSource);
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /edited in UI E2E/);

  const saveButton = page.getByRole("button", { name: "Save", exact: true });
  const buildButton = page.getByRole("button", { name: "Build", exact: true });
  await expect(saveButton).toBeEnabled();
  await saveButton.click();
  await expect(saveButton).toBeDisabled();
  await buildButton.click();
  await expect(page.locator(".build-ok")).toContainText("mock build succeeded");
  await expect(page.locator(".behavior-meta")).toContainText("status built");

  await page.getByRole("button", { name: "Deconstruct", exact: true }).click();
  await expect(page.locator(".metrics")).toContainText("blocks 22");
  await expect(page.locator(".inspector")).toContainText("Select a block");
  await expect(page.locator(".log-panel")).toContainText("deconstructed Drill");

  await page.getByRole("button", { name: /Storage/ }).click();
  await canvas.click({ position: tileCenter(20, 30) });
  await expect(page.locator(".metrics")).toContainText("blocks 23");
  await expect(page.locator(".inspector")).toContainText("storage");
  await expect(page.locator(".inspector")).toContainText("empty");
  await expect(page.locator(".log-panel")).toContainText("placed Storage at 20,30");

  const behaviorCalls = await page.evaluate(() => {
    return (
      window.__XAC_TEST_STATE__?.calls.filter((call) =>
        ["edit_builtin_copy", "save_behavior", "build_behavior"].includes(call.cmd)
      ) ?? []
    );
  });
  expect(behaviorCalls.map((call) => call.cmd)).toEqual(["edit_builtin_copy", "save_behavior", "build_behavior"]);
  expect(behaviorCalls[0].args).toEqual({ blockId: "drill_1" });
  expect(behaviorCalls[1].args).toEqual({ behaviorId: "behavior_1", source: editedSource });
  expect(behaviorCalls[2].args).toEqual({ behaviorId: "behavior_1" });

  const placeCalls = await page.evaluate(() => {
    return window.__XAC_TEST_STATE__?.calls.filter((call) => call.cmd === "place_block") ?? [];
  });
  expect(placeCalls).toHaveLength(23);
  expect(placeCalls[0].args).toEqual({ kind: "cpu_node", x: 19, y: 29, dir: "east" });
  expect(placeCalls.some((call) => call.args.kind === "wire" && call.args.x === 30 && call.args.y === 29)).toBe(true);
  expect(placeCalls.some((call) => call.args.kind === "conveyor" && call.args.x === 29 && call.args.y === 30)).toBe(true);
  expect(placeCalls.some((call) => call.args.kind === "storage" && call.args.x === 20 && call.args.y === 30)).toBe(true);
  expect(placeCalls.at(-1)?.args).toEqual({ kind: "storage", x: 20, y: 30, dir: "east" });

  const rotateCalls = await page.evaluate(() => {
    return window.__XAC_TEST_STATE__?.calls.filter((call) => call.cmd === "rotate_block") ?? [];
  });
  expect(rotateCalls).toEqual([
    { cmd: "rotate_block", args: { blockId: "conveyor_9" } },
    { cmd: "rotate_block", args: { blockId: "conveyor_9" } },
    { cmd: "rotate_block", args: { blockId: "conveyor_9" } },
    { cmd: "rotate_block", args: { blockId: "conveyor_9" } }
  ]);

  const deconstructCalls = await page.evaluate(() => {
    return window.__XAC_TEST_STATE__?.calls.filter((call) => call.cmd === "deconstruct_block") ?? [];
  });
  expect(deconstructCalls).toEqual([{ cmd: "deconstruct_block", args: { blockId: "drill_1" } }]);

  await page.getByRole("button", { name: /Turret/ }).click();
  await canvas.click({ position: tileCenter(34, 30) });
  await expect(page.locator(".inspector")).toContainText("turret");
  await page.getByLabel("Assign behavior preset").selectOption("builtin.turret.priority");
  await expect(page.locator(".inspector")).toContainText("behavior: Priority Turret");
  await expect(page.locator(".log-panel")).toContainText("assigned Priority Turret");

  await page.getByRole("button", { name: "Open", exact: true }).click();
  await expect(page.locator(".behavior-meta")).toContainText("Priority Turret");
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /attack_best runner wire_cutter/);

  const assignCalls = await page.evaluate(() => {
    return window.__XAC_TEST_STATE__?.calls.filter((call) => call.cmd === "assign_behavior") ?? [];
  });
  expect(assignCalls).toEqual([
    { cmd: "assign_behavior", args: { blockId: "turret_1", behaviorId: "builtin.turret.priority" } }
  ]);

  await page.getByRole("button", { name: /Drone Port/ }).click();
  await canvas.click({ position: tileCenter(35, 30) });
  await expect(page.locator(".inspector")).toContainText("drone_port");

  const droneId = await page.evaluate(() => window.__XAC_TEST_STATE__!.spawnCarrierDrone("drone_port_1"));
  expect(droneId).toBe("drone_1");
  await expect(page.locator(".inspector")).toContainText("drone_1");
  await expect(page.locator(".inspector")).toContainText("docked");
  await expect(page.locator(".inspector")).toContainText("empty cargo");

  await page.getByRole("button", { name: "Open", exact: true }).click();
  await expect(page.locator(".behavior-meta")).toContainText("Basic Carrier Drone");
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /claim_delivery_job/);

  await page.getByRole("button", { name: "Edit Copy", exact: true }).click();
  await expect(page.locator(".behavior-meta")).toContainText("Basic Carrier Drone Copy");
  await expect(page.locator(".behavior-meta")).toContainText("project behavior");
  await expect(page.getByLabel("Assign behavior preset")).toHaveValue("behavior_2");
  await page.getByLabel("Assign behavior preset").selectOption("builtin.carrier_drone.basic");
  await expect(page.locator(".log-panel")).toContainText("assigned Basic Carrier Drone");
  await expect(page.locator(".behavior-meta")).toContainText("Basic Carrier Drone");

  const droneSnapshot = await page.evaluate(() => window.__XAC_TEST_STATE__?.snapshot().drones ?? []);
  expect(droneSnapshot).toEqual([
    expect.objectContaining({ id: "drone_1", behavior_ref: "builtin.carrier_drone.basic" })
  ]);

  const droneBehaviorCalls = await page.evaluate(() => {
    return (
      window.__XAC_TEST_STATE__?.calls.filter((call) => {
        const args = call.args;
        return (
          (call.cmd === "open_behavior" && args.behaviorId === "builtin.carrier_drone.basic") ||
          (call.cmd === "edit_builtin_copy" && args.blockId === "drone_1") ||
          (call.cmd === "assign_behavior" && args.blockId === "drone_1")
        );
      }) ?? []
    );
  });
  expect(droneBehaviorCalls).toEqual([
    { cmd: "open_behavior", args: { behaviorId: "builtin.carrier_drone.basic" } },
    { cmd: "edit_builtin_copy", args: { blockId: "drone_1" } },
    { cmd: "assign_behavior", args: { blockId: "drone_1", behaviorId: "builtin.carrier_drone.basic" } },
    { cmd: "open_behavior", args: { behaviorId: "builtin.carrier_drone.basic" } }
  ]);

  await page.getByRole("button", { name: /Router/ }).click();
  await canvas.click({ position: tileCenter(34, 28) });
  await expect(page.locator(".inspector")).toContainText("router");
  await page.getByLabel("Assign behavior preset").selectOption("builtin.router.ammo_east");
  await expect(page.locator(".inspector")).toContainText("behavior: Ammo East Router");
  await page.getByRole("button", { name: "Open", exact: true }).click();
  await expect(page.locator(".behavior-meta")).toContainText("Ammo East Router");
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /push ammo east/);
  await page.getByRole("button", { name: "Edit Copy", exact: true }).click();
  await expect(page.locator(".behavior-meta")).toContainText("Ammo East Router Copy");
  await expect(page.locator(".behavior-meta")).toContainText("project behavior");

  const routerSource = "if output_available ammo south push ammo south\n";
  await page.evaluate((source) => window.__XAC_EDITOR__!.setValue(source), routerSource);
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /push ammo south/);
  await expect(saveButton).toBeEnabled();
  await saveButton.click();
  await expect(saveButton).toBeDisabled();
  await buildButton.click();
  await expect(page.locator(".behavior-meta")).toContainText("status built");

  await page.getByRole("button", { name: /Assembler/ }).click();
  await canvas.click({ position: tileCenter(35, 28) });
  await expect(page.locator(".inspector")).toContainText("assembler");
  await page.getByRole("button", { name: "Open", exact: true }).click();
  await expect(page.locator(".behavior-meta")).toContainText("Basic Assembler");
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /set_recipe plate/);
  await page.getByRole("button", { name: "Edit Copy", exact: true }).click();
  await expect(page.locator(".behavior-meta")).toContainText("Basic Assembler Copy");
  await expect(page.locator(".behavior-meta")).toContainText("project behavior");

  const assemblerSource = `set_recipe ammo
if input_count ore > 1 set_recipe plate
if can_produce produce
`;
  await page.evaluate((source) => window.__XAC_EDITOR__!.setValue(source), assemblerSource);
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /input_count ore/);
  await expect(saveButton).toBeEnabled();
  await saveButton.click();
  await expect(saveButton).toBeDisabled();
  await buildButton.click();
  await expect(page.locator(".behavior-meta")).toContainText("status built");

  const allCalls = (await page.evaluate(() => window.__XAC_TEST_STATE__?.calls ?? [])) as IpcCall[];
  const scriptEditCalls = allCalls.filter((call) => {
    return (
      (call.cmd === "assign_behavior" && call.args.blockId === "router_1") ||
      (call.cmd === "edit_builtin_copy" && ["router_1", "assembler_1"].includes(String(call.args.blockId))) ||
      (call.cmd === "save_behavior" && [routerSource, assemblerSource].includes(String(call.args.source))) ||
      (call.cmd === "build_behavior" && ["behavior_3", "behavior_4"].includes(String(call.args.behaviorId)))
    );
  });
  expect(scriptEditCalls).toEqual([
    { cmd: "assign_behavior", args: { blockId: "router_1", behaviorId: "builtin.router.ammo_east" } },
    { cmd: "edit_builtin_copy", args: { blockId: "router_1" } },
    { cmd: "save_behavior", args: { behaviorId: "behavior_3", source: routerSource } },
    { cmd: "build_behavior", args: { behaviorId: "behavior_3" } },
    { cmd: "edit_builtin_copy", args: { blockId: "assembler_1" } },
    { cmd: "save_behavior", args: { behaviorId: "behavior_4", source: assemblerSource } },
    { cmd: "build_behavior", args: { behaviorId: "behavior_4" } }
  ]);
});

test("wire cutter can sever a CPU network in the UI simulation", async ({ page }) => {
  await page.goto("/");

  const canvas = page.getByTestId("grid-world").locator("canvas");
  await expect(canvas).toBeVisible();

  await page.getByRole("button", { name: /CPU Node/ }).click();
  await canvas.click({ position: tileCenter(19, 29) });
  await page.getByRole("button", { name: /Wire/ }).click();
  for (let x = 20; x <= 30; x += 1) {
    await canvas.click({ position: tileCenter(x, 29) });
  }
  await page.getByRole("button", { name: /Ore Drill/ }).click();
  await canvas.click({ position: tileCenter(20, 30) });

  const connectedDrill = await page.evaluate(() =>
    window.__XAC_TEST_STATE__?.snapshot().blocks.find((block) => block.id === "drill_1")
  );
  expect(connectedDrill).toEqual(expect.objectContaining({ network_id: 1, effective_cpu_rate: 201 }));

  const cutterId = await page.evaluate(() => window.__XAC_TEST_STATE__!.spawnEnemy("wire_cutter", { x: 20.5, y: 29.5 }));
  expect(cutterId).toBe("enemy_2");
  const threatStatus = await page.evaluate(() => window.__XAC_TEST_STATE__!.snapshot().status);
  expect(threatStatus.wire_threats).toBe(1);

  await page.getByRole("button", { name: /\+40/ }).click();

  const severed = await page.evaluate(() => {
    const snapshot = window.__XAC_TEST_STATE__!.snapshot();
    return {
      drill: snapshot.blocks.find((block) => block.id === "drill_1"),
      wire: snapshot.blocks.find((block) => block.id === "wire_1"),
      networks: snapshot.networks,
      logs: snapshot.logs
    };
  });

  expect(severed.wire).toBeUndefined();
  expect(severed.drill).toEqual(expect.objectContaining({ network_id: null, effective_cpu_rate: 1 }));
  expect(severed.networks).toEqual(
    expect.arrayContaining([
      expect.objectContaining({ cpu_pool: 80, block_ids: ["cpu_node_1"], read_only_cache: true }),
      expect.objectContaining({ cpu_pool: 120, block_ids: expect.arrayContaining(["core_1"]), read_only_cache: false })
    ])
  );
  expect(severed.logs).toEqual(
    expect.arrayContaining([expect.objectContaining({ level: "warn", source: "wire_1", message: "block destroyed" })])
  );
});
