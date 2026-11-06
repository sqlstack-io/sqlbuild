<script lang="ts">
	import DownloadIcon from '@lucide/svelte/icons/download';
	import SearchIcon from '@lucide/svelte/icons/search';
	import type { RunOutputLine } from '$lib/api';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import { clockTime } from '$lib/operate/labels';

	let { lines, total, runId, filteredBy }: { lines: RunOutputLine[]; total: number; runId: string; filteredBy: string[] } = $props();

	let stream = $state<'all' | 'stdout' | 'stderr'>('all');
	let query = $state('');
	const q = $derived(query.trim().toLowerCase());
	const shown = $derived(lines.filter((l) => (stream === 'all' || l.stream === stream) && (!q || l.text.toLowerCase().includes(q))));

	function download() {
		const text = shown.map((l) => `${l.at}\t${l.stream}\t${l.text}`).join('\n');
		const url = URL.createObjectURL(new Blob([text + '\n'], { type: 'text/plain' }));
		const a = document.createElement('a');
		a.href = url;
		a.download = `${runId}${stream === 'all' ? '' : `-${stream}`}${filteredBy.length ? `-${filteredBy.join('-')}` : ''}.log`;
		a.click();
		URL.revokeObjectURL(url);
	}
</script>

<div class="filters">
	<SegmentedFilter
		bind:value={stream}
		options={[
			{ value: 'all', label: 'All', count: lines.length },
			{ value: 'stdout', label: 'stdout', count: lines.filter((l) => l.stream === 'stdout').length },
			{ value: 'stderr', label: 'stderr', count: lines.filter((l) => l.stream === 'stderr').length }
		]}
	/>
	<label class="search">
		<SearchIcon size={13} />
		<input bind:value={query} placeholder="Search output" aria-label="Search output" />
	</label>
	<button class="dl" onclick={download} disabled={!shown.length} title="Download the lines shown as a text file"><DownloadIcon size={13} /> Download</button>
</div>
{#if shown.length}
	<pre class="output">{#each shown as line, i (i)}<span class:err={line.stream === 'stderr'}><span class="faint">{clockTime(line.at).slice(0, 8)}</span>  {line.text}
</span>{/each}</pre>
{:else}
	<div class="empty">{total ? 'No output matches these filters.' : 'No output yet.'}</div>
{/if}

<style>
	.filters {
		position: sticky;
		top: 0;
		z-index: 2;
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 16px;
		border-bottom: 1px solid var(--border);
		background: var(--background);
	}
	.search {
		display: flex;
		align-items: center;
		gap: 6px;
		margin-left: auto;
		padding: 3px 8px;
		border: 1px solid var(--border);
		border-radius: 2px;
		color: var(--sb-text-faint);
	}
	.search input {
		width: 200px;
		border: 0;
		outline: 0;
		background: transparent;
		color: var(--foreground);
		font-size: 12px;
	}
	.dl {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		height: 26px;
		padding: 0 10px;
		border: 1px solid var(--border);
		border-radius: 2px;
		font-size: 12px;
	}
	.dl:hover:not(:disabled) {
		background: var(--sb-hover);
	}
	.dl:disabled {
		opacity: 0.5;
	}
	.output {
		padding: 12px 20px;
		color: var(--sb-mono);
		font-family: var(--font-mono);
		font-size: 11.5px;
		line-height: 1.7;
		white-space: pre-wrap;
	}
	.output .err {
		color: var(--sb-mono-err);
	}
	.faint {
		color: var(--sb-text-faint);
	}
	.empty {
		padding: 32px;
		color: var(--sb-text-faint);
		font-size: 12.5px;
		text-align: center;
	}
</style>
