import { expect, test, type Page } from "@playwright/test";

// The Overview's attention: the panel while something needs it; with only acknowledged items, a slim
// line that opens them; with nothing at all, nothing, as the header and the cards already say so.
test.use({ viewport: { width: 1600, height: 1000 } });

/** Acknowledge every open item, so only acknowledged ones are left. */
async function acknowledgeAll(page: Page): Promise<void> {
  const acks = page
    .locator("section.health .issues")
    .getByRole("button", { name: "Acknowledge" });
  await expect(acks.first()).toBeVisible({ timeout: 20_000 });
  while ((await acks.count()) > 0) {
    await acks.first().click();
    await page.waitForTimeout(150);
  }
}

test("something open shows the panel, never a healthy one", async ({
  page,
}) => {
  await page.goto("/?mock");
  const panel = page.locator("section.health");
  await expect(panel).toContainText("needs attention", { timeout: 20_000 });
  await expect(page.getByText(/is healthy/)).toHaveCount(0);
  await expect(page.getByTestId("attention-quiet")).toHaveCount(0);
});

test("nothing open but some acknowledged shows a slim line that opens them, with undo", async ({
  page,
}) => {
  await page.goto("/?mock");
  await acknowledgeAll(page);
  await expect(page.locator("section.health")).toHaveCount(0);
  const quiet = page.getByTestId("attention-quiet");
  await expect(quiet).toContainText("Nothing needs attention");
  const toggle = quiet.getByRole("button", { name: /^Acknowledged \(\d+\)$/ });
  await expect(quiet.locator(".issue")).toHaveCount(0);
  await toggle.click();
  const rows = quiet.locator(".issue");
  await expect(rows.first()).toContainText("Acknowledged by you");
  await rows.first().getByTestId("attention-undo").click();
  await expect(page.locator("section.health")).toContainText("needs attention");
  await expect(page.getByTestId("attention-quiet")).toHaveCount(0);
});

test("nothing open and nothing acknowledged shows no attention at all", async ({
  page,
}) => {
  await page.goto("/?mock&calm");
  await expect(page.locator(".metric-strip")).toBeVisible({ timeout: 20_000 });
  await expect(page.locator("section.health")).toHaveCount(0);
  await expect(page.getByTestId("attention-quiet")).toHaveCount(0);
  await expect(
    page.getByText(/is healthy|Nothing needs attention/),
  ).toHaveCount(0);
});
