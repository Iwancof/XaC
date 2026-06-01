import { expect, test, type Page } from "@playwright/test";
import { dragTiles, tileCenter, type IpcCall } from "./support/xacHarness";

type BuildTileId =
  | "drill"
  | "conveyor"
  | "wire"
  | "cpu_node"
  | "router"
  | "storage"
  | "assembler"
  | "turret"
  | "drone_port";

const clickBuild = async (page: Page, kind: BuildTileId) => {
  await page.getByTestId(`build-tile-${kind}`).click();
};

test("places minimum devices from the right block list and opens drill behavior @full @placement", async ({ page }) => {
  test.setTimeout(120_000);

  await page.goto("/");

  await expect(page.getByText("Blocks", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: /Core/ })).toHaveCount(0);
  await expect(page.getByTestId("build-tile-drill")).toBeVisible();
  await expect(page.getByTestId("build-tile-conveyor")).toBeVisible();
  await expect(page.getByTestId("build-tile-storage")).toBeVisible();
  await expect(page.getByTestId("template-list")).toContainText("Rust Basic Drill");
  await expect(page.getByTestId("template-list")).toContainText("AssemblyScript Basic Router");
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
  await expect(page.locator(".metrics")).toContainText("wave 0");
  await expect(page.locator(".metrics")).toContainText("net CPU 120");
  await expect(page.locator(".metrics")).toContainText("core HP 500/500");
  await expect(page.locator(".metrics")).toContainText("core ore 40");
  await expect(page.locator(".metrics")).toContainText("core plate 20");
  await expect(page.locator(".metrics")).toContainText("core ammo 60");
  await expect(page.locator(".metrics")).toContainText("enemies 0");
  await expect(page.getByTestId("overlay-details")).toContainText("Network");
  await expect(page.getByTestId("overlay-details")).toContainText("net 1");
  await expect(page.getByTestId("overlay-details")).toContainText("CPU 120");
  await expect(page.getByTestId("tutorial-panel")).toContainText("Objectives");
  await expect(page.getByTestId("tutorial-progress")).toContainText("0/7");
  await expect(page.getByTestId("tutorial-mining-loop")).toHaveAttribute("data-state", "pending");
  await expect(page.locator(".inspector")).toContainText("core");
  await page.getByRole("button", { name: /Tick/ }).click();
  await expect(page.locator(".metrics")).toContainText("enemies 0");
  await canvas.click({ position: tileCenter(32, 32) });
  await expect(page.locator(".inspector")).toContainText("core");

  await clickBuild(page, "drill");
  await expect(page.getByText("Placing drill")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByText("Select to place")).toBeVisible();
  await clickBuild(page, "drill");
  await expect(page.getByText("Placing drill")).toBeVisible();
  await page.getByRole("button", { name: "Cancel placement", exact: true }).click();
  await expect(page.getByText("Select to place")).toBeVisible();
  await canvas.click({ position: tileCenter(18, 30) });
  await expect(page.locator(".metrics")).toContainText("blocks 1");

  await clickBuild(page, "cpu_node");
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

  await clickBuild(page, "wire");
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
  await expect(page.getByTestId("tutorial-cpu-network")).toHaveAttribute("data-state", "complete");

  await clickBuild(page, "conveyor");
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

  await clickBuild(page, "drill");
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
  await page.getByRole("button", { name: "CPU", exact: true }).click();
  await expect(page.getByTestId("overlay-details")).toContainText("drill_1");
  await expect(page.getByTestId("overlay-details")).toContainText("201.0 fuel/s");
  await expect(page.getByTestId("overlay-details")).toContainText("network 1");
  await page.getByRole("button", { name: "Network", exact: true }).click();
  await expect(page.getByTestId("overlay-details")).toContainText("CPU 200");
  await expect(page.getByTestId("overlay-details")).toContainText("active 1");

  await page.getByRole("button", { name: /\+40/ }).click();
  await expect(page.locator(".metrics")).toContainText("core ore 41");
  await expect(page.locator(".metrics")).toContainText("flows");
  await expect(page.getByTestId("tutorial-mining-loop")).toHaveAttribute("data-state", "complete");
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
  await page.evaluate(() => window.__XAC_TEST_STATE__!.forceRuntimeError("drill_1", "mocked wasm unreachable trap"));
  await page.keyboard.press("Escape");
  await canvas.click({ position: tileCenter(20, 30) });
  await expect(page.locator(".inspector")).toContainText("runtime error");
  await expect(page.locator(".inspector")).toContainText("mocked wasm unreachable trap");
  await expect(page.locator(".inspector")).toContainText("fuel 12/40");
  await expect(page.locator(".log-panel")).toContainText("mocked wasm unreachable trap");
  await canvas.click({ position: tileCenter(32, 32) });
  await expect(page.locator(".inspector")).toContainText("core");
  await expect(page.locator(".inspector")).toContainText("Ore: 41");
  const defendedCore = await page.evaluate(() =>
    window.__XAC_TEST_STATE__?.snapshot().blocks.find((block) => block.id === "core_1")
  );
  expect(defendedCore?.hp).toBe(500);

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
  await expect(page.getByTestId("tutorial-edit-code")).toHaveAttribute("data-state", "complete");
  await page.waitForFunction(() => Boolean(window.__XAC_EDITOR__));

  const saveButton = page.getByRole("button", { name: "Save", exact: true });
  const buildButton = page.getByRole("button", { name: "Build", exact: true });

  const invalidSource = `mine ???
`;
  await page.evaluate((source) => window.__XAC_EDITOR__!.setValue(source), invalidSource);
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /mine \?\?\?/);
  await expect(saveButton).toBeEnabled();
  await saveButton.click();
  await expect(saveButton).toBeDisabled();
  await buildButton.click();
  await expect(page.locator(".build-fail")).toContainText("mock build failed: unsupported drill line 1");
  await expect(page.locator(".behavior-meta")).toContainText("status build failed");

  const pausedSource = `if output_blocked return
# paused in UI E2E
`;
  await page.evaluate((source) => window.__XAC_EDITOR__!.setValue(source), pausedSource);
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /paused in UI E2E/);
  await expect(saveButton).toBeEnabled();
  await saveButton.click();
  await expect(saveButton).toBeDisabled();
  await buildButton.click();
  await expect(page.locator(".build-ok")).toContainText("mock build succeeded");
  await expect(page.locator(".behavior-meta")).toContainText("status built");
  const pausedCoreOre = await page.evaluate(() => {
    const before = window.__XAC_TEST_STATE__!.snapshot().blocks.find((block) => block.id === "core_1")?.inventory.items.ore ?? 0;
    return before;
  });
  await page.getByRole("button", { name: /\+40/ }).click();
  const stillPausedCoreOre = await page.evaluate(
    () => window.__XAC_TEST_STATE__!.snapshot().blocks.find((block) => block.id === "core_1")?.inventory.items.ore ?? 0
  );
  expect(stillPausedCoreOre).toBe(pausedCoreOre);

  const editedSource = `if output_blocked return
# edited in UI E2E
mine
`;
  await page.evaluate((source) => window.__XAC_EDITOR__!.setValue(source), editedSource);
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /edited in UI E2E/);
  await expect(saveButton).toBeEnabled();
  await saveButton.click();
  await expect(saveButton).toBeDisabled();
  await buildButton.click();
  await expect(page.locator(".build-ok")).toContainText("mock build succeeded");
  await page.getByRole("button", { name: /\+40/ }).click();
  const resumedCoreOre = await page.evaluate(
    () => window.__XAC_TEST_STATE__!.snapshot().blocks.find((block) => block.id === "core_1")?.inventory.items.ore ?? 0
  );
  expect(resumedCoreOre).toBeGreaterThan(stillPausedCoreOre);

  await page.getByRole("button", { name: "Deconstruct", exact: true }).click();
  await expect(page.locator(".metrics")).toContainText("blocks 22");
  await expect(page.locator(".inspector")).toContainText("Select a block");
  await expect(page.locator(".log-panel")).toContainText("deconstructed Drill");

  await clickBuild(page, "storage");
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
  expect(behaviorCalls.map((call) => call.cmd)).toEqual([
    "edit_builtin_copy",
    "save_behavior",
    "build_behavior",
    "save_behavior",
    "build_behavior",
    "save_behavior",
    "build_behavior"
  ]);
  expect(behaviorCalls[0].args).toEqual({ blockId: "drill_1" });
  expect(behaviorCalls[1].args).toEqual({ behaviorId: "behavior_1", source: invalidSource });
  expect(behaviorCalls[2].args).toEqual({ behaviorId: "behavior_1" });
  expect(behaviorCalls[3].args).toEqual({ behaviorId: "behavior_1", source: pausedSource });
  expect(behaviorCalls[4].args).toEqual({ behaviorId: "behavior_1" });
  expect(behaviorCalls[5].args).toEqual({ behaviorId: "behavior_1", source: editedSource });
  expect(behaviorCalls[6].args).toEqual({ behaviorId: "behavior_1" });

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

  await clickBuild(page, "turret");
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

  await clickBuild(page, "drone_port");
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

  await clickBuild(page, "router");
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

  const invalidRouterSource = "push sideways\n";
  await page.evaluate((source) => window.__XAC_EDITOR__!.setValue(source), invalidRouterSource);
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /push sideways/);
  await expect(saveButton).toBeEnabled();
  await saveButton.click();
  await expect(saveButton).toBeDisabled();
  await buildButton.click();
  await expect(page.locator(".build-fail")).toContainText("mock build failed: unsupported router line 1");
  await expect(page.locator(".behavior-meta")).toContainText("status build failed");

  const routerSource = "if output_available ammo south push ammo south\n";
  await page.evaluate((source) => window.__XAC_EDITOR__!.setValue(source), routerSource);
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /push ammo south/);
  await expect(saveButton).toBeEnabled();
  await saveButton.click();
  await expect(saveButton).toBeDisabled();
  await buildButton.click();
  await expect(page.locator(".behavior-meta")).toContainText("status built");

  await clickBuild(page, "assembler");
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
      (call.cmd === "save_behavior" && [invalidRouterSource, routerSource, assemblerSource].includes(String(call.args.source))) ||
      (call.cmd === "build_behavior" && ["behavior_3", "behavior_4"].includes(String(call.args.behaviorId)))
    );
  });
  expect(scriptEditCalls).toEqual([
    { cmd: "assign_behavior", args: { blockId: "router_1", behaviorId: "builtin.router.ammo_east" } },
    { cmd: "edit_builtin_copy", args: { blockId: "router_1" } },
    { cmd: "save_behavior", args: { behaviorId: "behavior_3", source: invalidRouterSource } },
    { cmd: "build_behavior", args: { behaviorId: "behavior_3" } },
    { cmd: "save_behavior", args: { behaviorId: "behavior_3", source: routerSource } },
    { cmd: "build_behavior", args: { behaviorId: "behavior_3" } },
    { cmd: "edit_builtin_copy", args: { blockId: "assembler_1" } },
    { cmd: "save_behavior", args: { behaviorId: "behavior_4", source: assemblerSource } },
    { cmd: "build_behavior", args: { behaviorId: "behavior_4" } }
  ]);
});

