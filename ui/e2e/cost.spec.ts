import { expect, test, type Page } from '@playwright/test';

// The Cost page: under Operate for a location whose adapter reports cost; the selection (in the URL)
// decides what it shows. Rows open in place, names drill in by narrowing the selection, breadcrumbs
// and Back go up again, a model opens in a drawer, and a refused selection shows nothing.

test.use({ viewport: { width: 1600, height: 1000 } });

const crumbs = (page: Page) => page.getByRole('navigation', { name: 'Breadcrumb' });
const rows = (page: Page) => page.locator('.drill tbody tr[data-kind]');

async function open(page: Page, path: string) {
	await page.goto(path);
	await rows(page).first().waitFor({ timeout: 20_000 });
}

test('Cost is under Operate for a Snowflake location only', async ({ page }) => {
	await page.goto('/?mock');
	const sidebar = page.locator('nav').first();
	await expect(sidebar.getByRole('link', { name: 'Cost' })).toBeVisible({ timeout: 20_000 });
	await page.getByRole('button', { name: 'Switch location' }).click();
	await page.getByRole('menuitemradio', { name: /waffle-shop-dev/ }).click();
	await expect(page.getByRole('button', { name: 'Switch location' })).toContainText('waffle-shop-dev');
	await expect(sidebar.getByRole('link', { name: 'Cost' })).toHaveCount(0);
});

test('a job’s name drills in by selecting it, and the breadcrumb and Back go up again', async ({ page }) => {
	await open(page, '/cost?mock&costs');
	const job = rows(page).first();
	const name = (await job.locator('.row-name').innerText()).trim();
	await job.locator('.row-name').click();
	await expect(page).toHaveURL(new RegExp(`select=job%3A${name}`));
	await expect(crumbs(page)).toContainText(name);
	await expect(page.locator('.selection-bar')).toContainText(`job:${name}`);
	await expect(rows(page).first()).toHaveAttribute('data-kind', 'folder');
	// A folder narrows the selection to the job's models in it.
	await rows(page).first().locator('.row-name').click();
	await expect(page).toHaveURL(/path%3Asql%2Fmodels%2F/);
	await page.goBack();
	await expect(crumbs(page).getByRole('link')).toHaveCount(1);
	await crumbs(page).getByRole('link', { name: 'All' }).click();
	await expect(crumbs(page).getByRole('link')).toHaveCount(0);
	await expect(page.locator('.selection-bar')).not.toContainText('job:');
});

test('a click anywhere on a row, or Enter, opens it in place; Expand and Collapse all', async ({ page }) => {
	await open(page, '/cost?mock&costs');
	const first = rows(page).nth(1);
	await expect(first).toHaveAttribute('aria-expanded', 'false');
	await first.click({ position: { x: 700, y: 10 } });
	await expect(first).toHaveAttribute('aria-expanded', 'true');
	await expect(page.locator('.drill tbody tr[data-kind="folder"]').first()).toBeVisible();
	await first.press('Enter');
	await expect(first).toHaveAttribute('aria-expanded', 'false');
	await page.getByRole('button', { name: 'Expand all' }).click();
	await expect(page.locator('.drill tbody tr[aria-expanded="true"]').first()).toBeVisible();
	await page.getByRole('button', { name: 'Collapse all' }).click();
	await expect(page.locator('.drill tbody tr[aria-expanded="true"]')).toHaveCount(0);
	// Rows that open read as one target: the pointer anywhere on them.
	expect(await first.evaluate((tr) => getComputedStyle(tr).cursor)).toBe('pointer');
});

test('a model opens in the drawer with the jobs that spent its cost, and Escape closes it', async ({ page }) => {
	await open(page, '/cost?mock&costs&select=job:ml_features_daily&flat=1');
	const model = page.locator('.drill tbody tr[data-kind="model"]').first();
	const name = (await model.locator('.row-name').innerText()).trim();
	await model.locator('.row-name').click();
	const drawer = page.getByRole('complementary', { name });
	await expect(drawer).toBeVisible();
	await expect(drawer.getByText('ml_features_daily')).toBeVisible();
	await expect(drawer.locator('table').last().locator('tbody tr').first()).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(drawer).toHaveCount(0);
});

test('Add puts a row into the selection, and a refused selection shows nothing', async ({ page }) => {
	await open(page, '/cost?mock&costs');
	const job = rows(page).nth(2);
	const name = (await job.locator('.row-name').innerText()).trim();
	await job.hover();
	await job.getByRole('button', { name: `Add ${name} to the selection` }).click();
	await expect(page).toHaveURL(new RegExp(`select=job%3A${name}`));
	await page.goto('/cost?mock&costs&select=no_such_model');
	await expect(page.locator('.drill')).toContainText('No nodes selected.', { timeout: 20_000 });
	await expect(rows(page)).toHaveCount(0);
	await expect(page.locator('.sb-msg.err')).toBeVisible();
});

test('the small mock shows its runs’ cost in dollars at the project’s price', async ({ page }) => {
	await open(page, '/cost?mock');
	await expect(page.locator('.strip')).toContainText('$3.00 per credit');
	await expect(rows(page).first().locator('td').nth(1)).toContainText('$');
});
