<script lang="ts">
	import { page } from '$app/state';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import CodePreview from '$lib/components/code/code-preview.svelte';
	import NoAccess from '$lib/components/platform/no-access.svelte';
	import Panel from '$lib/components/platform/panel.svelte';
	import PermissionName from '$lib/components/admin/permission-name.svelte';
	import RoleSourceBadge from '$lib/components/admin/role-source-badge.svelte';
	import SourceChip from '$lib/components/admin/source-chip.svelte';
	import { CAPABILITIES, type Capability } from '$lib/api';
	import { builtInLabel, personHref, roleHref, roleSummary, scopeWords } from '$lib/platform/access';
	import { PERMISSIONS } from '$lib/platform/permission-catalog';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { plural } from '../utils';

	const name = $derived(page.params.name ?? '');
	const role = createResource((client, scope) => (app.can('role.assign') && name ? client.getRole(scope.locationId, name) : Promise.resolve(null)));
	const r = $derived(role.data ?? null);
	const locations = $derived(app.bootstrap?.locations ?? []);

	let compare = $state(false);
	let open = $state<Record<string, boolean>>({});

	/** Each addition as one action on one selector. */
	const additions = $derived(r && r.source !== 'built_in' ? r.grants.flatMap((g) => g.nodes.flatMap((selector) => g.permissions.map((action) => ({ action, selector })))) : []);
	const baseActions = $derived(new Set(r?.baseGrants.flatMap((g) => g.permissions) ?? []));
	const ownActions = $derived(r?.source === 'built_in' ? (r.grants.flatMap((g) => g.permissions) as Capability[]) : []);
	const matchOf = (selector: string) => r?.selectors.find((s) => s.selector === selector);

	function addedOn(action: Capability): string[] {
		return additions.filter((a) => a.action === action).map((a) => (a.selector === '*' ? 'every node' : a.selector));
	}
</script>

<AppTopbar title="Roles">
	<Button variant="outline" size="sm" onclick={() => role.refresh()}>↻ Refresh</Button>
</AppTopbar>

