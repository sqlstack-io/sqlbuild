import { expect, test, type Page } from "@playwright/test";

// The People list: one role everywhere changes inline, promoting to admin asks first, the last admin
// can't be demoted, roles that differ by location are "Mixed", and a click opens the person beside it.
test.use({ viewport: { width: 1500, height: 900 } });

const row = (page: Page, name: string) =>
  page.locator("tbody tr", { hasText: name });
const roleOf = (page: Page, name: string) =>
  row(page, name).getByTestId("role-select");

/** Opens a role menu and picks `role`; labels may add "(default)". */
async function pickRole(page: Page, trigger: ReturnType<Page["getByTestId"]>, role: string) {
  await trigger.click();
  await page.getByRole("menuitemradio", { name: new RegExp(`^${role}`) }).click();
}
const roleOption = (page: Page, role: string) =>
  page.getByRole("menuitemradio", { name: new RegExp(`^${role}`) });
const confirmDialog = (page: Page) => page.getByTestId("confirm-dialog");

async function openPeople(page: Page, search = "") {
  await page.goto(`/admin/people?mock${search}`);
  await expect(row(page, "Morgan Diaz")).toBeVisible({ timeout: 20_000 });
}

test("a role changes inline, and promoting to admin asks first", async ({
  page,
}) => {
  await openPeople(page);
  await expect(roleOf(page, "Morgan Diaz")).toHaveText(/^viewer/);
  await pickRole(page, roleOf(page, "Morgan Diaz"), "developer");
  await expect(roleOf(page, "Morgan Diaz")).toHaveText(/^developer/);

  await pickRole(page, roleOf(page, "Morgan Diaz"), "admin");
  await expect(confirmDialog(page)).toContainText(
    "Make Morgan Diaz an admin?",
  );
  await confirmDialog(page).getByRole("button", { name: "Cancel" }).click();
  await expect(confirmDialog(page)).toHaveCount(0);
  await expect(roleOf(page, "Morgan Diaz")).toHaveText(/^developer/);

  await pickRole(page, roleOf(page, "Morgan Diaz"), "admin");
  await confirmDialog(page).getByRole("button", { name: "Make admin" }).click();
  await expect(roleOf(page, "Morgan Diaz")).toHaveText(/^admin/);
  await expect(row(page, "Morgan Diaz").getByRole("alert")).toHaveCount(0);
});

test("the last admin cannot be demoted until someone else is an admin", async ({
  page,
}) => {
  await openPeople(page);
  const demo = roleOf(page, "Demo User");
  await expect(demo).toHaveText(/^admin/);
  await expect(row(page, "Demo User")).toContainText("last admin");
  await demo.click();
  for (const role of ["viewer", "operator", "developer"])
    await expect(roleOption(page, role)).toHaveAttribute("aria-disabled", "true");
  await page.keyboard.press("Escape");

  await pickRole(page, roleOf(page, "Morgan Diaz"), "admin");
  await confirmDialog(page).getByRole("button", { name: "Make admin" }).click();
  await expect(roleOf(page, "Morgan Diaz")).toHaveText(/^admin/);
  await expect(row(page, "Demo User")).not.toContainText("last admin");
  await demo.click();
  await expect(roleOption(page, "developer")).not.toHaveAttribute("aria-disabled", "true");
  await page.keyboard.press("Escape");
});

test("roles that differ by location show Mixed and link to their page", async ({
  page,
}) => {
  await openPeople(page);
  const mixed = row(page, "Alex Rivera").getByTestId("role-mixed");
  await expect(mixed).toHaveText("Mixed");
  await expect(row(page, "Alex Rivera").getByTestId("role-select")).toHaveCount(
    0,
  );
  await mixed.click();
  await expect(page).toHaveURL(/\/admin\/people\/alex/);
});

test("behind SSO, people arrive by signing in and cannot be added", async ({
  page,
}) => {
  await openPeople(page);
  await expect(page.getByTestId("people-arrive")).toHaveText(
    "People appear here the first time they sign in. Pick a role to change it.",
  );
  await expect(page.getByTestId("add-person")).toHaveCount(0);
});

test("with password sign-in, a person is added with a first password and a role", async ({
  page,
}) => {
  await openPeople(page, "&signin=password");
  await expect(page.getByTestId("people-arrive")).toHaveCount(0);
  await page.getByTestId("add-person").click();
  const panel = page.getByTestId("add-person-panel");
  await panel.getByLabel("Sign-in name").fill("quinn");
  await panel.getByLabel("First password").fill("a long first password");
  await pickRole(page, panel.getByTestId("new-person-role"), "developer");
  await panel.getByRole("button", { name: "Add person" }).click();

  await expect(panel).toHaveCount(0);
  await expect(page.getByTestId("person-panel")).toHaveAttribute(
    "aria-label",
    "quinn",
  );
  await expect(roleOf(page, "quinn")).toHaveText(/^developer/);
  await expect(row(page, "quinn")).toContainText("Password");
});

test("with password sign-in, adding someone who already exists says why", async ({
  page,
}) => {
  await openPeople(page, "&signin=password");
  await page.getByTestId("add-person").click();
  const panel = page.getByTestId("add-person-panel");
  await panel.getByLabel("Sign-in name").fill("morgan");
  await panel.getByLabel("First password").fill("a long first password");
  await panel.getByRole("button", { name: "Add person" }).click();

  await expect(panel.getByRole("alert")).toContainText(
    "a person named morgan already exists",
  );
});

test("clicking a person opens their roles and enable/disable beside the list", async ({
  page,
}) => {
  await openPeople(page);
  await row(page, "Jordan Lee").locator("td").nth(3).click();
  const panel = page.getByTestId("person-panel");
  await expect(panel).toContainText("Jordan Lee");
  await expect(
    panel.getByRole("link", { name: /Open their page/ }),
  ).toHaveAttribute("href", /\/admin\/people\/jordan/);
  await page.keyboard.press("Escape");
  await expect(panel).toHaveCount(0);
});

test("a person is disabled from the panel's menu after a confirmation, and a role removed from its row's menu", async ({
  page,
}) => {
  await openPeople(page);
  await row(page, "Jordan Lee").locator("td").nth(3).click();
  const panel = page.getByTestId("person-panel");
  await panel.getByTestId("person-menu").click();
  await page.getByRole("menuitem", { name: "Disable…" }).click();
  await expect(confirmDialog(page)).toContainText("Disable Jordan Lee?");
  await page.keyboard.press("Escape");
  await expect(panel).toBeVisible();
  await panel.getByTestId("person-menu").click();
  await page.getByRole("menuitem", { name: "Disable…" }).click();
  await confirmDialog(page).getByRole("button", { name: "Disable" }).click();
  await expect(row(page, "Jordan Lee")).toContainText("Disabled");

  await page.goto("/admin/people/alex?mock");
  await page.getByTestId("assignment-menu").first().click();
  await page.getByRole("menuitem", { name: "Remove…" }).click();
  await confirmDialog(page).getByRole("button", { name: "Remove" }).click();
  await expect(confirmDialog(page)).toHaveCount(0);
});

test("a role's page shows a service account holder as one, linking to its panel", async ({
  page,
}) => {
  await page.goto("/admin/roles/developer?mock");
  const holder = page.getByRole("link", { name: "orders-deploy" });
  await expect(holder).toHaveAttribute(
    "href",
    /\/admin\/people\?tab=service-accounts&open=orders-deploy/,
    { timeout: 20_000 },
  );
  await expect(page.getByText("service account").first()).toBeVisible();
});
