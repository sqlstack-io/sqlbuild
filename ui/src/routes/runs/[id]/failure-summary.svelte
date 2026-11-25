<script lang="ts">
	import CircleXIcon from '@lucide/svelte/icons/circle-x';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import Button, { buttonVariants } from '$lib/components/ui/button/button.svelte';
	import CopyButton from '$lib/components/copy-button.svelte';
	import { to } from '$lib/paths';
	import { app } from '$lib/state/app.svelte';
	import { useCatalogNodes } from '$lib/state/catalog.svelte';
	import QueryPanel from './query-panel.svelte';
	import { errorLine, failureLine, relationsRead, runFailures, type RunFailure } from './utils';
	import type { RunDetailState } from './state.svelte';

	// What made the run fail, under its title: one line, opening to the first failure in full and the
	// rest one line each. Only failed checks and nodes lead; the worker's own end line never does.
	let { view }: { view: RunDetailState } = $props();

	const target = $derived(view.run?.target ?? '');
	const failures = $derived(runFailures(view.nodes, view.events, view.statements, target));
	const first = $derived(failures[0]);
	let open = $state(false);
	/** Failures whose query is shown in place. */
	let queryOpen = $state<Set<string>>(new Set());
	/** The rest open one at a time, in full. */
	let restOpen = $state<string | null>(null);

	const catalog = useCatalogNodes();
	/** Each failed check's query, read once it is opened, for the inputs it reads. */
	let sqlOf = $state<Record<string, string | null>>({});
	$effect(() => {
		const scope = app.scope;
		const f = open ? first : null;
		if (!scope || !f?.statementId || f.statementId in sqlOf || !app.can('view.sql')) return;
		const id = f.statementId;
		const runId = view.statements.find((s) => s.id === id)?.runId ?? view.run?.id ?? '';
		app.client.getStatementSql(scope, runId, id).then(
			(res) => (sqlOf = { ...sqlOf, [id]: res.sql }),
			() => (sqlOf = { ...sqlOf, [id]: null })
		);
	});
	/** The inputs a failure's query reads that have never been built in this target. */
	function unbuilt(f: RunFailure): string[] {
		const sql = f.statementId ? sqlOf[f.statementId] : null;
		if (!sql || !catalog.data) return [];
		const byName = new Map(catalog.data.map((n) => [n.key.toLowerCase(), n]));
		return relationsRead(sql).flatMap((name) => {
			const n = byName.get(name);
			return n && !n.lastBuild ? [n.key] : [];
		});
	}
	function toggleQuery(id: string) {
		const next = new Set(queryOpen);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		queryOpen = next;
	}
</script>

{#snippet full(f: RunFailure)}
	{@const inputs = unbuilt(f)}
	{@const st = f.statementId ? view.statements.find((s) => s.id === f.statementId) : undefined}
	<div class="full">
		<dl>
			{#if f.code}<dt>Code</dt><dd><code>{f.code}</code></dd>{/if}
			<dt>Message</dt><dd class="message">{errorLine(f.message)}</dd>
			{#if inputs.length}<dt>Not built in {target}</dt><dd>{#each inputs as key, i (key)}{#if i}{', '}{/if}<a class="mono" href={to(`/catalog/${key}`)}>{key}</a>{/each}</dd>{/if}
		</dl>
		<div class="actions">
			{#if st}<Button variant="outline" size="sm" aria-pressed={queryOpen.has(f.id)} onclick={() => toggleQuery(f.id)}>{queryOpen.has(f.id) ? 'Hide query' : 'Show query'}</Button>{/if}
			<Button variant="outline" size="sm" onclick={() => view.showInLog(f.logQuery)}>Show in log</Button>
			{#if f.checkId}<a class={buttonVariants({ variant: 'outline', size: 'sm' })} href={to(`/checks/${f.checkId}`)}>Open the check</a>{:else if f.kind === 'node' && f.nodeKey}<Button variant="outline" size="sm" onclick={() => view.openDrawer(f.nodeKey)}>Open the node</Button>{/if}
			<CopyButton value={f.message} label="Copy error" text="Copy error" showValue={false} />
		</div>
		{#if st && queryOpen.has(f.id)}<div class="query"><QueryPanel statement={st} onstatements={() => view.showStatement(st.id, st.nodeKey)} /></div>{/if}
	</div>
{/snippet}

{#if first}
	<section class="failures" aria-label="Failures" data-testid="failure-summary">
		<!-- raw-button: the run's failure line, opening the first failure in full -->
		<button class="row head" onclick={() => (open = !open)} aria-expanded={open} title={open ? 'Hide the error' : 'Show the error, its query and the log'}>
			<CircleXIcon class="x" size={14} aria-hidden="true" />
			<span class="line">{failureLine(first)}</span>
			{#if failures.length > 1}<span class="faint">+{failures.length - 1} more</span>{/if}
			<ChevronRightIcon class={open ? 'caret open' : 'caret'} size={14} aria-hidden="true" />
		</button>
		{#if open}
			{@render full(first)}
			{#if failures.length > 1}
				<ul>
					{#each failures.slice(1) as f (f.id)}
						<li>
							<!-- raw-button: a further failure's line, opening it in full -->
							<button class="row" onclick={() => (restOpen = restOpen === f.id ? null : f.id)} aria-expanded={restOpen === f.id}>
								<CircleXIcon class="x" size={13} aria-hidden="true" />
								<span class="line">{failureLine(f)}</span>
								<ChevronRightIcon class={restOpen === f.id ? 'caret open' : 'caret'} size={13} aria-hidden="true" />
							</button>
							{#if restOpen === f.id}{@render full(f)}{/if}
						</li>
					{/each}
				</ul>
			{/if}
		{/if}
	</section>
{/if}

<style>
	.failures {
		flex: 0 0 auto;
		border-bottom: 1px solid var(--border-subtle);
		background: color-mix(in srgb, var(--sb-error) 2.5%, var(--background));
		font-size: 12.5px;
	}
	.row {
		display: flex;
		width: 100%;
		min-width: 0;
		align-items: center;
		gap: 8px;
		padding: 6px 24px;
		text-align: left;
	}
	.row:hover {
		background: color-mix(in srgb, var(--sb-error) 6%, transparent);
	}
	.head .line {
		color: var(--sb-error);
		font-weight: 500;
	}
	.row :global(.x) {
		flex: none;
		color: var(--sb-error);
	}
	.row :global(.caret) {
		flex: none;
		color: var(--sb-text-faint);
		transition: transform 0.12s;
	}
	.row :global(.caret.open) {
		transform: rotate(90deg);
	}
	.line {
		overflow: hidden;
		min-width: 0;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	ul {
		padding-left: 18px;
	}
	.full {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 2px 24px 10px 46px;
	}
	dl {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: 3px 14px;
	}
	dt {
		color: var(--muted-foreground);
	}
	dd {
		min-width: 0;
	}
	.message {
		color: var(--sb-mono-err);
		font-family: var(--font-mono);
		font-size: 12px;
		overflow-wrap: anywhere;
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px;
	}
	.query {
		max-width: 980px;
	}
	.mono,
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.faint {
		flex: none;
		color: var(--muted-foreground);
	}
</style>
