import { expect, type Page } from '@playwright/test';

// Zooming a graph to an exact level with the mouse wheel, for the specs that look at it zoomed out.

export const scaleOf = (page: Page) =>
	page.locator('.svelte-flow__viewport').first().evaluate((v) => Number(/scale\(([\d.]+)\)/.exec((v as HTMLElement).style.transform)?.[1] ?? 1));

/** Zooms with the wheel over the canvas until the zoom is within 0.02 of `target`, then lets the view settle. */
export async function zoomTo(page: Page, target: number) {
	const box = (await page.locator('.svelte-flow').first().boundingBox())!;
	await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
	for (let i = 0; i < 200; i++) {
		const zoom = await scaleOf(page);
		if (Math.abs(zoom - target) <= 0.02) break;
		// d3's wheel zoom: a factor of 2^(-deltaY / 500).
		const delta = Math.max(-60, Math.min(60, Math.log2(zoom / target) * 500 * 0.8));
		await page.mouse.wheel(0, Math.abs(delta) < 4 ? Math.sign(delta) * 4 : delta);
		await page.waitForTimeout(30);
	}
	expect(Math.abs((await scaleOf(page)) - target), `zoomed to ${target}`).toBeLessThanOrEqual(0.02);
	await page.waitForTimeout(600);
}
