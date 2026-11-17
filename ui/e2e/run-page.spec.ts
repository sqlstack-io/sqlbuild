import { expect, test } from '@playwright/test';

// The run page while a run goes on: nodes that haven't started are waiting their turn, not left out.
test.use({ viewport: { width: 1600, height: 1000 } });

test('a running run’s nodes that haven’t started read as waiting', async ({ page }) => {
	await page.goto('/runs/run-9d21?mock');
	const panel = page.getByLabel('Nodes by outcome');
	await expect(panel.locator('.group[data-group="waiting"]')).toContainText('Waiting', { timeout: 20_000 });
	await expect(panel.locator('.group[data-group="not_run"]')).toHaveCount(0);
	await expect(page.locator('.gantt .note', { hasText: 'Waiting' }).first()).toBeVisible();
	await expect(page.locator('.gantt .note', { hasText: /Pending|Not run/ })).toHaveCount(0);
});
