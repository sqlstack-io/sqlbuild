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

// A row is painted only within its own box, so a duration that would run past it reads on its bar's left.
for (const width of [760, 1000]) {
	test(`no node's duration runs past its row at ${width}px`, async ({ page }) => {
		await page.setViewportSize({ width, height: 1000 });
		for (const run of ['run-2b70', 'run-4f2a']) {
			await page.goto(`/runs/${run}?mock`);
			await expect(page.locator('.gantt .dur').first()).toBeVisible({ timeout: 20_000 });
			const overflows = await page.locator('.gantt .dur').evaluateAll((labels) =>
				labels
					.filter((label) => label.getBoundingClientRect().right > label.closest('.row')!.getBoundingClientRect().right + 0.5)
					.map((label) => label.textContent)
			);
			expect(overflows, run).toEqual([]);
		}
	});
}
