import { expect, test, type Page } from '@playwright/test';

// Every control on the run page that filters toggles: the first click turns its filter on and says so,
// the second puts the page back exactly as it was. Each text field clears with its ✕ or Esc, keeping focus.
test.use({ viewport: { width: 1500, height: 1000 } });

const RUN = '/runs/run-pa93?mock&failure-mock';
const summary = (page: Page) => page.getByTestId('run-summary');
const logCount = (page: Page) => page.getByRole('tab', { name: /^Log/ }).textContent();
const logFilter = (page: Page) => page.getByLabel('Filter the log');
const selector = (page: Page) => page.getByLabel('Selection', { exact: true });

async function open(page: Page, url = RUN) {
	await page.goto(url);
	await expect(summary(page)).toContainText('built', { timeout: 20_000 });
}

test('the header’s failed-checks figure shows only the failed checks, and again shows everything', async ({ page }) => {
	await open(page);
	const before = await logCount(page);
	const figure = summary(page).getByRole('link', { name: '1 check failed' });
	await figure.click();
	await expect(page.getByTestId('log-filter-active')).toContainText('Showing failed checks · 1 of');
	await expect(figure).toHaveAttribute('aria-current', 'true');
	await figure.click();
	await expect(page.getByTestId('log-filter-active')).toHaveCount(0);
	await expect(figure).not.toHaveAttribute('aria-current', 'true');
	expect(await logCount(page)).toBe(before);
});

test('the header’s built figure narrows the timeline and the log, and again shows everything', async ({ page }) => {
	await open(page);
	const before = await logCount(page);
	const figure = summary(page).getByRole('link', { name: '93 built' });
	await figure.click();
	await expect(page.getByTestId('timeline-filter')).toContainText('Showing built · 93 of 93 nodes');
	await expect(figure).toHaveAttribute('aria-current', 'true');
	await figure.click();
	await expect(page.getByTestId('timeline-filter')).toHaveCount(0);
	expect(await logCount(page)).toBe(before);
});

test('the Errors chip shows only failures, and again puts back the filter text exactly', async ({ page }) => {
	await open(page);
	await logFilter(page).fill('orders');
	const chip = page.getByRole('button', { name: /^Errors \d/ });
	await chip.click();
	await expect(page.getByTestId('errors-only')).toContainText('Showing errors only');
	await expect(chip).toHaveAttribute('aria-pressed', 'true');
	await chip.click();
	await expect(page.getByTestId('errors-only')).toHaveCount(0);
	await expect(logFilter(page)).toHaveValue('orders');
});

test('a Pick chip fills the selection, and again puts back the selection from before', async ({ page }) => {
	await open(page, '/runs/run-4f2a?mock');
	await selector(page).fill('fact_orders ');
	const skipped = page.getByRole('button', { name: 'Skipped', exact: true });
	await skipped.click();
	await expect(skipped).toHaveAttribute('aria-pressed', 'true');
	await expect(page.locator('.sel-field').first()).not.toContainText('fact_orders');
	await skipped.click();
	await expect(skipped).toHaveAttribute('aria-pressed', 'false');
	await expect(page.locator('.sel-field').first()).toContainText('fact_orders');
});

for (const how of ['✕', 'Esc'] as const) {
	test(`the node selector clears with ${how} and keeps focus`, async ({ page }) => {
		await open(page);
		await selector(page).fill('currency');
		const clear = page.getByRole('button', { name: 'Clear selection' });
		await expect(clear).toBeVisible();
		if (how === '✕') await clear.click();
		else await selector(page).press('Escape');
		await expect(selector(page)).toHaveValue('');
		await expect(clear).toHaveCount(0);
		await expect(selector(page)).toBeFocused();
	});

	test(`the log filter clears with ${how} and keeps focus`, async ({ page }) => {
		await open(page);
		await logFilter(page).fill('level:error');
		const clear = page.getByRole('button', { name: 'Clear the log filter' });
		await expect(clear).toBeVisible();
		if (how === '✕') await clear.click();
		else await logFilter(page).press('Escape');
		await expect(logFilter(page)).toHaveValue('');
		await expect(clear).toHaveCount(0);
		await expect(logFilter(page)).toBeFocused();
	});
}
