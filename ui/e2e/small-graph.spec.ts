import { expect, test } from '@playwright/test';

// A small job graph opens in full form: whole, at a zoom where its cards are drawn in full, even when
// the fit's margin alone would have pushed it just under the compact zoom.
test.use({ viewport: { width: 1600, height: 900 } });

test('a small job graph opens with its cards in full', async ({ page }) => {
	await page.goto('/jobs/core_build?mock');
	await page.locator('.graph-canvas:not(.unfitted) .svelte-flow__node').first().waitFor({ timeout: 20_000 });
	await page.waitForTimeout(800);
	const view = await page.evaluate(() => {
		const canvas = document.querySelector<HTMLElement>('.graph-canvas')!;
		const pane = canvas.querySelector('.svelte-flow')!.getBoundingClientRect();
		const cut = [...canvas.querySelectorAll<HTMLElement>('.svelte-flow__node')].filter((n) => {
			const r = n.getBoundingClientRect();
			return r.left < pane.left || r.right > pane.right || r.top < pane.top || r.bottom > pane.bottom;
		});
		return {
			zoom: Number(/scale\(([\d.]+)\)/.exec(canvas.querySelector<HTMLElement>('.svelte-flow__viewport')!.style.transform)?.[1]),
			compact: canvas.classList.contains('compact'),
			cards: canvas.querySelectorAll('.svelte-flow__node').length,
			cut: cut.map((n) => n.dataset.id)
		};
	});
	expect(view.cards).toBeGreaterThanOrEqual(5);
	expect(view.compact, `cards compact at zoom ${view.zoom}`).toBe(false);
	expect(view.zoom).toBeGreaterThanOrEqual(0.82);
	expect(view.cut, 'cards outside the canvas').toEqual([]);
});
