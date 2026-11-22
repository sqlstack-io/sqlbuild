import { expect, test, type Page } from "@playwright/test";

// The Service accounts tab beside People: the resting view shows state, and actions live in menus and
// dialogs. A key is shown once, in the dialog that made it; a click on the open row closes its panel.
test.use({ viewport: { width: 1500, height: 900 } });

const row = (page: Page, name: string) =>
  page.getByTestId("service-account-row").filter({ hasText: name });
const panel = (page: Page) => page.getByTestId("service-account-panel");

async function openTab(page: Page, search = "") {
  await page.goto(`/admin/people?mock&tab=service-accounts${search}`);
  await expect(row(page, "orders-deploy")).toBeVisible({ timeout: 20_000 });
}

test("a click opens a service account beside the list, and a click on its row again closes it", async ({
  page,
}) => {
  await openTab(page);
  await row(page, "orders-deploy").locator("td").nth(2).click();
  await expect(panel(page)).toHaveAttribute("aria-label", "orders-deploy");
  await expect(page.getByTestId("service-status")).toHaveText(
    /^Rotating · old key stops in \d+m$/,
  );
  await expect(panel(page).getByTestId("service-key")).toHaveCount(2);
  await row(page, "orders-deploy").locator("td").nth(2).click();
  await expect(panel(page)).toHaveCount(0);
});

test("rotating shows the new key once in its dialog, and the old key works on for the overlap", async ({
  page,
}) => {
  await openTab(page, "&open=ci-runner");
  await panel(page).getByTestId("rotate-key").click();
  const dialog = page.getByTestId("rotate-dialog");
  await dialog.getByTestId("overlap-select").click();
  await page.getByRole("menuitemradio", { name: "In 24 hours" }).click();
  await dialog.getByRole("button", { name: "Rotate key" }).click();

  await expect(dialog.getByTestId("key-reveal")).toContainText(
    "won't be shown again",
  );
  await expect(dialog.locator("code")).toHaveText(/^sqbk_[A-Za-z0-9]{40}$/);
  await dialog.getByRole("button", { name: "Done" }).click();
  await expect(dialog).toHaveCount(0);
  await expect(panel(page).getByTestId("service-key")).toHaveCount(2);
  await expect(page.getByTestId("service-status")).toHaveText(
    /^Rotating · old key stops in \d+h$/,
  );
});

test("a key is stopped and an account disabled from menus, each after a confirmation", async ({
  page,
}) => {
  await openTab(page, "&open=orders-deploy");
  await panel(page).getByTestId("key-menu").nth(1).click();
  await page.getByRole("menuitem", { name: "Stop now…" }).click();
  await page
    .getByTestId("confirm-dialog")
    .getByRole("button", { name: "Stop key" })
    .click();
  await expect(panel(page).getByTestId("service-key")).toHaveCount(1);

  await panel(page).getByTestId("service-menu").click();
  await page.getByRole("menuitem", { name: "Disable…" }).click();
  await page
    .getByTestId("confirm-dialog")
    .getByRole("button", { name: "Disable" })
    .click();
  await expect(page.getByTestId("service-status")).toHaveText(/^Disabled/);
  await expect(row(page, "orders-deploy")).toContainText("Disabled");
});

test("adding a service account shows its first key once", async ({ page }) => {
  await openTab(page);
  await page.getByTestId("add-service-account").click();
  const dialog = page.getByTestId("add-service-account-dialog");
  await dialog.getByLabel("Name").fill("nightly-export");
  await dialog.getByTestId("new-service-role").click();
  await page.getByRole("menuitemradio", { name: "operator" }).click();
  await dialog.getByRole("button", { name: "Add and create key" }).click();

  await expect(dialog.getByTestId("key-reveal")).toBeVisible();
  await dialog.getByRole("button", { name: "Done" }).click();
  await expect(panel(page)).toHaveAttribute("aria-label", "nightly-export");
  await expect(row(page, "nightly-export")).toContainText("operator");
});

test("with no service accounts the tab explains them, and without permission it says why", async ({
  page,
}) => {
  await page.goto("/admin/people?mock&tab=service-accounts&serviceaccounts=none");
  await expect(page.getByTestId("service-accounts-empty")).toContainText(
    "No service accounts yet",
    { timeout: 20_000 },
  );
  await page.goto("/admin/people?mock&as=operator&tab=service-accounts");
  await expect(
    page.getByText("You don't have access to service accounts"),
  ).toBeVisible({ timeout: 20_000 });
});