test("project behavior can be edited in place or forked for one block @behavior", async ({ page }) => {
  await page.goto("/");

  const canvas = page.getByTestId("grid-world").locator("canvas");
  await expect(canvas).toBeVisible();

  await clickBuild(page, "turret");
  await canvas.click({ position: tileCenter(34, 30) });
  await expect(page.locator(".inspector")).toContainText("turret_1");
  await page.getByRole("button", { name: "Edit Copy", exact: true }).click();
  await expect(page.locator(".behavior-meta")).toContainText("Basic Turret Copy");
  await expect(page.locator(".behavior-meta")).toContainText("project behavior");
  await expect(page.getByLabel("Assign behavior preset")).toHaveValue("behavior_1");
  await expect(page.getByRole("button", { name: "Edit", exact: true })).toBeVisible();

  await canvas.click({ position: tileCenter(35, 30) });
  await expect(page.locator(".inspector")).toContainText("turret_2");
  await page.getByLabel("Assign behavior preset").selectOption("behavior_1");
  await expect(page.locator(".inspector")).toContainText("behavior: Basic Turret Copy");

  await page.keyboard.press("Escape");
  await canvas.click({ position: tileCenter(34, 30) });
  await expect(page.locator(".inspector")).toContainText("turret_1");
  await expect(page.getByLabel("Assign behavior preset")).toHaveValue("behavior_1");
  await page.getByRole("button", { name: "Edit", exact: true }).click();
  await expect(page.locator(".behavior-meta")).toContainText("Basic Turret Copy");
  await expect(page.locator(".behavior-meta")).toContainText("behavior_1");

  await page.getByRole("button", { name: "Fork", exact: true }).click();
  await expect(page.locator(".behavior-meta")).toContainText("Basic Turret Copy Fork");
  await expect(page.locator(".behavior-meta")).toContainText("behavior_2");
  await expect(page.getByLabel("Assign behavior preset")).toHaveValue("behavior_2");

  const turretRefs = await page.evaluate(() => {
    const blocks = window.__XAC_TEST_STATE__?.snapshot().blocks ?? [];
    return Object.fromEntries(
      blocks.filter((block) => block.kind === "turret").map((block) => [block.id, block.behavior_ref])
    );
  });
  expect(turretRefs).toEqual({
    turret_1: "behavior_2",
    turret_2: "behavior_1"
  });

  const behaviorCalls = await page.evaluate(() => {
    return (
      window.__XAC_TEST_STATE__?.calls.filter((call) =>
        ["edit_builtin_copy", "assign_behavior", "fork_behavior"].includes(call.cmd)
      ) ?? []
    );
  });
  expect(behaviorCalls).toEqual([
    { cmd: "edit_builtin_copy", args: { blockId: "turret_1" } },
    { cmd: "assign_behavior", args: { blockId: "turret_2", behaviorId: "behavior_1" } },
    { cmd: "edit_builtin_copy", args: { blockId: "turret_1" } },
    { cmd: "fork_behavior", args: { blockId: "turret_1" } }
  ]);
});

