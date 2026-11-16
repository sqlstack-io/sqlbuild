import { expect, test, type Page } from '@playwright/test';
import { COMPACT_LEVELS } from '../src/lib/lineage/fit';

// Zoomed out, cards take their compact form: the whole card is its title, on its state's colour; never
// a small title in an empty frame, nor a shouting one. Each title shows centred, at 11 to 16 px on
// screen, and covers half its card, or spans the card's width or height, or is as large as it may be.
// Cards are drawn in full down to 0.82. Cards are drawn in full down to 0.6.
// With SHOTS set, each graph is also photographed at each zoom: SHOTS=/some/dir npx playwright test e2e/compact.spec.ts
const shots = process.env.SHOTS;

const GRAPHS: { name: string; path: string; click?: string }[] = [
	{ name: 'group', path: '/jobs/warehouse_nightly?mock&awkward' },
	{ name: 'group-members', path: '/jobs/warehouse_nightly?mock&awkward', click: 'Folders' },
	{ name: 'lineage-overview', path: '/lineage?mock&large' },
	{ name: 'lineage', path: '/lineage?mock' }
];

const scaleOf = (page: Page) =>
	page.locator('.svelte-flow__viewport').first().evaluate((v) => Number(/scale\(([\d.]+)\)/.exec((v as HTMLElement).style.transform)?.[1] ?? 1));

/** Zooms with the wheel over the canvas until the zoom is within 0.02 of `target`, then lets the view settle. */
async function zoomTo(page: Page, target: number) {
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

/**
 * Each compact card wholly in view whose title doesn't fill it, as problems. A title keeps its size on
 * screen through its level of zoom (`levels`: each level's lowest zoom, where it was fitted) while the
 * card grows, so how much of the card it fills is judged at that zoom.
 */
function emptyCards(levels: readonly number[]): string[] {
	const problems: string[] = [];
	const flow = document.querySelector('.svelte-flow')!.getBoundingClientRect();
	const level = Number(document.querySelector<HTMLElement>('.graph-canvas')!.dataset.compact);
	const zoom = Number(/scale\(([\d.]+)\)/.exec(document.querySelector<HTMLElement>('.svelte-flow__viewport')!.style.transform)?.[1] ?? 1);
	const cards = [...document.querySelectorAll<HTMLElement>('.svelte-flow__node')].filter((n) => n.querySelector('.cface'));
	let checked = 0;
	for (const node of cards) {
		const card = (node.firstElementChild as HTMLElement).getBoundingClientRect();
		if (card.left < flow.left || card.right > flow.right || card.top < flow.top || card.bottom > flow.bottom) continue;
		const name = (node.querySelector('.name')?.textContent ?? node.dataset.id ?? '').trim();
		const title = [...node.querySelectorAll<HTMLElement>('.cface .ctitle')].find((t) => t.offsetParent !== null);
		if (!title || getComputedStyle(title.closest('.cface')!).display === 'none') {
			problems.push(`${name}: no compact title shown`);
			continue;
		}
		checked++;
		// The title's own box: its lines, each as wide as its text.
		const lines = [...title.querySelectorAll('span')].map((s) => s.getBoundingClientRect());
		const [left, right] = [Math.min(...lines.map((r) => r.left)), Math.max(...lines.map((r) => r.right))];
		const [top, bottom] = [Math.min(...lines.map((r) => r.top)), Math.max(...lines.map((r) => r.bottom))];
		const grown = Math.max(1, zoom / levels[level - 1]);
		const [wide, tall] = [((right - left) / card.width) * grown, ((bottom - top) / card.height) * grown];
		const share = wide * tall;
		const px = parseFloat(getComputedStyle(title).fontSize) * zoom;
		if (px < 10.95 || px > 16.05) problems.push(`${name}: its title shows at ${px.toFixed(1)} px`);
		// As large as the card allows: a name too short to cover half its card spans the card's width or
		// height, or shows at the largest size.
		if (share < 0.5 && wide < 0.8 && tall < 0.75 && px < 15.5)
			problems.push(`${name}: its title covers ${Math.round(share * 100)}% of the card, ${Math.round(wide * 100)}% of its width and ${Math.round(tall * 100)}% of its height, at ${px.toFixed(1)} px`);
		// Centred in the card, beside its state edge.
		const face = title.closest('.cface')!.getBoundingClientRect();
		const edge = parseFloat(getComputedStyle(title.closest('.cface')!).borderLeftWidth) * zoom;
		const [dx, dy] = [(left + right) / 2 - (face.left + edge + face.right) / 2, (top + bottom) / 2 - (face.top + face.bottom) / 2];
		if (Math.abs(dx) > 4 || Math.abs(dy) > 4) problems.push(`${name}: its title is ${Math.round(dx)} px, ${Math.round(dy)} px off the card's centre`);
	}
	if (!checked) problems.push('no compact card in view');
	return problems;
}

test.use({ viewport: { width: 1600, height: 900 } });

for (const { name, path, click } of GRAPHS) {
	test(`${name}: zoomed out, each card is its title on its state's colour`, async ({ page }) => {
		test.setTimeout(90_000);
		await page.goto(path);
		await page.locator('.graph-canvas:not(.unfitted) .svelte-flow__node').first().waitFor({ timeout: 20_000 });
		if (click) await page.getByRole('button', { name: click }).click();
		await page.waitForTimeout(800);
		const canvas = page.locator('.graph-canvas').first();

		// Just above the compact zoom, cards are still drawn in full.
		await zoomTo(page, 0.85);
		await expect(canvas).not.toHaveClass(/\bcompact\b/);
		if (shots) await page.screenshot({ path: `${shots}/compact-${name}-z085-full.png` });

		// Each compact level: just below the switch, and further out.
		for (const zoom of [0.7, 0.5, 0.4]) {
			await zoomTo(page, zoom);
			await expect(canvas).toHaveClass(/\bcompact\b/);
			if (shots) await page.screenshot({ path: `${shots}/compact-${name}-z0${Math.round(zoom * 100)}.png` });
			expect(await page.evaluate(emptyCards, COMPACT_LEVELS), `${name} at ${zoom}`).toEqual([]);
		}
	});
}
