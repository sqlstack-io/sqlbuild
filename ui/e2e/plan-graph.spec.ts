import { expect, test, type Page } from "@playwright/test";

// A plan page's Graph view draws its nodes on a canvas that fills the box, on a rerun plan and on a
// build plan alike: the graph's root sized itself to its toolbar there, so the canvas was 0px tall.
test.use({ viewport: { width: 1280, height: 720 } });

/** Every drawn node card, as whether it lies inside the canvas. */
async function cardsInside(page: Page): Promise<boolean[]> {
  return page.locator(".svelte-flow__node").evaluateAll((cards) => {
    const canvas = cards[0]?.closest(".svelte-flow")?.getBoundingClientRect();
    return cards.map((card) => {
      const box = card.getBoundingClientRect();
      return (
        !!canvas &&
        box.height > 0 &&
        box.top >= canvas.top - 0.5 &&
        box.bottom <= canvas.bottom + 0.5
      );
    });
  });
}

for (const [description, path, expectedNodes] of [
  ["a rerun of a failed run", "/plan/rerun?mock&run=run-a7c1&mode=all", 3],
  // The deepest chain of the mock project, sources to a Python asset: a build plan whose graph fits whole.
  [
    "a build plan",
    `/plan?mock&cmd=${encodeURIComponent("sqb build --select +export_revenue_csv")}`,
    10,
  ],
] as const) {
  test(`${description}'s graph draws its nodes on a canvas that fills the box`, async ({
    page,
  }) => {
    await page.goto(path);
    await page
      .getByText("Graph", { exact: true })
      .first()
      .click({ timeout: 30_000 });
    await expect(page.locator(".svelte-flow__node")).toHaveCount(
      expectedNodes,
      { timeout: 20_000 },
    );

    const canvas = await page.locator(".svelte-flow").boundingBox();
    expect(
      canvas?.height ?? 0,
      "the canvas is taller than the toolbar above it",
    ).toBeGreaterThan(250);
    await expect
      .poll(() => cardsInside(page), {
        message: "each node card is drawn within the canvas",
      })
      .toEqual(Array(expectedNodes).fill(true));
  });
}
