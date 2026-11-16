import { expect, test, type Page } from '@playwright/test';

// Zoomed out, cards take their compact form: the whole card is its title, as large as fits, on its
// state's colour; never a small title in an empty frame. Each title covers half its card, or, for a
// name too short for that, spans the card's width or height. Cards are drawn in full down to 0.6.
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

/** Each compact card wholly in view whose title doesn't fill it, as problems. */
function emptyCards(): string[] {
	const problems: string[] = [];
	const flow = document.querySelector('.svelte-flow')!.getBoundingClientRect();
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
		const share = ((right - left) * (bottom - top)) / (card.width * card.height);
		const [wide, tall] = [(right - left) / card.width, (bottom - top) / card.height];
		// A title covers half its card, or is as large as the card allows: a name too short to cover half
		// of it spans the card's width or its height.
		if (share < 0.5 && wide < 0.8 && tall < 0.75) problems.push(`${name}: its title covers ${Math.round(share * 100)}% of the card, ${Math.round(wide * 100)}% of its width and ${Math.round(tall * 100)}% of its height`);
		const px = parseFloat(getComputedStyle(title).fontSize) * zoom;
		if (px < 10.95) problems.push(`${name}: its title shows at ${px.toFixed(1)} px`);
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
		await zoomTo(page, 0.65);
		await expect(canvas).not.toHaveClass(/\bcompact\b/);
		if (shots) await page.screenshot({ path: `${shots}/compact-${name}-full.png` });

		for (const [level, zoom] of [['near', 0.55], ['far', 0.4]] as const) {
			await zoomTo(page, zoom);
			await expect(canvas).toHaveClass(/\bcompact\b/);
			if (shots) await page.screenshot({ path: `${shots}/compact-${name}-${level}.png` });
			expect(await page.evaluate(emptyCards), `${name} at ${zoom}`).toEqual([]);
		}
	});
}
