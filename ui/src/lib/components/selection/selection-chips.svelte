<script lang="ts">
	import NetworkIcon from '@lucide/svelte/icons/network';
	import Button from '$lib/components/ui/button/button.svelte';
	import { to } from '$lib/paths';
	import { SELECT_PARAM, EXCLUDE_PARAM } from '$lib/selection/command';
	import { readingText } from '$lib/selection/reading';
	import { parseSelection } from '$lib/selection/syntax';
	import SelectionChip from './selection-chip.svelte';

	// A saved selection, such as a job's `select` and `exclude`, as the selection bar draws it, read
	// only, with a link that opens it in Lineage.
	let { select, exclude = '', lineage = true }: { select: string; exclude?: string; lineage?: boolean } = $props();

	const chosen = $derived(parseSelection(select));
	const dropped = $derived(parseSelection(exclude));
	const href = $derived.by(() => {
		const params = new URLSearchParams();
		if (select.trim()) params.set(SELECT_PARAM, select.trim());
		if (exclude.trim()) params.set(EXCLUDE_PARAM, exclude.trim());
		return to(`/lineage?${params}`);
	});
</script>

<div class="sel-chips" title="Reads as: {readingText(select, exclude)}">
	{#if chosen.length}
		{#each chosen as t, ti (ti)}
			<span class="grp" class:and={t.parts.length > 1}>
				{#each t.parts as p, pi (pi)}{#if pi}<span class="joint">and</span>{/if}<SelectionChip part={p} tokens={[]} token={0} index={pi} readonly />{/each}
			</span>
		{/each}
	{:else}<span class="faint">Every node</span>{/if}
	{#if dropped.length}
		<span class="faint">except</span>
		{#each dropped as t, ti (ti)}
			<span class="grp" class:and={t.parts.length > 1}>
				{#each t.parts as p, pi (pi)}{#if pi}<span class="joint">and</span>{/if}<SelectionChip part={p} tokens={[]} token={0} index={pi} tone="except" readonly />{/each}
			</span>
		{/each}
	{/if}
	{#if lineage}<Button variant="link" size="inline" href={href}><NetworkIcon size={13} />Open in Lineage</Button>{/if}
</div>

<style>
	.sel-chips {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 5px;
		min-width: 0;
	}
	.grp {
		display: inline-flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 4px;
		max-width: 100%;
	}
	.grp.and {
		padding: 1px;
		border: 1px dashed color-mix(in srgb, var(--sb-warning) 55%, transparent);
		border-radius: 3px;
	}
	.joint {
		color: var(--sb-warning);
		font-size: 11px;
		font-weight: 600;
	}
	.faint {
		color: var(--muted-foreground);
		font-size: 12px;
	}
</style>
