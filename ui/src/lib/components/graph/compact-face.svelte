<script module lang="ts">
	// Titles are fitted with the card font's real widths once it has loaded: until then the widths are
	// estimated, so every compact title is fitted once more when the fonts are ready (not per zoom).
	let fontsReady = $state(false);
	if (typeof document !== 'undefined') void document.fonts?.ready.then(() => (fontsReady = true));
</script>

<script lang="ts">
	import { LINE_HEIGHT, fitTitle } from './compact-title';
	import { CARD_WIDTH, COMPACT_LEVELS, MAX_TITLE_PX, MIN_TITLE_PX } from '$lib/lineage/fit';

	// A card's compact form, shown by the graph (the `compact` class on its canvas) when zoomed out too far
	// for the card's lines to read: the whole card is its title on the colour of its state, the title
	// between 11 and 16 px on screen, as large as fits. `height` is the card's inner height; the card
	// keeps its size and layout.
	let { title, tone, height, badge }: { title: string; tone: string; height: number; badge?: number } = $props();

	const PAD_X = 6;
	const PAD_Y = 2;
	const EDGE = 4;
	/**
	 * Each level's fit, at the level's lowest zoom, where the card is smallest on screen against its
	 * title: the card inside its state edge and padding (a badge sits on the card's corner), the title
	 * from 11 to 16 px on screen. `px` is the title's size on screen, kept through the level.
	 */
	const fits = $derived(
		(void fontsReady,
		COMPACT_LEVELS.map((zoom) => {
			const fit = fitTitle(title, {
				width: CARD_WIDTH - 2 - EDGE - 2 * PAD_X,
				height: height - 2 * PAD_Y,
				minFont: MIN_TITLE_PX / zoom,
				maxFont: Math.min(MAX_TITLE_PX / zoom, (height - 2 * PAD_Y) / LINE_HEIGHT)
			});
			return { px: Math.min(MAX_TITLE_PX, Math.max(MIN_TITLE_PX, Math.round(fit.font * zoom * 4) / 4)), lines: fit.lines };
		}))
	);
	/** The badge's size on screen. */
	const BADGE_PX = 9;
</script>

<div class="cface" style:--tone={tone} aria-hidden="true">
	{#each fits as fit, level (level)}
		<!-- Sized on screen: the canvas's --sb-title-unzoom undoes the zoom. -->
		<div class="ctitle" data-level={level + 1} style:font-size="calc({fit.px}px * var(--sb-title-unzoom, 1))">
			{#each fit.lines as line, i (i)}<span>{line}</span>{/each}
		</div>
	{/each}
	{#if badge != null}<span class="cbadge" style:font-size="calc({BADGE_PX}px * var(--sb-title-unzoom, 1))">{badge}</span>{/if}
</div>

<style>
	.cface {
		position: absolute;
		inset: 0;
		display: none;
		align-items: center;
		justify-content: center;
		padding: 2px 6px;
		border-left: 4px solid var(--tone);
		background: color-mix(in srgb, var(--tone) 26%, var(--card));
		color: var(--foreground);
		font-family: var(--font-sans);
		pointer-events: none;
	}
	/* Shown by the canvas once the view settles below the compact zoom, with the title fitted for the
	   level of zoom (`data-compact`). */
	:global(.graph-canvas.compact) .cface {
		display: flex;
	}
	.ctitle {
		display: none;
	}
	:global(.graph-canvas[data-compact='1']) .ctitle[data-level='1'],
	:global(.graph-canvas[data-compact='2']) .ctitle[data-level='2'],
	:global(.graph-canvas[data-compact='3']) .ctitle[data-level='3'] {
		display: flex;
		max-width: 100%;
		flex-direction: column;
		align-items: center;
		font-weight: 600;
		letter-spacing: -0.01em;
		line-height: 1.15;
		text-align: center;
		white-space: nowrap;
	}
	.ctitle span {
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	/* On the card's top right corner, outside the title's room. */
	.cbadge {
		position: absolute;
		top: 0;
		right: 0;
		padding: 0 0.3em;
		border: 1px solid var(--node-border);
		border-radius: 2px;
		background: var(--card);
		transform: translate(35%, -55%);
		color: var(--muted-foreground);
		font-variant-numeric: tabular-nums;
		font-weight: 600;
		line-height: 1.2;
	}
</style>
