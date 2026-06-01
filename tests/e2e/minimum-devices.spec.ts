import { expect, test } from "@playwright/test";

type IpcCall = {
  cmd: string;
  args: Record<string, unknown>;
};

declare global {
  interface Window {
    __XAC_TEST_STATE__?: {
      calls: IpcCall[];
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
  await expect(page.getByRole("button", { name: /Core/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Ore Drill/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Belt Conveyor/ })).toBeVisible();

  const canvas = page.getByTestId("grid-world").locator("canvas");
  await expect(canvas).toBeVisible();
  await expect(page.locator(".metrics")).toContainText("blocks 1");
  await expect(page.locator(".metrics")).toContainText("wave 1");
  await expect(page.locator(".metrics")).toContainText("net CPU 120");
  await expect(page.locator(".metrics")).toContainText("core HP 500/500");
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

  await page.getByRole("button", { name: /Wire/ }).click();
  await expect(page.getByText("Placing wire")).toBeVisible();
  for (let x = 20; x <= 30; x += 1) {
    await canvas.click({ position: tileCenter(x, 29) });
  }
  await expect(page.locator(".metrics")).toContainText("blocks 13");

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
  await expect(page.locator(".log-panel")).toContainText("placed Drill at 20,30");

  await page.getByRole("button", { name: /\+40/ }).click();
  await page.getByRole("button", { name: /Ore Drill/ }).click();
  await canvas.click({ position: tileCenter(32, 32) });
  await expect(page.locator(".inspector")).toContainText("core");
  await expect(page.locator(".inspector")).toContainText("ore: 41");
  await expect(page.locator(".inspector")).toContainText("received ore");

  await canvas.click({ position: tileCenter(20, 30) });
  await expect(page.locator(".inspector")).toContainText("drill");

  await page.getByRole("button", { name: "Open", exact: true }).click();
  await expect(page.locator(".behavior-meta")).toContainText("Basic Drill");
  await expect(page.locator(".behavior-meta")).toContainText("builtin.drill.basic");
  await expect(page.locator(".behavior-meta")).toContainText("read-only preset");
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

  await page.getByRole("button", { name: "Deconstruct", exact: true }).click();
  await expect(page.locator(".metrics")).toContainText("blocks 22");
  await expect(page.locator(".inspector")).toContainText("Select a block");
  await expect(page.locator(".log-panel")).toContainText("deconstructed Drill");

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
  expect(placeCalls).toHaveLength(22);
  expect(placeCalls[0].args).toEqual({ kind: "cpu_node", x: 19, y: 29, dir: "east" });
  expect(placeCalls.some((call) => call.args.kind === "wire" && call.args.x === 30 && call.args.y === 29)).toBe(true);
  expect(placeCalls.some((call) => call.args.kind === "conveyor" && call.args.x === 29 && call.args.y === 30)).toBe(true);
  expect(placeCalls.at(-1)?.args).toEqual({ kind: "drill", x: 20, y: 30, dir: "east" });

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
});
