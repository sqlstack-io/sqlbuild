<script lang="ts">
	// The audit failures a successful run let through (`ignore_audit_failures`): each model and audit,
	// marked ignored. The run counts them; the checks' newest results name them.
	import { plural } from '$lib/format/plural';
	import { to } from '$lib/paths';
	import { app } from '$lib/state/app.svelte';
	import type { Check } from '$lib/api';
	import { ignoredFailuresOf } from '$lib/run/ignored-audits';
	import { ignoresAuditFailures } from '$lib/status';
	import type { RunDetailState } from './state.svelte';

	let { view }: { view: RunDetailState } = $props();

	const run = $derived(view.run);
	const count = $derived(run && ignoresAuditFailures(run.status, run.ignoredAuditFailures) ? run.ignoredAuditFailures : 0);
	const runIds = $derived(new Set([run?.id ?? '', ...(view.detail?.children ?? []).map((c) => c.id)]));
	let checks = $state<Check[] | null>(null);
	let failed = $state(false);

	// The checks are read once, only for a run that let failures through.
	$effect(() => {
		const scope = app.scope;
		if (!count || !scope || checks || failed) return;
		app.client
			.listChecks(scope)
			.then((all) => (checks = all))
			.catch(() => (failed = true));
	});
	const found = $derived(checks ? ignoredFailuresOf(checks, runIds) : []);
	const unnamed = $derived(checks ? Math.max(0, count - found.length) : 0);
</script>

{#if count}
	<section class="ignored-audits" aria-label="Ignored audit failures" data-testid="ignored-audits">
		<p class="head"><b>{plural(count, 'audit failure')} ignored</b> <span class="faint">· audit failures don't stop this job, so the run went on</span></p>
		{#if found.length}
			<ul>
				{#each found as c (c.id)}
					<li>
						<span class="node mono">{c.node ?? '—'}</span>
						<a href={to(`/checks/${c.id}`)} class="audit mono">{c.name}</a>
						{#if c.latest?.violationCount}<span class="faint">{plural(c.latest.violationCount, 'row')} failing</span>{/if}
						<span class="tag">ignored</span>
					</li>
				{/each}
			</ul>
		{/if}
		{#if unnamed && checks}
			<p class="faint more">{unnamed === count ? 'Their checks' : `${plural(unnamed, 'more')}: their checks`} have newer results since. <a href={to('/checks?outcome=ignored')}>Ignored failures on the Checks page</a></p>
		{:else if failed}
			<p class="faint more">Couldn't read the checks. <a href={to('/checks?outcome=ignored')}>Ignored failures on the Checks page</a></p>
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