test("UI mock runs code-driven assembler ammo into turret defense @production @combat", async ({ page }) => {
  await page.goto("/");

  const canvas = page.getByTestId("grid-world").locator("canvas");
  await expect(canvas).toBeVisible();

  await clickBuild(page, "assembler");
  await canvas.click({ position: tileCenter(34, 30) });

  await page.evaluate(() => window.__XAC_TEST_STATE__!.setBlockInventory("assembler_1", { plate: 1 }));
  for (let i = 0; i < 15; i += 1) {
    await page.getByRole("button", { name: /\+40/ }).click();
  }
  await clickBuild(page, "conveyor");
  await canvas.click({ position: tileCenter(35, 30) });
  await clickBuild(page, "turret");
  await canvas.click({ position: tileCenter(36, 30) });
  for (let i = 0; i < 3; i += 1) {
    await page.getByRole("button", { name: /\+40/ }).click();
  }

  const supplied = await page.evaluate(() => {
    const snapshot = window.__XAC_TEST_STATE__!.snapshot();
    return {
      turret: snapshot.blocks.find((block) => block.id === "turret_1"),
      ammoFlow: snapshot.item_flows.some(
        (flow) => flow.item === "ammo" && flow.from_entity === "conveyor_1" && flow.to_entity === "turret_1"
      )
    };
  });
  expect(supplied.turret?.inventory.items.ammo ?? 0).toBeGreaterThan(0);
  expect(supplied.ammoFlow).toBe(true);
  await expect(page.getByTestId("tutorial-ammo-production")).toHaveAttribute("data-state", "complete");

  const enemyId = await page.evaluate(() => window.__XAC_TEST_STATE__!.spawnEnemy("grunt", { x: 37.5, y: 30.5 }));
  expect(enemyId).toBe("enemy_1");
  for (let i = 0; i < 5; i += 1) {
    await page.getByRole("button", { name: /\+40/ }).click();
  }

  const defended = await page.evaluate(() => {
    const snapshot = window.__XAC_TEST_STATE__!.snapshot();
    return {
      enemy: snapshot.enemies.find((enemy) => enemy.id === "enemy_1"),
      turret: snapshot.blocks.find((block) => block.id === "turret_1")
    };
  });
  expect(defended.enemy?.hp ?? 0).toBeLessThan(30);
  expect(defended.turret?.target_id).toBe("enemy_1");
  await expect(page.getByTestId("tutorial-defense")).toHaveAttribute("data-state", "complete");
});

