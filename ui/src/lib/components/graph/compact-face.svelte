<script module lang="ts">
	// Titles are fitted with the card font's real widths once it has loaded: until then the widths are
	// estimated, so every compact title is fitted once more when the fonts are ready (not per zoom).
	let fontsReady = $state(false);
	if (typeof document !== 'undefined') void document.fonts?.ready.then(() => (fontsReady = true));
</script>

<script lang="ts">
	import { LINE_HEIGHT, fitTitle } from './compact-title';
	import { CARD_WIDTH, COMPACT_MIN_FONT } from '$lib/lineage/fit';

	// A card's compact form, shown by the graph (the `compact` class on its canvas) when zoomed out too far
	// for the card's lines to read: the whole card is its title, as large as fits, on the colour of its
	// state. `height` is the card's inner height; the card keeps its size and layout.
	let { title, tone, height, badge }: { title: string; tone: string; height: number; badge?: number } = $props();

	const PAD_X = 6;
	const PAD_Y = 2;
	const EDGE = 4;
	/** A level's title box: the card inside its state edge and padding. (A badge sits on the card's corner.) */
	function box(minFont: number) {
		// As large as one line of it fits the card's height; the title's width limits it from there.
		const inner = height - 2 * PAD_Y;
		return { width: CARD_WIDTH - 2 - EDGE - 2 * PAD_X, height: inner, minFont, maxFont: Math.max(minFont, inner / LINE_HEIGHT) };
	}
	/** The badge's font against the title's smallest. */
	const BADGE = 0.7;
	const near = $derived((void fontsReady, fitTitle(title, box(COMPACT_MIN_FONT.near))));
	const far = $derived((void fontsReady, fitTitle(title, box(COMPACT_MIN_FONT.far))));
</script>

<div class="cface" style:--tone={tone} style:--badge-near="{BADGE * COMPACT_MIN_FONT.near}px" style:--badge-far="{BADGE * COMPACT_MIN_FONT.far}px" aria-hidden="true">
	<div class="ctitle near" style:font-size="{near.font}px">
		{#each near.lines as line, i (i)}<span>{line}</span>{/each}
	</div>
	<div class="ctitle far" style:font-size="{far.font}px">
		{#each far.lines as line, i (i)}<span>{line}</span>{/each}
	</div>
	{#if badge != null}<span class="cbadge">{badge}</span>{/if}
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
	/* Shown by the canvas once the view settles below the compact zoom; `far` is further out, where the
	   title's smallest font grows so it still reads. */
	:global(.graph-canvas.compact) .cface {
		display: flex;
	}
	:global(.graph-canvas.compact.far) .near,
	:global(.graph-canvas.compact:not(.far)) .far {
		display: none;
	}
	.ctitle {
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
		font-size: var(--badge-near);
		font-variant-numeric: tabular-nums;
		font-weight: 600;
		line-height: 1.2;
	}
	:global(.graph-canvas.compact.far) .cbadge {
		font-size: var(--badge-far);
	}
</style>
