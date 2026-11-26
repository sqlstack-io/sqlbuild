<script lang="ts">
	import { goto } from '$app/navigation';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import Button, { buttonVariants } from '$lib/components/ui/button/button.svelte';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import { to } from '$lib/paths';
	import { rerunHref } from '$lib/run/command';
	import { app } from '$lib/state/app.svelte';
	import { rerunChoices } from './utils';
	import type { RunDetailState } from './state.svelte';
	import { rerunRows } from './utils/rerun-rows';
	import RotateCcwIcon from '@lucide/svelte/icons/rotate-ccw';

	// What to run again, from the failure outwards, on the app's own menu: the first row is the default,
	// rows that don't apply are left out, and two rows that would run the same thing show once. Each opens
	// the rerun plan, where it is checked before it runs.
	let { view }: { view: RunDetailState } = $props();
	// Prominence follows usefulness: tinted (never red) when the run didn't succeed, quiet like Details when it did.
	const succeeded = $derived(view.run?.status === 'success' || view.run?.status === 'warn');

	const detail = $derived(view.detail);
	const runId = $derived(view.run?.id ?? '');
	const failures = $derived(view.failures);
	/** Why nothing can run again now: still running, or no permission; null when it can. */
	const blocked = $derived(detail ? (rerunChoices(detail, [], (c) => app.can(c)).find((c) => c.mode === 'all')?.disabledReason ?? null) : 'Loading');
	const rows = $derived(
		rerunRows({
			failures,
			unfinished: detail?.rerunFromFailureNodes ?? [],
			total: view.nodes.length,
			jobKey: view.run?.jobKey ?? null
		})
	);
	const hrefOf = (r: (typeof rows)[number]) => (r.mode === 'selected' ? rerunHref([runId], 'selected', r.nodes) : rerunHref([runId], r.mode));
</script>

{#if failures.length || detail?.rerunFromFailureNodes.length}
	<span title={blocked ?? undefined}>
		<DropdownMenu.Root>
			<DropdownMenu.Trigger class={buttonVariants({ variant: succeeded ? 'ghost' : 'accent', size: 'sm' })} disabled={!!blocked} data-testid="rerun-menu"><RotateCcwIcon />Rerun <ChevronDownIcon /></DropdownMenu.Trigger>
			<DropdownMenu.Content class="w-80 p-1.5" align="end">
				{#each rows as r, i (r.key)}
					{#if r.mode === 'all' && i > 0}<DropdownMenu.Separator />{/if}
					<DropdownMenu.Item class="items-start gap-3 px-2.5 py-2" onSelect={() => goto(to(hrefOf(r)))}>
						<span class="flex min-w-0 flex-1 flex-col gap-0.5">
							<span class="text-[13px] font-medium">{r.label}</span>
							<small class="text-muted-foreground truncate text-[11.5px]" title={r.description}>{r.description}</small>
						</span>
						<span class="text-muted-foreground shrink-0 pt-px text-[12px] tabular-nums">{r.count}</span>
					</DropdownMenu.Item>
				{/each}
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	</span>
{:else if succeeded && view.nodes.length}
	<!-- Nothing failed: one choice, the whole run, so no menu; quiet like Details. -->
	<Button variant="ghost" size="sm" href={blocked ? undefined : to(rerunHref([runId], 'all'))} disabled={!!blocked} title={blocked ?? 'Open a rerun of the whole run'} data-testid="rerun-menu"><RotateCcwIcon />Rerun</Button>
{/if}
