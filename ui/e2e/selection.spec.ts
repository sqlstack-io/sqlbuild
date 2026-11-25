import { expect, test, type Page } from '@playwright/test';

// The selection bar on Lineage against the mock project: type a selector, pick a value, widen it
// upstream, read the count and the plain-words line, copy the command, leave nodes out, fix an
// unknown value, and open the plan. Set SELECTION_SHOT_DIR to save screenshots there.

test.use({ viewport: { width: 1440, height: 900 }, permissions: ['clipboard-read', 'clipboard-write'] });

const shot = async (page: Page, name: string) => {
	if (process.env.SELECTION_SHOT_DIR) await page.screenshot({ path: `${process.env.SELECTION_SHOT_DIR}/selection-real-${name}.png` });
};

async function openLineage(page: Page, query = '') {
	await page.goto(`/lineage?mock${query}`);
	await page.waitForLoadState('networkidle');
	await expect(page.locator('.selection-bar')).toBeVisible();
}

const field = (page: Page, name = 'Selection') => page.getByRole('textbox', { name, exact: true });
const chips = (page: Page, row = 0) => page.locator('.selection-bar .sel-field').nth(row).locator('.sel-chip');

test('a selection is typed, picked, widened, copied, narrowed and built', async ({ page }) => {
	await openLineage(page);
	await field(page).click();
	await field(page).pressSequentially('tag');
	const suggestions = page.getByRole('listbox', { name: 'Selection suggestions' });
	await expect(suggestions.getByRole('option', { name: /tag:\s*nodes with a tag/ })).toBeVisible();
	const finance = suggestions.getByRole('option').filter({ hasText: /tag:finance/ });
	await expect(finance).toContainText(/\d/);
	await finance.click();

	await expect(chips(page)).toHaveCount(1);
	await expect(chips(page).first()).toHaveText(/tag:finance/);
	await expect(field(page)).toHaveValue('');

	// Upstream from the chip's picker: the + sits inside the chip.
	await chips(page).first().locator('.chip-face').click();
	const panel = page.locator('.chip-panel');
	await expect(panel.getByRole('option', { name: /finance/ })).toHaveAttribute('aria-selected', 'true');
	await panel.getByRole('button', { name: 'Upstream' }).click();
	await page.keyboard.press('Escape');
	await expect(chips(page).first()).toHaveText(/^\+tag:finance/);

	const count = page.locator('.sb-count');
	await expect(count).toHaveText(/^\d+ nodes?$/);
	await expect(page.locator('.sb-msg')).toHaveCount(0);
	await expect.poll(() => new URL(page.url()).searchParams.get('select')).toBe('+tag:finance');

	await page.getByRole('button', { name: /Copy as/ }).click();
	expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('sqb build -s +tag:finance');

	// The except row.
	await page.getByRole('button', { name: 'Exclude' }).click();
	await field(page, 'Except').click();
	await field(page, 'Except').pressSequentially('tag:revenue');
	await page.keyboard.press('Enter');
	await expect(chips(page, 1)).toHaveText([/tag:revenue/]);
	await expect(count).toHaveText(/^\d+ nodes?$/);
	await expect.poll(() => new URL(page.url()).searchParams.get('exclude')).toBe('tag:revenue');
	await page.getByRole('button', { name: /Copy as/ }).click();
	expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('sqb build -s +tag:finance --exclude tag:revenue');

	// Backspace turns the last chip back into its text, and Space makes it a chip again.
	await field(page, 'Except').click();
	await page.keyboard.press('Backspace');
	await expect(field(page, 'Except')).toHaveValue('tag:revenue');
	await expect(chips(page, 1)).toHaveCount(0);
	await page.keyboard.press(' ');
	await expect(chips(page, 1)).toHaveCount(1);
	await page.locator('.graph-panel').click({ position: { x: 5, y: 5 } });
	await shot(page, 'finished');

	await page.getByRole('button', { name: 'Build these' }).click();
	await page.waitForURL(/\/plan\?/);
	expect(new URL(page.url()).searchParams.get('cmd')).toBe('sqb build -s +tag:finance --exclude tag:revenue');
});

test('an unknown value is a red chip with a fix, and Build these waits for it', async ({ page }) => {
	await openLineage(page, '&select=%2Btag%3Afinanse');
	const bad = chips(page).first();
	await expect(bad).toHaveClass(/bad/);
	const alert = page.getByRole('alert');
	await expect(alert).toContainText('No tag called finanse.');
	await expect(alert).toContainText('Did you mean');
	await expect(page.getByRole('button', { name: 'Build these' })).toBeDisabled();
	await expect(page.getByRole('button', { name: /Copy as/ })).toBeDisabled();
	await shot(page, 'error');

	await field(page).click();
	await page.keyboard.press('Tab');
	await expect(chips(page).first()).toHaveText(/^\+tag:finance/);
	await expect(chips(page).first()).not.toHaveClass(/bad/);
	await expect(page.getByRole('alert')).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'Build these' })).toBeEnabled();
});

test('a job page shows its selection as chips that open in Lineage', async ({ page }) => {
	await page.goto('/jobs/revenue_refresh?mock');
	const chips = page.locator('.sel-chips');
	await expect(chips.locator('.sel-chip')).toHaveText([/daily_revenue/, /revenue_report/, /export_revenue_csv/]);
	await chips.getByRole('link', { name: 'Open in Lineage' }).click();
	await page.waitForURL(/\/lineage\?/);
	expect(new URL(page.url()).searchParams.get('select')).toBe('daily_revenue revenue_report export_revenue_csv');
	await expect(page.locator('.selection-bar .sel-chip')).toHaveCount(3);
	await expect(page.locator('.sb-count')).toHaveText(/^\d+ nodes?$/);
});

test('a job selecting folders opens in Lineage without an error', async ({ page }) => {
	await page.goto('/jobs/core_build?mock');
	await expect(page.locator('.sel-chips .sel-chip')).toHaveText([/path:sql\/models\/staging/, /path:sql\/models\/marts/]);
	await page.locator('.sel-chips').getByRole('link', { name: 'Open in Lineage' }).click();
	await page.waitForURL(/\/lineage\?/);
	await expect(page.locator('.sb-count')).toHaveText(/^\d+ nodes?$/);
	await expect(page.getByRole('alert')).toHaveCount(0);
});

test('a folder on the Overview opens in the catalog as a path selection', async ({ page }) => {
	await page.goto('/?mock');
	const marts = page.locator('a.group-row', { hasText: 'marts' }).first();
	await marts.click();
	await page.waitForURL(/\/catalog\?/);
	expect(new URL(page.url()).searchParams.get('select')).toBe('path:sql/models/marts');
	await expect(page.locator('.selection-bar .sel-chip')).toHaveText([/path:sql\/models\/marts/]);
	await expect(page.getByRole('alert')).toHaveCount(0);
	await expect(page.getByRole('button', { name: /^Folder/ })).toHaveCount(0);
});
