import { expect, test } from '@playwright/test';

// "Planned in 10.6 s" beside "Plan again" opens where the planning time went; a plan without timings shows no button.
const PLAN = `/plan?mock&cmd=${encodeURIComponent('sqb build --changed')}`;
const shots = process.env.SHOTS;
test.use({ viewport: { width: 1440, height: 900 } });

test('the plan time opens a breakdown of its steps', async ({ page }) => {
	await page.goto(PLAN);
	const button = page.getByTestId('plan-timings');
	await expect(button).toHaveText('Planned in 10.6 s', { timeout: 30_000 });
	// It sits just before "Plan again".
	const again = page.getByRole('button', { name: 'Plan again' });
	const [b, a] = [await button.boundingBox(), await again.boundingBox()];
	expect(a!.x - (b!.x + b!.width)).toBeLessThan(24);
	await button.click();
	const popover = page.locator('[data-slot="popover-content"]');
	await expect(popover.getByText('Where the planning time went')).toBeVisible();
	const rows = popover.locator('tbody tr');
	await expect(rows).toHaveCount(9);
	await expect(rows.first()).toHaveText(/Reading warehouse state\s*3\.3\s*31%/);
	await expect(rows.last()).toHaveText(/Other\s*0\.4\s*4%/);
	await expect(popover.locator('tfoot')).toHaveText(/Total\s*10\.6/);
	await expect(popover.locator('.foot')).toHaveText(/^On the location, from the synced warehouse state · \d+ nodes?$/);
	if (shots) await page.screenshot({ path: `${shots}/plan-timings.png` });
	await page.keyboard.press('Escape');
	await expect(popover).toHaveCount(0);
});

test('a plan without timings shows no plan time', async ({ page }) => {
	await page.addInitScript(() => ((window as unknown as { __sqbPlanWithoutTimings: boolean }).__sqbPlanWithoutTimings = true));
	await page.goto(PLAN);
	await expect(page.getByRole('button', { name: 'Plan again' })).toBeVisible({ timeout: 30_000 });
	await expect(page.getByTestId('plan-timings')).toHaveCount(0);
});
