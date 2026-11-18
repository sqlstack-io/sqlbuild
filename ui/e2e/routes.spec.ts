import { expect, test } from '@playwright/test';

// Edges run through the gaps between cards, never across a card they don't join: each graph's edge
// paths, sampled every few pixels, stay out of every card but their source and target.
test.use({ viewport: { width: 1600, height: 900 } });

/** Each edge that runs across a card it doesn't join, as problems. */
function crossings(): string[] {
	const problems: string[] = [];
	const cards = [...document.querySelectorAll<HTMLElement>('.svelte-flow__node:not(.parent)')].map((n) => ({ id: n.dataset.id!, box: n.getBoundingClientRect() }));
	const ids = new Set(cards.map((c) => c.id));
	for (const edge of document.querySelectorAll<SVGGElement>('.svelte-flow__edge')) {
		const id = edge.dataset.id!;
		// An edge's id is its source and target joined by '-'.
		const ends = new Set([...ids].filter((c) => id.startsWith(`${c}-`) || id.endsWith(`-${c}`)));
		const path = edge.querySelector<SVGPathElement>('path.svelte-flow__edge-path')!;
		const m = path.getScreenCTM()!;
		const length = path.getTotalLength();
		for (const { id: card, box } of cards) {
			if (ends.has(card)) continue;
			for (let at = 0; at <= length; at += 3) {
				const p = path.getPointAtLength(at);
				const [x, y] = [m.a * p.x + m.c * p.y + m.e, m.b * p.x + m.d * p.y + m.f];
				if (x > box.left + 1 && x < box.right - 1 && y > box.top + 1 && y < box.bottom - 1) {
					problems.push(`${id} runs across ${card}`);
					break;
				}
			}
		}
	}
	return problems;
}

const GRAPHS: { name: string; path: string; click?: string }[] = [
	{ name: 'the big group by folder', path: '/jobs/warehouse_nightly?mock' },
	{ name: 'the big group by job', path: '/jobs/warehouse_nightly?mock', click: 'Folders' },
	{ name: 'the Lineage overview', path: '/lineage?mock&large' },
	{ name: 'Lineage', path: '/lineage?mock' }
];

for (const { name, path, click } of GRAPHS) {
	test(`${name}: no edge runs across a card it doesn't join`, async ({ page }) => {
		await page.goto(path);
		await page.locator('.graph-canvas:not(.unfitted) .svelte-flow__edge').first().waitFor({ state: 'attached', timeout: 20_000 });
		if (click) await page.getByRole('button', { name: click }).click();
		await page.waitForTimeout(1200);
		expect(await page.locator('.svelte-flow__edge').count(), 'edges drawn').toBeGreaterThan(3);
		expect(await page.evaluate(crossings)).toEqual([]);
	});
}
