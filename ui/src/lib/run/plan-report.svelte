<script lang="ts">
	// A plan in the order and words of `sqb plan`: what runs first, each change with its detail and diff,
	// then the routine models.
	import { to } from '$lib/paths';
	import type { PlanDocument, PlanWarning } from '$lib/api';
	import { formatAgo } from '$lib/format/time';
	import { app } from '$lib/state/app.svelte';
	import { changedRule, changedSections, looseWarnings, planSections, unknownWords, type PlanEntry } from './plan-text';

	/** `refresh` words what refreshes the nodes a `--changed` plan holds back for a job. */
	let { plan, refresh = (job: string) => `job ${job}` }: { plan: PlanDocument; refresh?: (job: string) => string } = $props();

	const sections = $derived([...planSections(plan), ...(plan.changed ? changedSections(plan.changed, refresh) : [])]);
	/** Rows a section shows before "Show all", as the CLI truncates long sections. */
	const limit = 50;
	let expanded = $state<Record<string, boolean>>({});
	const loose = $derived(looseWarnings(plan, sections));
	/** A warning shown under its model, without the model name the CLI opens it with. */
	const warningText = (w: PlanWarning) => {
		const text = w.model_name ? w.message : w.message.slice(w.message.indexOf(':') + 1).trim();
		return text.charAt(0).toUpperCase() + text.slice(1);
	};

	const href = (e: PlanEntry, title: string) => (/^\d+$/.test(e.name) || title.includes('unction') || title.startsWith('Sources') || title.startsWith('Loaders') ? null : to(`/catalog/model:${e.name}`));
	const diffClass = (line: string) => (line.startsWith('+++') || line.startsWith('---') || line.startsWith('@@') ? 'hunk' : line.startsWith('+') ? 'add' : line.startsWith('-') ? 'del' : '');
</script>

