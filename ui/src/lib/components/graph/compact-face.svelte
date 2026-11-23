<script lang="ts">
	import { getContext } from 'svelte';
	import DiamondIcon from '@lucide/svelte/icons/diamond';
	import PlayIcon from '@lucide/svelte/icons/play';
	import SquareSquareIcon from '@lucide/svelte/icons/square-square';
	import { COMPACT_LEVELS, LABEL_MIN_ZOOM, NEW_PILL_LEVELS } from '$lib/lineage/fit';
	import { labelRule } from '$lib/lineage/label-rule.svelte';
	import { cardsMode } from './cards-mode.svelte';
	import { fonts } from './fonts.svelte';
	import { measureCompact, newPillFont, newPillRoom, newPillText, pillFont, pillRoom, pillText } from './compact-title';
	import { COMPACT_TAILS, type CompactTails } from './compact-tails';

	// A card's compact form, shown by the graph (the `compact` class on its canvas) when zoomed out too far
	// for the card's lines to read: a one-line pill across the card, a ring in the colour of the card's
	// state holding its kind's icon, then the part of its name that tells it apart from the other cards
	// in its box (`siblingTails`), the full name on hover. The card keeps its size and layout, so edges don't move.
	// Sizes are on screen: the canvas's --sb-title-unzoom undoes the zoom.
	let { id, title, tone, kind, badge }: { id: string; title: string; tone: string; kind: 'asset' | 'job' | 'group'; badge?: number } = $props();

	const tails = getContext<CompactTails | undefined>(COMPACT_TAILS);
	const tail = $derived(tails?.tail(id) ?? title);
	const lead = $derived(tails?.lead?.(id) ?? 0);
	/**
	 * The name's size on screen at each level, fitted at the level's lowest zoom, where the card is
	 * narrowest; measured again once the name's font has loaded.
	 */
	const mode = $derived(cardsMode());
	const sizes = $derived(
		(void fonts.ready,
		void fonts.version,
		mode === 'new'
			? // Mockup (`cards=new`): the pill's full width; the end cut, or a discovered step's shared prefix.
				NEW_PILL_LEVELS.map((zoom) => {
					const room = newPillRoom(zoom, LABEL_MIN_ZOOM);
					const shared = labelRule.on || /\[[^\]]+\]$/.test(tail) ? lead : 0;
					const px = newPillFont(tail, room, measureCompact);
					return { px, text: newPillText(tail, px, room, measureCompact, shared) };
				})
			: COMPACT_LEVELS.map((zoom) => {
					const px = pillFont(tail, pillRoom(zoom), measureCompact);
					return { px, text: pillText(tail, px, pillRoom(zoom), measureCompact, lead) };
				}))
	);
	/** The icon's size on screen. */
	const ICON = 10;
	/** The badge's size on screen. */
	const BADGE_PX = 9;
</script>

<div class="cface" class:group={kind === 'group'} style:--tone={tone} aria-hidden="true">
	<div class="pill" title={title} data-kind={kind} data-parent={tails?.parent(id) ?? ''}>
		<span class="ring" class:play={kind === 'job'} style:--icon="calc({ICON}px * var(--sb-title-unzoom, 1))">
			{#if kind === 'job'}<PlayIcon strokeWidth={2.5} />{:else if kind === 'group'}<SquareSquareIcon strokeWidth={2.25} />{:else}<DiamondIcon strokeWidth={2.5} />{/if}
		</span>
		{#each sizes as fit, level (level)}<span class="nm" data-level={level + 1} data-tail={tail} style:font-size="calc({fit.px}px * var(--sb-title-unzoom, 1))">{fit.text}</span>{/each}
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
	/* Shown by the canvas once the view settles below the compact zoom; the card's own frame and lines
	   give way to the pill. */
	:global(.graph-canvas.compact) .cface {
		display: flex;
	}
	:global(.graph-canvas.compact .svelte-flow__node > :has(> .cface)) {
		border-color: transparent;
		background: transparent;
		box-shadow: none;
		overflow: visible;
	}
	:global(.graph-canvas.compact .svelte-flow__node > :has(> .cface) > :not(.cface)) {
		visibility: hidden;
	}
	.pill {
		position: relative;
		display: flex;
		box-sizing: border-box;
		width: 100%;
		height: calc(30px * var(--u));
		align-items: center;
		gap: calc(8px * var(--u));
		padding: 0 calc(14px * var(--u)) 0 calc(4px * var(--u));
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
	.ring {
		display: grid;
		box-sizing: border-box;
		width: calc(20px * var(--u));
		height: calc(20px * var(--u));
		flex: none;
		place-items: center;
		border: calc(2px * var(--u)) solid var(--tone);
		border-radius: 50%;
		color: color-mix(in srgb, var(--foreground) 78%, transparent);
	}
	.ring :global(svg) {
		width: var(--icon);
		height: var(--icon);
	}
	/* A play triangle's weight sits a twelfth of its box left of the box's centre: moved right by that,
	   it looks centred in the ring. */
	.ring.play :global(svg) {
		transform: translateX(calc(var(--icon) / 12));
	}
	.nm {
		display: none;
		min-width: 0;
		overflow: hidden;
		color: var(--foreground);
		font-family: var(--font-node-compact);
		font-weight: 500;
		line-height: 1.2;
		white-space: nowrap;
	}
	:global(.graph-canvas[data-compact='1']) .nm[data-level='1'],
	:global(.graph-canvas[data-compact='2']) .nm[data-level='2'],
	:global(.graph-canvas[data-compact='3']) .nm[data-level='3'] {
		display: block;
	}
	/* Mockup (`cards=new`): a status dot rather than an icon ring, tighter padding, the name across the
	   pill's full width; at the dot level, the dot alone. */
	:global(.graph-canvas.cards-new) .pill {
		height: min(calc(26px * var(--u)), 100%);
		gap: calc(6px * var(--u));
		padding: 0 calc(8px * var(--u)) 0 calc(6px * var(--u));
	}
	:global(.graph-canvas.cards-new) .ring {
		width: calc(10px * var(--u));
		height: calc(10px * var(--u));
		border: 0;
		background: var(--tone);
	}
	:global(.graph-canvas.cards-new) .ring :global(svg) {
		display: none;
	}
	:global(.graph-canvas.cards-new[data-stage='dot']) .pill {
		width: max-content;
		margin: 0 auto;
		padding: calc(5px * var(--u));
	}
	:global(.graph-canvas.cards-new[data-stage='dot']) .ring {
		width: calc(14px * var(--u));
		height: calc(14px * var(--u));
	}
	/* Name-only stage: node cards keep their own frame; other cards show their pill. */
	:global(.graph-canvas.nameonly .svelte-flow__node-asset) .cface {
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
