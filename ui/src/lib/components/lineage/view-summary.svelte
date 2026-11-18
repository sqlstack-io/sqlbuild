<script lang="ts">
	import { plural } from '$lib/format/plural';
	import InspectorSection from './inspector-section.svelte';
	import { nodeKindPlural } from '$lib/build-area/labels';
	import type { ViewSummary } from '$lib/lineage/view-summary';

	// A graph's sidebar with nothing selected: what is in view, the filters narrowing it, and what the colours mean.
	let { summary, filters = [] }: { summary: ViewSummary; filters?: string[] } = $props();
	const key = [
		{ tone: 'var(--sb-success)', label: 'Succeeded', title: 'Last build succeeded' },
		{ tone: 'var(--sb-error)', label: 'Failed', title: 'Last build failed' },
		{ tone: 'var(--sb-warning)', label: 'Warnings', title: 'Built with warnings' },
		{ tone: 'var(--sb-stale)', label: 'Changed', title: 'Changed since its last build' },
		{ tone: 'var(--sb-behind)', label: 'Behind', title: 'Behind an upstream rebuild' },
		{ tone: 'var(--sb-text-faint)', label: 'Not built or source', title: 'Not built yet, or a source' }
	];
</script>

<InspectorSection title="In view">
	{#snippet meta()}{plural(summary.total, 'node')}{/snippet}
	<dl class="facts">
		{#each summary.byKind as k (k.kind)}<div><dt>{nodeKindPlural[k.kind]}</dt><dd>{k.count.toLocaleString('en')}</dd></div>{/each}
	</dl>
	<p class="needs">
		{#if summary.changed || summary.behind}
			Needs building:
			{#if summary.changed}<span class="changed">{summary.changed.toLocaleString('en')} changed</span>{/if}
			{#if summary.behind}<span class="behind">{summary.changed ? '+' : ''}{summary.behind.toLocaleString('en')} behind</span>{/if}
		{:else}
			Nothing in view needs building.
		{/if}
		{#if summary.failed}<span class="failed">· {summary.failed.toLocaleString('en')} failed</span>{/if}
	</p>
</InspectorSection>
{#if filters.length}
	<InspectorSection title="Filters">
		<ul class="filters">
			{#each filters as f (f)}<li>{f}</li>{/each}
		</ul>
	</InspectorSection>
{/if}
<InspectorSection title="Colour key">
	<ul class="key">
		{#each key as k (k.label)}<li title={k.title}><i style:background={k.tone}></i>{k.label}</li>{/each}
	</ul>
</InspectorSection>
<p class="hint">Select a node to see its details.</p>

<style>
	.facts div {
		display: flex;
		justify-content: space-between;
		padding: 3px 0;
		font-size: 12.5px;
	}
	.facts dt {
		color: var(--muted-foreground);
	}
	.facts dd {
		font-variant-numeric: tabular-nums;
	}
	.needs {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
		margin-top: 10px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.changed {
		color: var(--sb-stale);
		font-weight: 500;
	}
	.behind {
		color: var(--sb-behind);
	}
	.failed {
		color: var(--sb-error);
	}
	.filters li {
		padding: 2px 0;
		font-size: 12.5px;
	}
	.key li {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 2px 0;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.key i {
		width: 8px;
		height: 8px;
		flex: 0 0 8px;
		border-radius: 50%;
	}
	.hint {
		padding: 12px 16px;
		color: var(--sb-text-faint);
		font-size: 12px;
	}
</style>