<div class="report">
	{#if plan.synced_copy}
		{@const copy = plan.synced_copy}
		<section class="preview">
			<p>
				A preview from warehouse metadata {#if copy.synced_at}synced <time datetime={copy.synced_at} title={copy.synced_at}>{formatAgo(copy.synced_at, app.now)}</time>{:else}that hasn’t fully synced yet{/if}. The run checks the warehouse again before it builds.
			</p>
			{#if copy.unknown_until_run.length}
				<details class="unknown">
					<summary>What the preview can’t know yet</summary>
					<ul>{#each copy.unknown_until_run as item (item)}<li>{unknownWords(item)}</li>{/each}</ul>
				</details>
			{/if}
		</section>
	{/if}
	{#if plan.changed}<p class="policy" data-testid="changed-policy">{changedRule(plan.changed)}</p>{/if}
	{#each sections as s (s.title)}
		{#if s.collapsed && !expanded[s.title]}
			<button class="collapsed" aria-expanded="false" onclick={() => (expanded[s.title] = true)}>
				<i></i>{s.entries.length} {s.title.toLowerCase()}, {s.entries[0].value} <span>Show</span>
			</button>
		{:else}
		{@const shown = expanded[s.title] ? s.entries : s.entries.slice(0, limit)}
		<section>
			<h4><i class:changed={s.changed}></i>{s.title} <span>({s.entries.length})</span></h4>
			{#each shown as e (`${e.name}:${e.value}`)}
				{@const link = href(e, s.title)}
				{@const hasDiff = !!(e.queryDiff?.length || e.configDiff?.length || e.schema?.length)}
				<div class="entry">
					<div class="line">
						{#if link}<a class="name" href={link}>{e.name}</a>{:else}<span class="name">{e.name}</span>{/if}
						<span class="value" class:accent={s.changed}>{e.value}</span>
					</div>
					{#each e.detail as d, i (i)}<div class="detail">{d}</div>{/each}
					{#each e.warnings ?? [] as w, i (i)}<p class="entry-warning" class:err={w.severity === 'error'}>{warningText(w)}</p>{/each}
					{#if hasDiff}
						{#if e.schema?.length}
							<div class="detail">schema diff:</div>
							<pre class="diff">{#each e.schema as f (f.text)}<span class={f.sign === '+' ? 'add' : f.sign === '-' ? 'del' : 'hunk'}>{f.sign} {f.text}</span>{'\n'}{/each}</pre>
						{/if}
						{#if e.configDiff?.length}
							<div class="detail">config diff:</div>
							<pre class="diff">{#each e.configDiff as l, i (i)}<span class={diffClass(l)}>{l}</span>{'\n'}{/each}</pre>
						{/if}
						{#if e.queryDiff?.length}
							<div class="detail">query diff:</div>
							<pre class="diff">{#each e.queryDiff as l, i (i)}<span class={diffClass(l)}>{l}</span>{'\n'}{/each}</pre>
						{/if}
					{/if}
				</div>
			{/each}
			{#if s.entries.length > shown.length}
				<button class="more" onclick={() => (expanded[s.title] = true)}>… and {s.entries.length - shown.length} more</button>
			{/if}
		</section>
		{/if}
	{/each}
	{#if loose.length}
		<section>
			<h4><i class="warn"></i>Warnings <span>({loose.length})</span></h4>
			{#each loose as w, i (i)}
				<div class="entry warning" class:err={w.severity === 'error'}>
					{#if w.model_name}<span class="name">{w.model_name}</span>{/if}
					<span>{#if w.code}<code>{w.code}</code> {/if}{w.message}</span>
				</div>
			{/each}
		</section>
	{/if}
</div>

<style>
	.report {
		display: flex;
		flex-direction: column;
		gap: 18px;
		font-size: 12.5px;
	}
	.policy {
		margin: 0;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.collapsed {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 4px 0;
		border-bottom: 1px solid var(--border);
		color: var(--muted-foreground);
		font-size: 12.5px;
		text-align: left;
	}
	.collapsed span {
		color: var(--primary);
		font-size: 11.5px;
	}
	.collapsed:hover span {
		text-decoration: underline;
	}
	.entry-warning {
		margin: 4px 0 2px 16px;
		padding: 4px 8px;
		border-left: 2px solid var(--sb-warning);
		background: color-mix(in srgb, var(--sb-warning) 8%, transparent);
		color: var(--foreground);
		font-size: 12px;
		line-height: 17px;
	}
	.entry-warning.err {
		border-left-color: var(--sb-error);
	}
	h4 {
		display: flex;
		align-items: center;
		gap: 6px;
		margin: 0 0 4px;
		padding-bottom: 4px;
		border-bottom: 1px solid var(--border);
		color: var(--foreground);
		font-size: 12.5px;
		font-weight: 500;
	}
	h4 span {
		color: var(--sb-text-faint);
		font-weight: 400;
	}
	i {
		display: inline-block;
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--sb-text-faint);
	}
	i.changed {
		background: var(--sb-warning);
	}
	i.warn {
		background: var(--sb-warning);
	}
	.entry {
		padding: 4px 0;
		border-bottom: 1px solid var(--border-subtle);
	}
	.line {
		display: flex;
		align-items: baseline;
		gap: 12px;
	}
	.name {
		min-width: 0;
		overflow: hidden;
		color: var(--foreground);
		font-family: var(--font-mono);
		font-size: 11.5px;
		text-overflow: ellipsis;
		white-space: nowrap;
		flex: 0 1 auto;
		width: 280px;
	}
	a.name:hover {
		color: var(--primary);
	}
	.value {
		color: var(--muted-foreground);
		white-space: nowrap;
	}
	.value.accent {
		color: var(--foreground);
	}
	.detail {
		padding-left: 16px;
		color: var(--sb-text-faint);
		font-family: var(--font-mono);
		font-size: 11px;
		line-height: 17px;
	}
	.diff {
		margin: 2px 0 6px 16px;
		padding: 6px 10px;
		overflow-x: auto;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--sb-inset);
		color: var(--muted-foreground);
		font-family: var(--font-mono);
		font-size: 11px;
		line-height: 16px;
	}
	.diff .add {
		color: var(--sb-success);
	}
	.diff .del {
		color: var(--sb-error);
	}
	.diff .hunk {
		color: var(--sb-text-faint);
	}
	.more {
		color: var(--primary);
		font-size: 11.5px;
		align-self: flex-start;
		padding-top: 4px;
	}
	.warning {
		display: flex;
		gap: 12px;
		padding-left: 10px;
		border-left: 2px solid var(--sb-warning);
	}
	.warning.err {
		border-left-color: var(--sb-error);
	}
	.preview {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 8px 10px;
		border: 1px solid var(--border);
		border-left: 2px solid var(--primary);
		border-radius: 2px;
		background: var(--sb-inset);
		color: var(--muted-foreground);
	}
	.preview p {
		margin: 0;
	}
	.preview time {
		color: var(--foreground);
	}
	.unknown summary {
		color: var(--sb-text-faint);
		font-size: 11.5px;
		cursor: pointer;
	}
	.unknown ul {
		margin: 4px 0 0;
		padding-left: 16px;
		list-style: disc;
		font-size: 12px;
	}
	code {
		color: var(--sb-text-faint);
		font-family: var(--font-mono);
		font-size: 11px;
	}
</style>
