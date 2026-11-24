<script lang="ts" generics="T">
	import type { Snippet } from 'svelte';
	import * as Popover from '$lib/components/ui/popover';

	// Items on one line: as many as fit the cell, then a "+N" badge whose popup lists them all, on
	// hover or click. A hidden copy of the items measures them, so the count follows the cell's width.
	// Items are keyed by position: a list may name the same item twice.
	let {
		items,
		item,
		label
	}: {
		items: readonly T[];
		item: Snippet<[T]>;
		/** What the items are, for the badge, e.g. "selectors". */
		label: string;
	} = $props();

	const GAP = 4;
	let box = $state<HTMLDivElement>();
	let measure = $state<HTMLDivElement>();
	let shown = $state(Number.MAX_SAFE_INTEGER);
	let open = $state(false);

	/** How many items fit `width` with room for the badge when some are left out; at least one. */
	function fitting(widths: number[], more: number, width: number): number {
		const total = widths.reduce((sum, w, i) => sum + w + (i ? GAP : 0), 0);
		if (total <= width) return widths.length;
		let used = 0;
		let count = 0;
		for (const w of widths) {
			const next = used + (count ? GAP : 0) + w;
			if (next + GAP + more > width) break;
			used = next;
			count++;
		}
		return Math.max(1, count);
	}

	$effect(() => {
		void items.length;
		if (!box || !measure) return;
		const fit = () => {
			if (!box || !measure) return;
			const widths = [...measure.querySelectorAll<HTMLElement>(':scope > .fit-item')].map((e) => e.getBoundingClientRect().width);
			const more = measure.querySelector<HTMLElement>(':scope > .fit-more')?.getBoundingClientRect().width ?? 0;
			shown = fitting(widths, more, box.clientWidth);
		};
		fit();
		const observer = new ResizeObserver(fit);
		observer.observe(box);
		return () => observer.disconnect();
	});
</script>

<div class="fit" bind:this={box}>
	{#each items as entry, i (i)}
		<span class="fit-item" class:first={i === 0} hidden={i >= shown}>{@render item(entry)}</span>
	{/each}
	{#if shown < items.length}
		<Popover.Root bind:open>
			<Popover.Trigger
				openOnHover
				openDelay={150}
				closeDelay={200}
				class="fit-more-badge"
				aria-label="All {items.length} {label}"
				onclick={(e: MouseEvent) => e.stopPropagation()}>+{items.length - shown}</Popover.Trigger
			>
			<Popover.Content class="fit-pop" side="bottom" align="start" sideOffset={4}>
				{#each items as entry, i (i)}<div class="fit-pop-item">{@render item(entry)}</div>{/each}
			</Popover.Content>
		</Popover.Root>
	{/if}
</div>
<div class="fit-measure" bind:this={measure} aria-hidden="true" inert>
	{#each items as entry, i (i)}<span class="fit-item">{@render item(entry)}</span>{/each}
	<span class="fit-more">+{items.length}</span>
</div>

<style>
	.fit {
		display: flex;
		min-width: 0;
		max-width: 100%;
		flex-wrap: nowrap;
		align-items: center;
		gap: 4px;
		overflow: hidden;
	}
	.fit-item {
		display: inline-flex;
		flex: none;
		align-items: center;
	}
	/* The first item shrinks with an ellipsis when even it alone doesn't fit. */
	.fit-item.first {
		min-width: 0;
		flex: 0 1 auto;
		overflow: hidden;
	}
	.fit-item[hidden] {
		display: none;
	}
	.fit-measure {
		position: absolute;
		visibility: hidden;
		display: flex;
		gap: 4px;
		white-space: nowrap;
		pointer-events: none;
		left: -10000px;
		top: 0;
	}
	.fit-more,
	.fit :global(.fit-more-badge) {
		flex: none;
		padding: 0 6px;
		border: 1px solid var(--border);
		border-radius: 2px;
		color: var(--muted-foreground);
		font-size: 11.5px;
		line-height: 18px;
	}
	.fit :global(.fit-more-badge:hover) {
		color: var(--foreground);
	}
	:global(.fit-pop) {
		display: flex;
		width: auto !important;
		max-width: 420px;
		max-height: 320px;
		flex-direction: column;
		gap: 4px !important;
		padding: 8px 10px !important;
		overflow: auto;
		border-radius: 2px !important;
		font-size: 12px !important;
	}
	.fit-pop-item {
		display: flex;
		align-items: center;
	}
</style>
