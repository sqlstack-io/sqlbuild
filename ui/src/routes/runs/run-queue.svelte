<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import { openRow, to } from '$lib/paths';
	import type { QueueEntry } from '$lib/api';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import type { Resource } from '$lib/state/resource.svelte';
	import { app } from '$lib/state/app.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { runKindOriginLabel } from '$lib/status';
	import { describeWait, shortRunId, waitTechnical } from '$lib/operate/labels';

	let { entries, resource }: { entries: QueueEntry[]; resource: Resource<unknown> } = $props();

	let error = $state<string | null>(null);
	let cancelling = $state<Set<string>>(new Set());

	async function cancel(runId: string) {
		const scope = app.scope;
		if (!scope) return;
		error = null;
		cancelling = new Set([...cancelling, runId]);
		try {
			await app.client.cancelRun(scope, runId);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			cancelling = new Set([...cancelling].filter((id) => id !== runId));
		}
	}
	const tableColumns: TableColumn[] = [
		{ id: 'pos', label: '#', width: '50px', align: 'right' },
		{ id: 'run', label: 'Run', width: '170px' },
		{ id: 'job', label: 'Job', width: '20%' },
		{ id: 'priority', label: 'Priority', width: '80px', align: 'right', title: 'Higher priority runs go first; equal priority runs go in queue order' },
		{ id: 'since', label: 'Waiting since', width: '130px' },
		{ id: 'why', label: 'Waiting on', minWidth: 240, truncate: false },
		{ id: 'cancel', label: '', width: '100px', align: 'right' }
	];
</script>

{#if error}<div class="queue-error">{error}</div>{/if}
<DataTable columns={tableColumns} rows={entries} key={(q) => q.run.id} {resource} empty="Nothing queued.">
	{#snippet row(q)}
				<tr onclick={(e) => openRow(e, `/runs/${q.run.id}`)}>
					<td class="num muted">{q.position}</td>
					<td>
						<a class="code" href={to(`/runs/${q.run.id}`)} title={q.run.id}>{shortRunId(q.run.id)}</a>
						<div class="cell-desc">{runKindOriginLabel(q.run)} · {q.run.requestedBy.name}</div>
					</td>
					<td>
						{#if q.run.jobKey}<a class="link" href={to(`/jobs/${q.run.jobKey}`)}>{q.run.jobKey}</a>{:else}<span class="muted">Ad hoc</span>{/if}
						{#if q.run.parentRunId}<div class="cell-desc">in group run <a class="link code" href={to(`/runs/${q.run.parentRunId}`)}>{shortRunId(q.run.parentRunId)}</a></div>{/if}
					</td>
					<td class="num">{q.run.priority}</td>
					<td class="muted" title={formatStamp(q.run.queuedAt)}>{formatAgo(q.run.queuedAt, app.now)}</td>
					<td class="waits">
						{#each q.waits as w, i (i)}
							<div class="wait" title={waitTechnical(w)}>
								{#each describeWait(w) as part, j (j)}{#if part.href}<a class="link" class:code={part.code} href={to(part.href)}>{part.text}</a>{:else if part.code}<code>{part.text}</code>{:else}{part.text}{/if}{/each}
							</div>
						{:else}
							<span class="muted" title="Nothing blocks it; starts on the next scheduler pass">Next pass</span>
						{/each}
					</td>
					<td class="num">
						<!-- svelte-ignore a11y_click_events_have_key_events -->
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<span onclick={(e) => e.stopPropagation()}>
							<GatedButton requires="run.cancel" size="sm" busy={cancelling.has(q.run.id)} onclick={() => cancel(q.run.id)}>Cancel</GatedButton>
						</span>
					</td>
				</tr>
	{/snippet}
</DataTable>

<style>
	.wait + .wait {
		margin-top: 3px;
	}
	.queue-error {
		padding: 8px 20px;
		color: var(--sb-error);
		font-size: 12.5px;
	}
</style>
