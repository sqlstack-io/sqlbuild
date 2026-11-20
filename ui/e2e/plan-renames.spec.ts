import { expect, test, type Page } from '@playwright/test';

// The plan index under Will run and Won't run, the Renames and migrations section and the plan's width
// (CHI-1563), on a plan shaped like a large real one's (`?mock&summary`).
test.use({ viewport: { width: 1600, height: 1000 } });

const CMD = `cmd=${encodeURIComponent('sqb build --changed')}`;
const renames = (page: Page) => page.getByTestId('plan-renames');
type Call = { section: string; offset: number; limit: number; name?: string };
const calls = (page: Page) => page.evaluate(() => (window as unknown as { __sqbPlanRows?: Call[] }).__sqbPlanRows ?? []);

test('the index counts what will run and what won’t once each, renames as nodes', async ({ page }) => {
	await page.goto(`/plan?mock&summary&${CMD}`);
	await expect(page.locator('.plan-browser')).toBeVisible({ timeout: 30_000 });
	await expect(page.getByTestId('index-will-run')).toHaveText('Will run2,877');
	await expect(page.getByTestId('index-wont-run')).toHaveText('Won’t run97');
	await expect(page.locator('.index .entry[data-section="renames"] .count')).toHaveText('58');
	await expect(page.locator('.index')).not.toContainText('migration steps');
});

test('Renamed lists old name → new name, and migration steps by kind say only what their heading doesn’t', async ({ page }) => {
	await page.goto(`/plan?mock&summary&section=renames&${CMD}`);
	await expect(renames(page).locator('h3')).toHaveText('Renames and migrations 58 nodes · 63 migration steps', { timeout: 30_000 });
	const renamed = renames(page).locator('[data-part="renamed"] .row');
	await expect(renamed).toHaveCount(10);
	await expect(renamed.first().locator('.from')).toHaveText(/_old$/);
	// A rename's usual rebuild goes unsaid; what stays at the old name is said.
	await expect(renames(page).locator('[data-part="renamed"]')).not.toContainText('rebuilt');
	await expect(renamed.nth(1).locator('.detail')).toHaveText('old table archived; a view at the old name until 2026-12-18');
	await expect(renames(page)).not.toContainText('On nodes counted elsewhere');
	expect(await renames(page).locator('.kind h5').allTextContents()).toEqual(['Data moved between models 2', 'Column renames 41', 'Table type changes 12', 'Time travel retention 8']);
	const columns = renames(page).locator('[data-kind="column_migrations"] .row .detail');
	await expect(columns.first()).toHaveText('cust_id → customer_id');
	await expect(renames(page).locator('[data-kind="column_migrations"]')).not.toContainText('renamed in place');
	await expect(columns.nth(4)).toHaveText('sku_code → product_sku·already renamed; recorded');
	// Each kind shows a few rows and loads the rest of its own section when asked.
	await renames(page).locator('[data-kind="column_migrations"]').getByRole('button', { name: /^Show more/ }).click();
	await expect(renames(page).locator('[data-kind="column_migrations"] .row')).toHaveCount(41);
});

test('the filter finds a renamed model by its old name, and a retention change by its models', async ({ page }) => {
	await page.goto(`/plan?mock&summary&section=renames&${CMD}`);
	await expect(renames(page).locator('h3')).toHaveText('Renames and migrations 58 nodes · 63 migration steps', { timeout: 30_000 });
	const filter = renames(page).getByRole('textbox', { name: 'Filter renames and migrations' });
	const old = (await renames(page).locator('[data-part="renamed"] .row .from').first().textContent()) ?? '';
	await filter.fill(old);
	await expect(renames(page).locator('[data-part="renamed"] .row')).toHaveCount(1);
	await expect(renames(page).locator('[data-part="steps"]')).toHaveCount(0);
	// A retention change lists its models, not a model: it is found by any of them.
	await filter.fill('summary_334');
	await expect(renames(page).locator('[data-part="renamed"]')).toHaveCount(0);
	await expect(renames(page).locator('[data-kind="retention"] .row')).toHaveCount(1);
	await expect(renames(page).locator('[data-kind="retention"] .row').first()).toContainText('raised: 1 day → 14 days');
	await filter.fill('nowhere');
	await expect(renames(page)).toContainText('No renames or migrations match “nowhere”.');
	await renames(page).getByRole('button', { name: 'Clear the filter' }).click();
	await expect(renames(page).locator('[data-part="renamed"] .row')).toHaveCount(10);
});

