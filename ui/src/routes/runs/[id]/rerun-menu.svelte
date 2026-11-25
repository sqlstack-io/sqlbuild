<script lang="ts">
	import { goto } from '$app/navigation';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import { buttonVariants } from '$lib/components/ui/button/button.svelte';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import { to } from '$lib/paths';
	import { rerunHref } from '$lib/run/command';
	import { app } from '$lib/state/app.svelte';
	import { cn } from '$lib/utils';
	import { rerunChoices, runFailures } from './utils';
	import type { RunDetailState } from './state.svelte';

	// One rerun action for a run that failed: what to run again, from the failure outwards. Each choice
	// opens the rerun plan, where it is checked before it runs.
	let { view }: { view: RunDetailState } = $props();

	const detail = $derived(view.detail);
	const runId = $derived(view.run?.id ?? '');
	const failures = $derived(runFailures(view.nodes, view.events, view.statements, view.run?.target ?? ''));
	const first = $derived(failures[0]);
	/** Why no choice can run now: still running, or no permission; null when they can. */
	const blocked = $derived(detail ? (rerunChoices(detail, [], (c) => app.can(c)).find((c) => c.mode === 'all')?.disabledReason ?? null) : 'Loading');
	const unfinished = $derived(detail?.rerunFromFailureNodes.length ?? 0);
	type Choice = { label: string; note: string; href: string | null; reason: string | null };
	const choices = $derived.by((): Choice[] => {
		const check = first?.kind === 'check' ? first : null;
		const node = first?.nodeKey ?? null;
		return [
			check
				? { label: 'Failed check only', note: check.name, href: rerunHref([runId], 'selected', [check.name]), reason: null }
				: { label: 'Failed node only', note: node ?? '', href: node ? rerunHref([runId], 'selected', [node]) : null, reason: node ? null : 'Nothing failed' },
			{ label: 'Node and its checks', note: node ?? '—', href: node ? rerunHref([runId], 'selected', [node]) : null, reason: node ? null : 'An end-of-build check has no node' },
			{ label: "Didn't succeed + skipped", note: unfinished ? `${unfinished} nodes` : `${failures.length} ${failures.length === 1 ? 'check' : 'checks'}`, href: rerunHref([runId], 'from_failure'), reason: null },
			{ label: 'Whole run', note: view.run?.jobKey ?? 'selection', href: rerunHref([runId], 'all'), reason: null }
		];
	});
</script>

{#if first}
	<span title={blocked ?? undefined}>
		<DropdownMenu.Root>
			<DropdownMenu.Trigger class={cn(buttonVariants({ variant: 'outline', size: 'sm' }))} disabled={!!blocked} data-testid="rerun-failed">Rerun failed <ChevronDownIcon size={13} /></DropdownMenu.Trigger>
			<DropdownMenu.Content class="w-80" align="end">
				{#each choices as c (c.label)}
					<DropdownMenu.Item disabled={!c.href || !!c.reason} title={c.reason ?? undefined} onSelect={() => c.href && goto(to(c.href))}>
						<span class="choice"><span>{c.label}</span><span class="note">{c.note}</span></span>
					</DropdownMenu.Item>
				{/each}
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	</span>
{/if}

<style>
	.choice {
		white-space: nowrap;
		display: flex;
		width: 100%;
		min-width: 0;
		align-items: baseline;
		justify-content: space-between;
		gap: 12px;
	}
	.note {
		overflow: hidden;
		color: var(--sb-text-faint);
		font-family: var(--font-mono);
		font-size: 11.5px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
