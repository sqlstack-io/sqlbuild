import { expect, test, type Page } from '@playwright/test';
import { interactionTasks, median } from './support/main-thread';

// The Cost page on a large project (`?mock&costs`: 2,281 models, 30 jobs): opening a row and moving
// to another level each keep the main thread's longest task under a frame (16 ms), from the input
// to the frame that draws it. Each is judged on the median of five, so a one-off stall of a shared
// machine doesn't fail it; the timings are printed either way.

test.use({ viewport: { width: 1600, height: 1000 } });
test.describe.configure({ mode: 'serial' });

const FRAME_MS = 16;
const TIMES = 5;

async function open(page: Page, path: string) {
	await page.goto(path);
	await page.locator('tr.expandable').first().waitFor({ timeout: 20_000 });
	await page.waitForTimeout(800);
}

/** The longest task of each of `TIMES` runs of `act`, after `before` puts the page back. */
async function judge(page: Page, name: string, before: () => Promise<void>, act: () => Promise<void>) {
	const longest: number[] = [];
	for (let i = 0; i < TIMES; i++) {
		await before();
		await page.waitForTimeout(300);
		longest.push((await interactionTasks(page, act))[0] ?? 0);
	}
	console.info(`Cost, ${name}: longest task ${longest.map((t) => t.toFixed(1)).join(', ')} ms`);
	expect(median(longest), `ms of the longest main-thread task when ${name}`).toBeLessThan(FRAME_MS);
}

test('opening a row of the Cost tree keeps every task under a frame', async ({ page }) => {
	test.setTimeout(120_000);
	await open(page, '/cost?mock&costs&still');
	const row = page.locator('tr.expandable').nth(1);
	// With the mouse, anywhere on the row, and with Enter; each run opens the row closed before.
	await judge(
		page,
		'opening a job with a click',
		() => row.hover({ position: { x: 600, y: 10 } }),
		async () => {
			await page.mouse.down();
			await page.mouse.up();
		}
	);
	await judge(
		page,
		'opening a job with Enter',
		() => row.focus(),
		() => page.keyboard.press('Enter')
	);
});

test('moving to another level of the Cost tree keeps every task under a frame', async ({ page }) => {
	test.setTimeout(120_000);
	await open(page, '/cost?mock&costs&still');
	const crumbs = page.getByRole('navigation', { name: 'Breadcrumb' });
	const jobName = page.locator('tr.expandable .row-name').first();
	await judge(
		page,
		'drilling into a job',
		async () => {
			if ((await crumbs.getByRole('link').count()) > 0) await crumbs.getByRole('link', { name: 'All' }).click();
			await jobName.waitFor();
			await jobName.hover();
		},
		() => jobName.click()
	);
	await judge(
		page,
		'going back up to every job',
		async () => {
			if ((await crumbs.getByRole('link').count()) === 0) await page.locator('tr.expandable .row-name').first().click();
			await crumbs.getByRole('link', { name: 'All' }).hover();
		},
		() => crumbs.getByRole('link', { name: 'All' }).click()
	);
});
