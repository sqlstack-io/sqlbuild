import { expect, test, type Page } from '@playwright/test';
import { interactionCost, median, type InteractionCost } from './support/main-thread';

// The Cost page on a large project (`?mock&costs`: 2,281 models, 30 jobs): opening a row and moving
// to another level each run at most a frame (16 ms) of the page's own script, and draw their result
// within 50 ms of the input. Each is judged on the median of five, so a one-off stall of a shared
// machine doesn't fail it; the timings are printed either way.

test.use({ viewport: { width: 1600, height: 1000 } });
test.describe.configure({ mode: 'serial' });

/** The page's own script for one interaction, at most: one frame. */
const SCRIPT_MS = 16;
/** From the input to the frame that draws its result, at most, with style, layout and paint. */
const TO_FRAME_MS = 50;
const TIMES = 5;

async function open(page: Page, path: string) {
	await page.goto(path);
	await page.locator('tr.expandable').first().waitFor({ timeout: 20_000 });
	await page.waitForTimeout(800);
}

/** The cost of each of `TIMES` runs of `act`, after `before` puts the page back, judged on their medians. */
async function judge(page: Page, name: string, before: () => Promise<void>, act: () => Promise<void>) {
	const runs: InteractionCost[] = [];
	for (let i = 0; i < TIMES; i++) {
		await before();
		await page.waitForTimeout(300);
		runs.push(await interactionCost(page, act));
	}
	const list = (pick: (r: InteractionCost) => number) => runs.map((r) => pick(r).toFixed(1)).join(', ');
	console.info(`Cost, ${name}: script ${list((r) => r.script)} ms; input to frame ${list((r) => r.toFrame)} ms`);
	expect.soft(median(runs.map((r) => r.script)), `ms of the page's script when ${name}`).toBeLessThan(SCRIPT_MS);
	expect.soft(median(runs.map((r) => r.toFrame)), `ms from the input to the frame when ${name}`).toBeLessThan(TO_FRAME_MS);
}

test('opening a row of the Cost tree stays within a frame of script', async ({ page }) => {
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

test('moving to another level of the Cost tree stays within a frame of script', async ({ page }) => {
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
