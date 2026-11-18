<script lang="ts">
	// The audit failures a successful run let through (`ignore_audit_failures`): each model and audit,
	// marked ignored, from the run and its group's member runs.
	import { plural } from '$lib/format/plural';
	import { to } from '$lib/paths';
	import { app } from '$lib/state/app.svelte';
	import type { IgnoredAuditFailure } from '$lib/api';
	import { ignoresAuditFailures } from '$lib/status';
	import type { RunDetailState } from './state.svelte';

	let { view }: { view: RunDetailState } = $props();

	const run = $derived(view.run);
	const count = $derived(run && ignoresAuditFailures(run.status, run.ignoredAuditFailures) ? run.ignoredAuditFailures : 0);
	const group = $derived((view.detail?.children ?? []).length > 0);
	let failures = $state<IgnoredAuditFailure[] | null>(null);
	let failed = $state(false);

	// Read once, only for a run that let failures through.
	$effect(() => {
		const scope = app.scope;
		const id = run?.id;
		if (!count || !scope || !id || failures || failed) return;
		app.client
			.listRunIgnoredAudits(scope, id)
			.then((all) => (failures = all))
			.catch(() => (failed = true));
	});
</script>

{#if count}
	<section class="ignored-audits" aria-label="Ignored audit failures" data-testid="ignored-audits">
		<p class="head"><b>{plural(count, 'audit failure')} ignored</b> <span class="faint">· audit failures don't stop this job, so the run went on</span></p>
		{#if failures?.length}
			<ul>
				{#each failures as f, i (`${f.runId}:${f.checkId ?? f.audit}:${i}`)}
					<li>
						{#if group && f.jobKey}<span class="faint mono">{f.jobKey}</span>{/if}
						<span class="node mono">{f.node ?? '—'}{f.column ? `.${f.column}` : ''}</span>
						{#if f.checkId}<a href={to(`/checks/${f.checkId}`)} class="audit mono">{f.audit}</a>{:else}<span class="audit mono">{f.audit}</span>{/if}
						{#if f.violationCount}<span class="faint">{plural(f.violationCount, 'row')} failing</span>{/if}
						<span class="tag">ignored</span>
					</li>
				{/each}
			</ul>
		{/if}
		{#if failed}
			<p class="faint more">Couldn't read the failures. <a href={to('/checks?outcome=ignored')}>Ignored failures on the Checks page</a></p>
		{:else if failures && failures.length < count}
			<p class="faint more">{failures.length ? `${(count - failures.length).toLocaleString()} more no longer recorded` : 'Their results are no longer recorded'}. <a href={to('/checks?outcome=ignored')}>Ignored failures on the Checks page</a></p>
		{/if}
	</section>
{/if}

<style>
	.ignored-audits {
		flex: 0 0 auto;
		padding: 6px 24px 8px;
		border-bottom: 1px solid color-mix(in srgb, var(--sb-warning) 30%, var(--border));
		background: color-mix(in srgb, var(--sb-warning) 5%, var(--background));
		font-size: 12.5px;
	}
	.head {
		margin: 0;
	}
	.head b {
		color: color-mix(in srgb, var(--sb-warning) 80%, var(--foreground));
		font-weight: 600;
	}
	ul {
		display: flex;
		flex-direction: column;
		margin: 4px 0 0;
		padding: 0 0 0 18px;
		max-height: 180px;
		overflow: auto;
	}
	li {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 2px 0;
	}
	.mono {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.node {
		font-weight: 500;
	}
	.audit {
		color: var(--foreground);
	}
	.audit:hover,
	.more a:hover {
		color: var(--primary);
	}
	.more a {
		text-decoration: underline;
	}
	.tag {
		padding: 0 6px;
		border-radius: 999px;
		background: color-mix(in srgb, var(--sb-warning) 14%, transparent);
		color: color-mix(in srgb, var(--sb-warning) 75%, var(--foreground));
		font-size: 11px;
	}
	.faint {
		color: var(--muted-foreground);
	}
	.more {
		margin: 4px 0 0 18px;
	}
</style>
