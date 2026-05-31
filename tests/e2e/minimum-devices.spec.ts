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
  await expect(page.locator(".inspector")).toContainText("core");

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

  await page.getByRole("button", { name: /Ore Drill/ }).click();
  await expect(page.getByText("Placing drill")).toBeVisible();
  await canvas.click({ position: tileCenter(20, 30) });
  await expect(page.locator(".metrics")).toContainText("blocks 23");
  await expect(page.locator(".metrics")).toContainText("net CPU 200");
  await expect(page.locator(".inspector")).toContainText("drill");
  await expect(page.locator(".inspector")).toContainText("network CPU 200");
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
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /\(module/);
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /xac:drill/);
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /\(call \$output_blocked\)/);
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /\(call \$mine\)/);

  const placeCalls = await page.evaluate(() => {
    return window.__XAC_TEST_STATE__?.calls.filter((call) => call.cmd === "place_block") ?? [];
  });
  expect(placeCalls).toHaveLength(22);
  expect(placeCalls[0].args).toEqual({ kind: "cpu_node", x: 19, y: 29, dir: "east" });
  expect(placeCalls.some((call) => call.args.kind === "wire" && call.args.x === 30 && call.args.y === 29)).toBe(true);
  expect(placeCalls.some((call) => call.args.kind === "conveyor" && call.args.x === 29 && call.args.y === 30)).toBe(true);
  expect(placeCalls.at(-1)?.args).toEqual({ kind: "drill", x: 20, y: 30, dir: "east" });
});
