import { expect, test, type Locator, type Page } from "@playwright/test";

// Lineage's folder boxes: hovering an open box only outlines it; a card lights its edges once the
// pointer rests on it; clicking a box's body selects it and lights its edges; its header drags the
// box, and a click on the header closes it without leaving anything dimmed.
test.use({ viewport: { width: 1600, height: 1000 } });

const dimmed = (page: Page) =>
  page.locator(
    ".svelte-flow__node .node.dimmed, .svelte-flow__node .group.dimmed",
  );
const box = (page: Page) =>
  page.locator(".svelte-flow__node-folderBox").first();

/** Lineage of the large project with its first folder opened, the pointer parked off the canvas. */
async function openFirstFolder(page: Page): Promise<Locator> {
  await page.goto("/lineage?mock&large");
  const card = page.locator(".svelte-flow__node-folder").first();
  await expect(card).toBeVisible({ timeout: 30_000 });
  await card.click();
  await expect(box(page)).toBeVisible();
  await page.mouse.move(2, 500);
  await expect(dimmed(page)).toHaveCount(0);
  return box(page);
}

/** A point on the bare canvas, where a click lands on the pane and nothing drawn over it. */
async function emptyCanvasPoint(page: Page): Promise<{ x: number; y: number }> {
  const point = await page.evaluate(() => {
    const pane = document
      .querySelector(".svelte-flow__pane")!
      .getBoundingClientRect();
    for (let y = pane.bottom - 40; y > pane.top + 40; y -= 37)
      for (let x = pane.left + 40; x < pane.right - 40; x += 41)
        if (
          document
            .elementFromPoint(x, y)
            ?.classList.contains("svelte-flow__pane")
        )
          return { x, y };
    return null;
  });
  expect(point, "a bare spot on the canvas").not.toBeNull();
  return point!;
}

/** A point in a box's body, clear of its header and its cards. */
async function bodyPoint(target: Locator): Promise<{ x: number; y: number }> {
  const b = (await target.boundingBox())!;
  return { x: b.x + b.width - 5, y: b.y + b.height - 5 };
}

test("hovering an open box only outlines it, lighting no edges", async ({
  page,
}) => {
  const open = await openFirstFolder(page);
  const { x, y } = await bodyPoint(open);
  await page.mouse.move(x, y);
  await expect(open).toHaveClass(/box-hovered/);
  await page.waitForTimeout(500);
  await expect(dimmed(page)).toHaveCount(0);
});

test("a card lights its edges only once the pointer rests, and leaving clears at once", async ({
  page,
}) => {
  await page.goto("/lineage?mock&large");
  const card = page.locator(".svelte-flow__node-folder").first();
  await expect(card).toBeVisible({ timeout: 30_000 });
  const b = (await card.boundingBox())!;
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
  await page.waitForTimeout(100);
  await expect(dimmed(page)).toHaveCount(0);
  await expect
    .poll(() => dimmed(page).count(), { timeout: 2_000 })
    .toBeGreaterThan(0);
  await page.mouse.move(2, 500);
  await expect(dimmed(page)).toHaveCount(0, { timeout: 150 });
});

test("clicking a box body selects it and lights its edges; the canvas clears it", async ({
  page,
}) => {
  const open = await openFirstFolder(page);
  const { x, y } = await bodyPoint(open);
  await page.mouse.click(x, y);
  await expect(open).toHaveClass(/box-picked/);
  await page.mouse.move(2, 500);
  await expect.poll(() => dimmed(page).count()).toBeGreaterThan(0);
  const bare = await emptyCanvasPoint(page);
  await page.mouse.click(bare.x, bare.y);
  await expect(open).not.toHaveClass(/box-picked/);
  await expect(dimmed(page)).toHaveCount(0);
});

test("dragging a box by its header moves it without closing it", async ({
  page,
}) => {
  const open = await openFirstFolder(page);
  const head = open.locator(".folder-head");
  const before = (await open.boundingBox())!;
  const h = (await head.boundingBox())!;
  await page.mouse.move(h.x + 20, h.y + h.height / 2);
  await page.mouse.down();
  await page.mouse.move(h.x + 80, h.y + h.height / 2 + 60, { steps: 8 });
  await page.mouse.up();
  await expect(box(page)).toBeVisible();
  const after = (await box(page).boundingBox())!;
  expect(Math.round(after.x - before.x)).toBeGreaterThan(30);
  expect(Math.round(after.y - before.y)).toBeGreaterThan(30);
});

for (const [description, before] of [
  ["a click on the header closes the box", async () => {}],
  [
    "closing a hovered box leaves nothing dimmed",
    async (page: Page, open: Locator) => {
      const { x, y } = await bodyPoint(open);
      await page.mouse.move(x, y);
      await page.waitForTimeout(400);
    },
  ],
  [
    "closing a selected box leaves nothing dimmed or selected",
    async (page: Page, open: Locator) => {
      const { x, y } = await bodyPoint(open);
      await page.mouse.click(x, y);
      await expect(open).toHaveClass(/box-picked/);
    },
  ],
] as const) {
  test(description, async ({ page }) => {
    const open = await openFirstFolder(page);
    const id = await open.getAttribute("data-id");
    await before(page, open);
    await open.locator(".folder-head").click();
    await expect(
      page.locator(`.svelte-flow__node[data-id="${id}"]`),
    ).toHaveCount(0);
    await page.waitForTimeout(500);
    await expect(dimmed(page)).toHaveCount(0);
    await expect(page.locator(".svelte-flow__node.box-picked")).toHaveCount(0);
  });
}
