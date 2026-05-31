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

  await page.getByRole("button", { name: /Core/ }).click();
  await expect(page.getByText("Placing core")).toBeVisible();
  await canvas.click({ position: tileCenter(10, 10) });
  await expect(page.locator(".metrics")).toContainText("blocks 2");
  await expect(page.locator(".inspector")).toContainText("core");

  await page.getByRole("button", { name: /Belt Conveyor/ }).click();
  await expect(page.getByText("Placing conveyor")).toBeVisible();
  await canvas.click({ position: tileCenter(21, 30) });
  await expect(page.locator(".metrics")).toContainText("blocks 3");
  await expect(page.locator(".inspector")).toContainText("conveyor");

  await page.getByRole("button", { name: /Ore Drill/ }).click();
  await expect(page.getByText("Placing drill")).toBeVisible();
  await canvas.click({ position: tileCenter(20, 30) });
  await expect(page.locator(".metrics")).toContainText("blocks 4");
  await expect(page.locator(".inspector")).toContainText("drill");
  await expect(page.locator(".log-panel")).toContainText("placed Drill at 20,30");

  await page.getByRole("button", { name: /\+40/ }).click();
  await expect(page.locator(".inspector")).toContainText("ore: 1");
  await expect(page.locator(".inspector")).toContainText("mined ore");

  await page.getByRole("button", { name: "Open", exact: true }).click();
  await expect(page.locator(".behavior-meta")).toContainText("Basic Drill");
  await expect(page.locator(".behavior-meta")).toContainText("builtin.drill.basic");
  await expect(page.locator(".behavior-meta")).toContainText("read-only preset");
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /loop:/);
  await expect(page.getByTestId("code-editor")).toHaveAttribute("data-source", /self\.mine\(\)/);

  const placeCalls = await page.evaluate(() => {
    return window.__XAC_TEST_STATE__?.calls.filter((call) => call.cmd === "place_block") ?? [];
  });
  expect(placeCalls.map((call: IpcCall) => call.args)).toEqual([
    { kind: "core", x: 10, y: 10, dir: "east" },
    { kind: "conveyor", x: 21, y: 30, dir: "east" },
    { kind: "drill", x: 20, y: 30, dir: "east" }
  ]);
});