{#if !app.can('role.assign')}
	<NoAccess what="roles" requires="role.assign" />
{:else}
	<div class="flex min-h-0 flex-1 flex-col">
		<div class="head">
			<a href="/admin/roles" class="back">← Roles</a>
			<span class="title">{name}</span>
			{#if r}
				<RoleSourceBadge role={r} />
				<span class="muted">{roleSummary(r)}</span>
			{/if}
		</div>

		{#if role.data === null && !role.loading}
			<p class="muted px-6 py-4">No role named <code>{name}</code> in {app.location?.name}. Project roles only exist in the locations deployed from that project.</p>
		{:else if r}
			<div class="min-h-0 flex-1 overflow-auto px-6 pt-5 pb-10">
				<div class="columns">
					<div class="min-w-0">
						<p class="desc">{r.description}</p>

						{#if r.source === 'built_in'}
							<Panel title="What it allows" note="On every node, in the locations it's given">
								<ul class="actions">
									{#each ownActions as a (a)}<li><PermissionName action={a} risk /></li>{/each}
								</ul>
							</Panel>
						{:else}
							<Panel title="Base role">
								{#snippet actions()}
									{#if r.extends}<label class="toggle"><input type="checkbox" class="sb-check" bind:checked={compare} /> Compare with base</label>{/if}
								{/snippet}
								{#if r.extends}
									<p>
										Starts from <a class="link" href={roleHref(r.extends)}>{builtInLabel(r.extends)}</a>: {plural(baseActions.size, 'action')} on every node, then adds the actions below. Roles don't
										inherit further than one built-in role.
									</p>
								{:else}
									<p class="muted">No built-in base.</p>
								{/if}
								{#if compare}
									<table class="compare">
										<thead>
											<tr><th>Action</th><th>{r.extends ? builtInLabel(r.extends) : 'Base'}</th><th>{r.name}</th></tr>
										</thead>
										<tbody>
											{#each CAPABILITIES as c (c)}
												{@const inBase = baseActions.has(c)}
												{@const added = addedOn(c)}
												{#if inBase || added.length}
													<tr class:diff={!inBase}>
														<td><PermissionName action={c} risk /></td>
														<td>{inBase ? '✓ everywhere' : '—'}</td>
														<td>{inBase ? '✓ everywhere' : `✓ ${added.join(', ')}`}</td>
													</tr>
												{/if}
											{/each}
										</tbody>
									</table>
								{/if}
							</Panel>

							<Panel title="Additions" note="Approved actions on top of the base role">
								<table class="sb-list static framed">
									<thead>
										<tr>
											<th>Action</th>
											<th>On</th>
											<th>Matches today</th>
										</tr>
									</thead>
									<tbody>
										{#each additions as a (a.action + a.selector)}
											{@const m = matchOf(a.selector)}
											<tr class:risky={PERMISSIONS[a.action].risk === 'changes_data' || PERMISSIONS[a.action].risk === 'administer'}>
												<td>
													<PermissionName action={a.action} risk />
													<div class="cell-desc">{PERMISSIONS[a.action].description}</div>
												</td>
												<td><code>{a.selector === '*' ? 'every node' : a.selector}</code></td>
												<td>
													{#if a.selector === '*'}
														<span class="muted">Every node</span>
													{:else if m}
														{plural(m.nodes.length, 'node')}{#if m.newSinceLastWeek.length}<span class="growth">, +{m.newSinceLastWeek.length} since last week</span>{/if}
													{/if}
												</td>
											</tr>
										{:else}
											<tr><td colspan="3" class="empty">No additions.</td></tr>
										{/each}
									</tbody>
								</table>
							</Panel>

							{#if r.selectors.length}
								<Panel title="What the selectors match today" note="Selectors can match more nodes as the project grows, so watch the growth">
									<ul class="selectors">
										{#each r.selectors as s (s.selector)}
											<li>
												<button class="sel" onclick={() => (open = { ...open, [s.selector]: !open[s.selector] })} aria-expanded={!!open[s.selector]}>
													<ChevronRightIcon size={13} class={open[s.selector] ? 'rotate-90' : ''} />
													<code>{s.selector}</code> → {plural(s.nodes.length, 'node')}
													{#if s.newSinceLastWeek.length}<span class="growth">+{s.newSinceLastWeek.length} since last week</span>{/if}
												</button>
												{#if open[s.selector]}
													<div class="nodes">
														{#each s.nodes as n (n)}
															<a href="/catalog/{n}" class:new={s.newSinceLastWeek.includes(n)} title={s.newSinceLastWeek.includes(n) ? 'Started matching in the last week' : undefined}>{n}</a>
														{:else}
															<span class="muted">Matches nothing today.</span>
														{/each}
													</div>
												{/if}
											</li>
										{/each}
									</ul>
								</Panel>
							{/if}
						{/if}

						{#if r.definition}
							<Panel title="Definition" note="As the location published it; change it in the project's roles file">
								<CodePreview file={r.definedIn ?? 'published roles document'} code={r.definition} language="text" maxLines={24} />
							</Panel>
						{/if}
					</div>

					<div class="min-w-0">
						<Panel title="Who has it" note={plural(r.holders, 'account')}>
							<ul class="holders">
								{#each r.assignments as a (a.id)}
									<li class:stale={a.stale}>
										<a href={personHref(a.principal)}>{a.principal.displayName}</a>
										<span class="muted">{scopeWords(a.scope.locationId, locations)}</span>
										<SourceChip assignment={a} />
									</li>
								{:else}
									<li class="muted">Nobody has this role yet.</li>
								{/each}
							</ul>
						</Panel>
					</div>
				</div>
			</div>
		{:else if role.error}
			<p class="err px-6 py-4">Couldn't load: {role.error}</p>
		{:else}
			<p class="muted px-6 py-4">Loading…</p>
		{/if}
	</div>
{/if}

<style>
	.head {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 10px;
		padding: 14px 24px;
		border-bottom: 1px solid var(--border);
	}
	.back {
		color: var(--muted-foreground);
		font-size: 13px;
	}
	.back:hover {
		color: var(--foreground);
	}
	.title {
		font-family: var(--font-display);
		font-size: 17px;
		font-weight: 600;
	}
	.columns {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 340px;
		gap: 0 36px;
		max-width: 1400px;
		font-size: 12.5px;
	}
	.desc {
		margin-bottom: 18px;
		font-size: 13px;
	}
	.actions {
		display: flex;
		flex-direction: column;
		gap: 5px;
	}
	.toggle {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.compare {
		width: 100%;
		margin-top: 10px;
		border-collapse: collapse;
	}
	.compare th,
	.compare td {
		padding: 4px 10px 4px 0;
		border-bottom: 1px solid var(--border);
		text-align: left;
	}
	.compare th {
		color: var(--muted-foreground);
		font-size: 12px;
		font-weight: 500;
	}
	.compare tr.diff td:last-child {
		color: var(--sb-success);
	}
	.framed {
		border-top: 1px solid var(--border);
	}
	:global(.sb-list.static tbody tr) {
		cursor: default;
	}
	tr.risky td:first-child {
		box-shadow: inset 2px 0 0 var(--sb-warning);
	}
	.growth {
		color: var(--sb-warning);
	}
	.selectors {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.sel {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}
	.sel :global(svg) {
		color: var(--sb-text-faint);
		transition: transform 0.1s;
	}
	.nodes {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 10px;
		padding: 6px 0 4px 20px;
	}
	.nodes a {
		color: var(--primary);
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.nodes a.new::after {
		content: ' new';
		color: var(--sb-warning);
		font-family: var(--font-sans);
		font-size: 11px;
	}
	.holders {
		display: flex;
		flex-direction: column;
		gap: 7px;
	}
	.holders li {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
	}
	.holders a,
	.link {
		color: var(--primary);
	}
	li.stale a {
		text-decoration: line-through;
	}
	.muted {
		color: var(--muted-foreground);
	}
	.err {
		color: var(--sb-error);
		font-size: 12.5px;
	}
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
</style>
