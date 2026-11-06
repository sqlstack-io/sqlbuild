<script lang="ts">
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import RunTimeline from '$lib/components/timeline/run-timeline.svelte';
	import RunPanel from '$lib/run/run-panel.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import BulkBar from './bulk-bar.svelte';
	import RunList from './run-list.svelte';
	import RunQueue from './run-queue.svelte';
	import RunsToolbar from './runs-toolbar.svelte';
	import { createRunsState } from './state.svelte';
	import type { TokenKey } from './types';
	import { addToken } from './utils';

	const list = createRunsState();
	const queue = createResource((client, scope) => client.listQueue(scope), ['runs']);
	let panelOpen = $state(false);
	let timelineQuery = $state('');

	const p = $derived(list.params);
	const matchingActive = $derived(p.tab === 'active' ? (list.counts?.active ?? null) : p.tab === 'queued' ? (list.counts?.queued ?? null) : null);
	const filtered = $derived(p.q.trim() !== '' || p.range !== 'any');
	const emptyText = $derived.by(() => {
		if (filtered) return 'No runs match these filters. Clear a filter or widen the date range.';
		if (p.tab === 'active') return 'Nothing is running or waiting right now.';
		if (p.tab === 'queued') return 'Nothing is waiting. Runs queue here while the data they write is busy or all slots are in use.';
		if (p.tab === 'failed') return 'No failed runs.';
		if (p.tab === 'cancelled') return 'No cancelled runs.';
		return 'No runs yet for this target. Start one from Jobs, or with New run.';
	});

	function filterBy(key: TokenKey, value: string) {
		list.update({ q: addToken(p.q, key, value), view: 'list' });
	}
</script>

<AppTopbar title="Runs">
	<GatedButton requires="run.adhoc" variant="default" onclick={() => (panelOpen = true)}>▷ New run</GatedButton>
</AppTopbar>

<RunsToolbar {list} queued={queue.data?.length ?? null} />

{#if p.view === 'timeline'}
	<div class="timeline-page"><RunTimeline bind:query={timelineQuery} /></div>
{:else if p.view === 'queue'}
	<RunQueue entries={queue.data ?? []} resource={queue} />
{:else}
	<BulkBar {list} {matchingActive} />
	<RunList {list} onfilter={filterBy} {emptyText} />
{/if}

<RunPanel bind:open={panelOpen} />

<style>
	.timeline-page {
		display: flex;
		min-height: 0;
		flex: 1;
		flex-direction: column;
	}
</style>
