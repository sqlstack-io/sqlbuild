<script lang="ts">
	import { goto } from '$app/navigation';
	import type { QueueEntry } from '$lib/api';
	import LoadRow from '$lib/components/list/load-row.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import type { Resource } from '$lib/state/resource.svelte';
	import { app } from '$lib/state/app.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { runKindOriginLabel } from '$lib/status';
	import { describeWait, overlapHelp, overlapPlain, shortRunId, waitTechnical } from '$lib/operate/labels';

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
</script>

{#if error}<div class="queue-error">{error}</div>{/if}
<div class="sb-list-wrap">
	<table class="sb-list">
		<thead>
			<tr>
				<th class="num">#</th>
				<th>Run</th>
				<th>Job</th>
				<th class="num" title="Higher priority runs go first; equal priority runs go in queue order">Priority</th>
				<th>Waiting since</th>
				<th>Why it waits</th>
				<th>If more requests arrive</th>
				<th></th>
			</tr>
		</thead>
		<tbody>
			{#each entries as q (q.run.id)}
				<tr onclick={() => goto(`/runs/${q.run.id}`)}>
					<td class="num muted">{q.position}</td>
					<td>
						<a class="code" href="/runs/{q.run.id}" title={q.run.id} onclick={(e) => e.stopPropagation()}>{shortRunId(q.run.id)}</a>
						<div class="cell-desc">{runKindOriginLabel(q.run)} · {q.run.requestedBy.name}</div>
					</td>
					<td>
						{#if q.run.jobKey}<a class="link" href="/jobs/{q.run.jobKey}" onclick={(e) => e.stopPropagation()}>{q.run.jobKey}</a>{:else}<span class="muted">Ad hoc</span>{/if}
						{#if q.run.parentRunId}<div class="cell-desc">in group run <a class="link code" href="/runs/{q.run.parentRunId}" onclick={(e) => e.stopPropagation()}>{shortRunId(q.run.parentRunId)}</a></div>{/if}
					</td>
					<td class="num">{q.run.priority}</td>
					<td class="muted" title={formatStamp(q.run.queuedAt)}>{formatAgo(q.run.queuedAt, app.now)}</td>
					<td class="waits">
						{#each q.waits as w, i (i)}
							<div class="wait" title={waitTechnical(w)}>
								{#each describeWait(w) as part, j (j)}{#if part.href}<a class="link" class:code={part.code} href={part.href} onclick={(e) => e.stopPropagation()}>{part.text}</a>{:else if part.code}<code>{part.text}</code>{:else}{part.text}{/if}{/each}
							</div>
						{:else}
							<span class="muted">Nothing in the way; it starts on the next scheduler pass</span>
						{/each}
					</td>
					<td class="muted" title={q.overlap ? overlapHelp[q.overlap.policy] : undefined}>
						{#if q.overlap}<span title="Overlap policy: {q.overlap.policy}">{overlapPlain[q.overlap.policy]}</span><div class="cell-desc">{q.overlap.detail}</div>{:else}—{/if}
					</td>
					<td class="num">
						<!-- svelte-ignore a11y_click_events_have_key_events -->
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<span onclick={(e) => e.stopPropagation()}>
							<GatedButton requires="run.cancel" size="xs" busy={cancelling.has(q.run.id)} onclick={() => cancel(q.run.id)}>Cancel</GatedButton>
						</span>
					</td>
				</tr>
			{:else}
				<LoadRow colspan={8} {resource} empty="Nothing is waiting. Runs wait here while another run is updating the same data, all slots are in use, or an earlier job hasn't finished." />
			{/each}
		</tbody>
	</table>
</div>

<style>
	.waits {
		max-width: 520px;
	}
	.wait + .wait {
		margin-top: 3px;
	}
	.queue-error {
		padding: 8px 20px;
		color: var(--sb-error);
		font-size: 12.5px;
	}
</style>
