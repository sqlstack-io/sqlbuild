import { expect, test } from '@playwright/test';

// Served over plain HTTP from a host name, a page is not a secure context: `crypto.randomUUID` and
// `navigator.clipboard` don't exist. Both are removed before the app loads, as they would be there.
test.beforeEach(async ({ page }) => {
	await page.addInitScript(() => {
		Object.defineProperty(Crypto.prototype, 'randomUUID', { value: undefined, configurable: true });
		Object.defineProperty(Navigator.prototype, 'clipboard', { get: () => undefined, configurable: true });
		const w = window as unknown as { copied: string[] };
		w.copied = [];
		document.addEventListener('copy', () => w.copied.push(String(document.getSelection() ?? '')));
	});
});

test('a plan, which makes a request ID, opens without randomUUID', async ({ page }) => {
	const errors: string[] = [];
	page.on('pageerror', (e) => errors.push(e.message));
	await page.goto(`/plan?mock&large&cmd=${encodeURIComponent('sqb build --changed')}`);
	expect(await page.evaluate(() => typeof crypto.randomUUID)).toBe('undefined');
	await expect(page.locator('.pane .plan-row').first()).toBeVisible({ timeout: 30_000 });
	expect(errors).toEqual([]);
});

test('copying a selection works without navigator.clipboard', async ({ page }) => {
	await page.goto('/catalog?mock');
	const name = (await page.locator('table.catalog tbody tr .cell-name a').first().innerText({ timeout: 20_000 })).trim();
	await page.goto(`/catalog?mock&select=${encodeURIComponent(name)}`);
	const copy = page.getByRole('button', { name: /^Copy as/ });
	// Enabled once the selection is checked and names a node.
	await expect(page.getByText(/^1 of \d+ nodes$/)).toBeVisible({ timeout: 20_000 });
	await expect(copy).toBeEnabled();
	await copy.click();
	await expect.poll(() => page.evaluate(() => (window as unknown as { copied: string[] }).copied)).toEqual([`sqb build -s ${name}`]);
});