test("drag placement paints wire and conveyor mining lines @smoke @placement", async ({ page }) => {
  await page.goto("/");

  const canvas = page.getByTestId("grid-world").locator("canvas");
  await expect(canvas).toBeVisible();
  await expect(page.getByTestId("build-fragment")).toContainText("Ore Drill");
  await page.getByTestId("build-category-distribution").click();
  await expect(page.getByTestId("build-fragment")).toContainText("Belt Conveyor");
  await page.getByTestId("build-category-factory").click();

  await clickBuild(page, "cpu_node");
  await canvas.click({ position: tileCenter(19, 29) });
  await clickBuild(page, "wire");
  await dragTiles(page, canvas, tileCenter(20, 29), tileCenter(30, 29));
  await clickBuild(page, "conveyor");
  await dragTiles(page, canvas, tileCenter(21, 30), tileCenter(29, 30));
  await clickBuild(page, "drill");
  await canvas.click({ position: tileCenter(20, 30) });

  await page.getByRole("button", { name: /\+40/ }).click();
  await clickBuild(page, "conveyor");
  await dragTiles(page, canvas, tileCenter(18, 30), tileCenter(18, 33));

  const dragged = await page.evaluate(() => {
    const snapshot = window.__XAC_TEST_STATE__!.snapshot();
    return {
      drill: snapshot.blocks.find((block) => block.id === "drill_1"),
      wireCount: snapshot.blocks.filter((block) => block.kind === "wire").length,
      conveyorCount: snapshot.blocks.filter((block) => block.kind === "conveyor").length,
      southConveyors: snapshot.blocks
        .filter((block) => block.kind === "conveyor" && block.pos.x === 18)
        .map((block) => block.dir),
      coreOre: snapshot.blocks.find((block) => block.id === "core_1")?.inventory.items.ore ?? 0,
      placeBlocksCalls: window.__XAC_TEST_STATE__!.calls.filter((call) => call.cmd === "place_blocks")
    };
  });

  expect(dragged.wireCount).toBe(11);
  expect(dragged.conveyorCount).toBe(13);
  expect(dragged.southConveyors).toEqual(["south", "south", "south", "south"]);
  expect(dragged.drill).toEqual(expect.objectContaining({ network_id: 1, effective_cpu_rate: 201 }));
  expect(dragged.coreOre).toBeGreaterThan(40);
  expect(dragged.placeBlocksCalls).toEqual([
    {
      cmd: "place_blocks",
      args: {
        kind: "wire",
        positions: expect.arrayContaining([expect.objectContaining({ x: 20, y: 29 }), expect.objectContaining({ x: 30, y: 29 })]),
        dir: "east"
      }
    },
    {
      cmd: "place_blocks",
      args: {
        kind: "conveyor",
        positions: expect.arrayContaining([expect.objectContaining({ x: 21, y: 30 }), expect.objectContaining({ x: 29, y: 30 })]),
        dir: "east"
      }
    },
    {
      cmd: "place_blocks",
      args: {
        kind: "conveyor",
        positions: expect.arrayContaining([expect.objectContaining({ x: 18, y: 30 }), expect.objectContaining({ x: 18, y: 33 })]),
        dir: "south"
      }
    }
  ]);
});