test('the plan stops at its capped width, its toolbar with it, and the graph stays full width', async ({ page }) => {
	await page.setViewportSize({ width: 2000, height: 1000 });
	await page.goto(`/plan?mock&summary&${CMD}`);
	await expect(page.locator('.plan-browser')).toBeVisible({ timeout: 30_000 });
	const right = async (selector: string) => {
		const box = await page.locator(selector).first().boundingBox();
		return box ? box.x + box.width : 0;
	};
	const pane = await right('.plan-browser');
	expect(pane - (await page.locator('.plan-browser').boundingBox())!.x).toBeLessThanOrEqual(230 + 24 + 1100 + 1);
	expect(Math.abs((await right('.summary')) - pane)).toBeLessThanOrEqual(1);
	await page.getByRole('tablist', { name: 'View' }).getByRole('tab', { name: 'Graph' }).click();
	expect(await right('.summary')).toBeGreaterThan(pane + 200);
});

test('a filter typed before a re-plan reads the new plan’s rows', async ({ page }) => {
	await page.goto(`/plan?mock&summary&section=renames&${CMD}`);
	await expect(renames(page).locator('h3')).toHaveText(/58 nodes/, { timeout: 30_000 });
	await renames(page).getByRole('textbox', { name: 'Filter renames and migrations' }).fill('summary_334');
	await expect(renames(page).locator('[data-kind="retention"] .row')).toHaveCount(1);
	const asked = (await calls(page)).filter((c) => c.name === 'summary_334').length;
	await page.getByRole('button', { name: 'Plan again' }).click();
	// The new plan is asked for the same name, and its rows are shown.
	await expect.poll(async () => (await calls(page)).filter((c) => c.name === 'summary_334').length, { timeout: 30_000 }).toBeGreaterThan(asked);
	await expect(renames(page).locator('[data-kind="retention"] .row')).toHaveCount(1);
	await expect(renames(page).getByRole('textbox', { name: 'Filter renames and migrations' })).toHaveValue('summary_334');
});

test('a failed read in Renames and migrations is not repeated on its own; Retry reads again', async ({ page }) => {
	await page.addInitScript(() => ((window as unknown as { __sqbFailPlanRows?: boolean }).__sqbFailPlanRows = true));
	await page.goto(`/plan?mock&summary&section=renames&${CMD}`);
	await expect(renames(page).getByTestId('renames-error')).toContainText('The plan has expired', { timeout: 30_000 });
	await page.waitForTimeout(1500);
	expect((await calls(page)).filter((c) => c.section === 'renamed'), 'one read, not a retry loop').toHaveLength(1);
	await page.evaluate(() => ((window as unknown as { __sqbFailPlanRows?: boolean }).__sqbFailPlanRows = false));
	await renames(page).getByRole('button', { name: 'Retry' }).click();
	await expect(renames(page).locator('[data-part="renamed"] .row')).toHaveCount(10);
	await expect(renames(page).getByTestId('renames-error')).toHaveCount(0);
});

test('a failed page read in Renames and migrations leaves the filter working, and a re-plan reads afresh', async ({ page }) => {
	const fail = (on: boolean) => page.evaluate((v) => ((window as unknown as { __sqbFailPlanRows?: boolean }).__sqbFailPlanRows = v), on);
	await page.addInitScript(() => ((window as unknown as { __sqbFailPlanRows?: boolean }).__sqbFailPlanRows = true));
	await page.goto(`/plan?mock&summary&section=renames&${CMD}`);
	await expect(renames(page).getByTestId('renames-error')).toBeVisible({ timeout: 30_000 });
	await fail(false);
	await renames(page).getByRole('textbox', { name: 'Filter renames and migrations' }).fill('summary_334');
	await expect(renames(page).locator('[data-kind="retention"] .row')).toHaveCount(1);
	await renames(page).getByRole('button', { name: 'Clear the filter' }).click();
	await expect(renames(page).getByTestId('renames-error')).toBeVisible();
	await page.getByRole('button', { name: 'Plan again' }).click();
	await expect(renames(page).locator('[data-part="renamed"] .row')).toHaveCount(10, { timeout: 30_000 });
	await expect(renames(page).getByTestId('renames-error')).toHaveCount(0);
});
