<script lang="ts">
	import type { JobDetail } from '$lib/api';

	// A group's members in their derived order: lineage between members, plus
	// declared `after` edges lineage can't see. Members in one stage run side by side.
	let { job }: { job: JobDetail } = $props();

	const stages = $derived.by(() => {
		const max = Math.max(0, ...job.groupMembers.map((m) => m.stage));
		return Array.from({ length: max + 1 }, (_, i) => job.groupMembers.filter((m) => m.stage === i));
	});
</script>

<div class="stages">
	{#each stages as stage, i (i)}
		<div class="stage">
			<div class="label" title="Jobs in the same step run side by side">Step {i + 1}</div>
			{#each stage as m (m.jobKey)}
				<a class="member" href="/jobs/{m.jobKey}">
					<b>{m.jobKey}</b>
					{#if m.upstream.length}
						{#each m.upstream as u (u.jobKey)}
							<span class="edge" class:after={u.source === 'after'} title={u.source === 'after' ? 'Set in the code with after=' : 'Because it reads data that job builds (lineage)'}>
								after {u.jobKey} · {u.source === 'after' ? 'set in code' : 'uses its data'}
							</span>
						{/each}
					{:else}
						<span class="edge">starts first</span>
					{/if}
				</a>
			{/each}
		</div>
		{#if i < stages.length - 1}<div class="arrow">→</div>{/if}
	{/each}
</div>
{#if job.criticalNodes.length}
	<p class="critical">
		Critical nodes (if one fails, the whole group stops):
		{#each job.criticalNodes as n, i (n)}{i ? ', ' : ''}<a href="/catalog/{n}"><code>{n}</code></a>{/each}
	</p>
{/if}

<style>
	.stages {
		display: flex;
		flex-wrap: wrap;
		align-items: stretch;
		gap: 10px;
	}
	.stage {
		display: flex;
		min-width: 200px;
		flex-direction: column;
		gap: 6px;
	}
	.label {
		color: var(--sb-text-faint);
		font-size: 11.5px;
	}
	.member {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 8px 10px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--secondary);
		font-size: 12.5px;
	}
	.member:hover {
		border-color: var(--primary);
	}
	.member b {
		font-family: var(--font-mono);
		font-size: 12px;
		font-weight: 500;
	}
	.edge {
		color: var(--sb-text-faint);
		font-size: 11.5px;
	}
	.edge.after {
		color: var(--sb-warning);
	}
	.arrow {
		align-self: center;
		color: var(--sb-text-faint);
	}
	.critical {
		margin-top: 10px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	code {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	a:hover code {
		color: var(--primary);
	}
</style>