test("UI mock dispatches carrier drone ammo delivery @drones", async ({ page }) => {
  await page.goto("/");

  const canvas = page.getByTestId("grid-world").locator("canvas");
  await expect(canvas).toBeVisible();

  await clickBuild(page, "drone_port");
  await canvas.click({ position: tileCenter(34, 30) });
  await clickBuild(page, "turret");
  await canvas.click({ position: tileCenter(42, 30) });

  for (let i = 0; i < 10; i += 1) {
    await page.getByRole("button", { name: /\+40/ }).click();
  }

  const delivered = await page.evaluate(() => {
    const snapshot = window.__XAC_TEST_STATE__!.snapshot();
    return {
      drones: snapshot.drones,
      pendingJobs: snapshot.pending_jobs,
      turret: snapshot.blocks.find((block) => block.id === "turret_1"),
      droneFlow: snapshot.item_flows.some(
        (flow) => flow.item === "ammo" && flow.from_entity === "drone_1" && flow.to_entity === "turret_1"
      )
    };
  });

  expect(delivered.drones).toHaveLength(1);
  expect(delivered.pendingJobs).toHaveLength(0);
  expect(delivered.turret?.inventory.items.ammo ?? 0).toBeGreaterThan(0);
  expect(delivered.droneFlow).toBe(true);
  expect(delivered.drones[0].pos.x % 1).not.toBe(0);
  await expect(page.getByTestId("tutorial-drone-delivery")).toHaveAttribute("data-state", "complete");
});

