<script lang="ts">
	import { getContext } from 'svelte';
	import { fonts } from './fonts.svelte';
	import { measureCompact, pillFit } from './compact-title';
	import { discoveredStep } from '$lib/lineage/discovered-labels';
	import { COMPACT_TAILS, type CompactTails } from './compact-tails';

	// A card's compact form, shown by the graph (the `compact` class on its canvas) when zoomed out too far
	// for the card's lines to read: a one-line pill across the card's full width, a dot in the colour of
	// the card's state, then the part of its name that tells it apart from the other cards in its box
	// (`siblingTails`), the full name on hover; zoomed out further, the dot alone. The card keeps its size
	// and layout, so edges don't move. Sizes are on screen: the canvas's --sb-title-unzoom undoes the zoom.
	let { id, title, tone, kind, badge }: { id: string; title: string; tone: string; kind: 'asset' | 'job' | 'group'; badge?: number } = $props();

	const tails = getContext<CompactTails | undefined>(COMPACT_TAILS);
	const tail = $derived(tails?.tail(id) ?? title);
	// Only a discovered step drops the prefix it shares with its siblings; other names lose their end.
	const lead = $derived(discoveredStep(id) ? (tails?.lead(id) ?? 0) : 0);
	/** The name's size on screen, fitted at the pill stage's lowest zoom; measured again once its font has loaded. */
	const fit = $derived((void fonts.ready, void fonts.version, pillFit(tail, lead, measureCompact)));
	/** The badge's size on screen. */
	const BADGE_PX = 9;
</script>

<div class="cface" class:group={kind === 'group'} style:--tone={tone} aria-hidden="true">
	<div class="pill" title={title} data-kind={kind} data-parent={tails?.parent(id) ?? ''}>
		<span class="dot"></span>
		<span class="nm" data-tail={tail} style:font-size="calc({fit.px}px * var(--sb-title-unzoom, 1))">{fit.text}</span>
		{#if badge != null}<span class="cbadge" style:font-size="calc({BADGE_PX}px * var(--sb-title-unzoom, 1))">{badge}</span>{/if}
	</div>
</div>

<style>
	.cface {
		--u: var(--sb-title-unzoom, 1);
		position: absolute;
		inset: 0;
		display: none;
		align-items: center;
		pointer-events: none;
	}
	/* Shown by the canvas once the view settles below the full cards' zoom; the card's own frame and
	   lines give way to the pill. Node cards show their name alone first, so not at that stage. */
	:global(.graph-canvas.compact:not([data-stage='name'])) .cface,
	:global(.graph-canvas[data-stage='name'] .svelte-flow__node:not(.svelte-flow__node-asset)) .cface {
		display: flex;
	}
	:global(.graph-canvas.compact:not([data-stage='name']) .svelte-flow__node > :has(> .cface)),
	:global(.graph-canvas[data-stage='name'] .svelte-flow__node:not(.svelte-flow__node-asset) > :has(> .cface)) {
		border-color: transparent;
		background: transparent;
		box-shadow: none;
		overflow: visible;
	}
	:global(.graph-canvas.compact:not([data-stage='name']) .svelte-flow__node > :has(> .cface) > :not(.cface)),
	:global(.graph-canvas[data-stage='name'] .svelte-flow__node:not(.svelte-flow__node-asset) > :has(> .cface) > :not(.cface)) {
		visibility: hidden;
	}
	.pill {
		position: relative;
		display: flex;
		box-sizing: border-box;
		width: 100%;
		height: min(calc(26px * var(--u)), 100%);
		align-items: center;
		gap: calc(6px * var(--u));
		padding: 0 calc(8px * var(--u)) 0 calc(6px * var(--u));
		border: calc(1px * var(--u)) solid color-mix(in srgb, var(--foreground) 11%, transparent);
		border-radius: 999px;
		background: var(--card);
		pointer-events: auto;
		white-space: nowrap;
	}
	.group .pill {
		border-color: color-mix(in srgb, var(--chart-3) 45%, transparent);
		background: color-mix(in srgb, var(--chart-3) 10%, var(--card));
	}
	/* Selected: unmistakable at any zoom, a primary edge, a ring around it and a tinted fill. */
	:global(.svelte-flow__node.selected) .cface .pill {
		border-color: var(--primary);
		background: color-mix(in srgb, var(--primary) 16%, var(--card));
		box-shadow: 0 0 0 calc(3px * var(--u)) color-mix(in srgb, var(--primary) 40%, transparent);
	}
	/* The card's state. */
	.dot {
		width: calc(10px * var(--u));
		height: calc(10px * var(--u));
		flex: none;
		border-radius: 50%;
		background: var(--tone);
	}
	.nm {
		min-width: 0;
		overflow: hidden;
		color: var(--foreground);
		font-family: var(--font-node-compact);
		font-weight: 500;
		line-height: 1.2;
		white-space: nowrap;
	}
	/* Zoomed out furthest, the dot alone. */
	:global(.graph-canvas[data-stage='dot']) .pill {
		width: max-content;
		margin: 0 auto;
		padding: calc(5px * var(--u));
	}
	:global(.graph-canvas[data-stage='dot']) .dot {
		width: calc(14px * var(--u));
		height: calc(14px * var(--u));
	}
	:global(.graph-canvas[data-stage='dot']) .nm {
		display: none;
	}
	/* On the pill's top right, outside the name's room. */
	.cbadge {
		position: absolute;
		top: 0;
		right: calc(10px * var(--u));
		padding: 0 0.35em;
		border: 1px solid var(--node-border);
		border-radius: 999px;
		background: var(--card);
		transform: translateY(-55%);
		color: var(--muted-foreground);
		font-variant-numeric: tabular-nums;
		font-weight: 600;
		line-height: 1.3;
	}
</style>
