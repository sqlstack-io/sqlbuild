import { expect, test, type Page } from "@playwright/test";

// The People list: one role everywhere changes inline, promoting to admin asks first, the last admin
// can't be demoted, roles that differ by location are "Mixed", and a click opens the person beside it.
test.use({ viewport: { width: 1500, height: 900 } });

const row = (page: Page, name: string) =>
  page.locator("tbody tr", { hasText: name });
const roleOf = (page: Page, name: string) =>
  row(page, name).getByTestId("role-select");

async function openPeople(page: Page, search = "") {
  await page.goto(`/admin/people?mock${search}`);
  await expect(row(page, "Morgan Diaz")).toBeVisible({ timeout: 20_000 });
}

test("a role changes inline, and promoting to admin asks first", async ({
  page,
}) => {
  await openPeople(page);
  await expect(roleOf(page, "Morgan Diaz")).toHaveValue("viewer");
  await roleOf(page, "Morgan Diaz").selectOption("developer");
  await expect(roleOf(page, "Morgan Diaz")).toHaveValue("developer");

  const asked: string[] = [];
  page.once(
    "dialog",
    (dialog) => (asked.push(dialog.message()), void dialog.dismiss()),
  );
  await roleOf(page, "Morgan Diaz").selectOption("admin");
  await expect(roleOf(page, "Morgan Diaz")).toHaveValue("developer");
  expect(asked).toEqual([
    expect.stringMatching(/^Make Morgan Diaz \(morgan\) an admin\?/),
  ]);

  page.once("dialog", (dialog) => void dialog.accept());
  await roleOf(page, "Morgan Diaz").selectOption("admin");
  await expect(roleOf(page, "Morgan Diaz")).toHaveValue("admin");
  await expect(row(page, "Morgan Diaz").getByRole("alert")).toHaveCount(0);
});

test("the last admin cannot be demoted until someone else is an admin", async ({
  page,
}) => {
  await openPeople(page);
  const demo = roleOf(page, "Demo User");
  await expect(demo).toHaveValue("admin");
  await expect(row(page, "Demo User")).toContainText("last admin");
  for (const role of ["viewer", "operator", "developer"])
    await expect(demo.locator(`option[value="${role}"]`)).toBeDisabled();

  page.once("dialog", (dialog) => void dialog.accept());
  await roleOf(page, "Morgan Diaz").selectOption("admin");
  await expect(roleOf(page, "Morgan Diaz")).toHaveValue("admin");
  await expect(row(page, "Demo User")).not.toContainText("last admin");
  await expect(demo.locator('option[value="developer"]')).toBeEnabled();
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
  await panel.getByLabel("Role everywhere").selectOption("developer");
  await panel.getByRole("button", { name: "Add person" }).click();

  await expect(panel).toHaveCount(0);
  await expect(page.getByTestId("person-panel")).toHaveAttribute(
    "aria-label",
    "quinn",
  );
  await expect(roleOf(page, "quinn")).toHaveValue("developer");
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
  await expect(panel.getByRole("button", { name: "Disable" })).toBeVisible();
  await expect(
    panel.getByRole("link", { name: /Open their page/ }),
  ).toHaveAttribute("href", /\/admin\/people\/jordan/);
  await page.keyboard.press("Escape");
  await expect(panel).toHaveCount(0);
});