test("quick save and load restore the UI world state @save", async ({ page }) => {
  await page.goto("/");

  const canvas = page.getByTestId("grid-world").locator("canvas");
  await expect(canvas).toBeVisible();

  await clickBuild(page, "drill");
  await canvas.click({ position: tileCenter(20, 30) });
  await expect(page.locator(".metrics")).toContainText("blocks 2");
  await expect(page.locator(".inspector")).toContainText("drill");

  await page.getByRole("button", { name: "Save World", exact: true }).click();
  await expect(page.locator(".log-panel")).toContainText("world saved to quick");

  await page.getByRole("button", { name: "Deconstruct", exact: true }).click();
  await expect(page.locator(".metrics")).toContainText("blocks 1");
  await expect(page.locator(".inspector")).toContainText("Select a block");

  await page.getByRole("button", { name: "Load World", exact: true }).click();
  await expect(page.locator(".metrics")).toContainText("blocks 2");
  await expect(page.locator(".inspector")).toContainText("drill");
  await expect(page.locator(".log-panel")).toContainText("world loaded from quick");

  const restored = await page.evaluate(() => {
    const snapshot = window.__XAC_TEST_STATE__!.snapshot();
    return {
      drill: snapshot.blocks.find((block) => block.id === "drill_1"),
      selectedId: snapshot.selected_id,
      saveCalls: window.__XAC_TEST_STATE__!.calls.filter((call) => call.cmd === "save_world"),
      loadCalls: window.__XAC_TEST_STATE__!.calls.filter((call) => call.cmd === "load_world")
    };
  });
  expect(restored.drill).toEqual(expect.objectContaining({ kind: "drill" }));
  expect(restored.selectedId).toBe("drill_1");
  expect(restored.saveCalls).toEqual([{ cmd: "save_world", args: { slot: "quick" } }]);
  expect(restored.loadCalls).toEqual([{ cmd: "load_world", args: { slot: "quick" } }]);
});

test("wire cutter can sever a CPU network in the UI simulation @network", async ({ page }) => {
  await page.goto("/");

  const canvas = page.getByTestId("grid-world").locator("canvas");
  await expect(canvas).toBeVisible();

  await clickBuild(page, "cpu_node");
  await canvas.click({ position: tileCenter(19, 29) });
  await clickBuild(page, "wire");
  for (let x = 20; x <= 30; x += 1) {
    await canvas.click({ position: tileCenter(x, 29) });
  }
  await clickBuild(page, "drill");
  await canvas.click({ position: tileCenter(20, 30) });

  const connectedDrill = await page.evaluate(() =>
    window.__XAC_TEST_STATE__?.snapshot().blocks.find((block) => block.id === "drill_1")
  );
  expect(connectedDrill).toEqual(expect.objectContaining({ network_id: 1, effective_cpu_rate: 201 }));

  const cutterId = await page.evaluate(() => window.__XAC_TEST_STATE__!.spawnEnemy("wire_cutter", { x: 20.5, y: 29.5 }));
  expect(cutterId).toBe("enemy_1");
  const threatStatus = await page.evaluate(() => window.__XAC_TEST_STATE__!.snapshot().status);
  expect(threatStatus.wire_threats).toBe(1);
  await expect(page.getByTestId("tutorial-cpu-network")).toHaveAttribute("data-state", "complete");

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
  await expect(page.getByTestId("tutorial-wire-cutter")).toHaveAttribute("data-state", "complete");
});
