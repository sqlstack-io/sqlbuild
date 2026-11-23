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

test('copying keeps what the person had selected', async ({ page }) => {
	await page.goto('/catalog?mock');
	const name = (await page.locator('table.catalog tbody tr .cell-name a').first().innerText({ timeout: 20_000 })).trim();
	await page.goto(`/catalog?mock&select=${encodeURIComponent(name)}`);
	await expect(page.getByText(/^1 of \d+ nodes$/)).toBeVisible({ timeout: 20_000 });
	await page.locator('h1').first().selectText();
	const before = await page.evaluate(() => String(getSelection()));
	expect(before.length).toBeGreaterThan(0);
	await page.getByRole('button', { name: /^Copy as/ }).click();
	await expect.poll(() => page.evaluate(() => (window as unknown as { copied: string[] }).copied.length)).toBe(1);
	expect(await page.evaluate(() => String(getSelection()))).toBe(before);
});

test('a one-time secret that cannot be copied is selected, with a hint to copy it by hand', async ({ page }) => {
	await page.addInitScript(() => (document.execCommand = () => false));
	await page.goto('/locations/0d9c6a52-3f1e-4b7a-9c43-5b8e2f61a7d4?mock');
	await page.getByRole('button', { name: 'Create token' }).click({ timeout: 20_000 });
	const issued = page.locator('.issued');
	await issued.getByRole('button', { name: 'Copy' }).click();
	await expect(issued.getByRole('alert')).toContainText('copy it with Ctrl+C');
	expect(await page.evaluate(() => String(getSelection()))).toBe(await issued.locator('code').innerText());
});

test('a key shown in a modal dialog copies without navigator.clipboard, and Copy reads again after a moment', async ({ page }) => {
	await page.goto('/admin/people?mock&tab=service-accounts&open=ci-runner');
	const panel = page.getByTestId('service-account-panel');
	await panel.getByTestId('rotate-key').click({ timeout: 20_000 });
	const dialog = page.getByTestId('rotate-dialog');
	await dialog.getByRole('button', { name: 'Rotate key' }).click();
	const reveal = dialog.getByTestId('key-reveal');
	const key = (await reveal.locator('code').innerText()).trim();
	await reveal.getByRole('button', { name: 'Copy' }).click();
	await expect.poll(() => page.evaluate(() => (window as unknown as { copied: string[] }).copied)).toEqual([key]);
	await expect(reveal.getByRole('button', { name: 'Copied' })).toBeVisible();
	await expect(reveal.getByRole('button', { name: 'Copy' })).toBeVisible({ timeout: 5_000 });
	await reveal.getByRole('button', { name: 'Copy' }).click();
	await expect.poll(() => page.evaluate(() => (window as unknown as { copied: string[] }).copied)).toEqual([key, key]);
	await expect(reveal.getByRole('alert')).toHaveCount(0);
});
