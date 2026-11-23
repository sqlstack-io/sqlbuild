<script lang="ts">
	import BotIcon from '@lucide/svelte/icons/bot';
	import PageBody from '$lib/components/page-body.svelte';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import Loading from '$lib/components/loading.svelte';
	import { to } from '$lib/paths';
	import { page } from '$app/state';
	import DisclosureButton from '$lib/components/disclosure-button.svelte';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import CodePreview from '$lib/components/code/code-preview.svelte';
	import NoAccess from '$lib/components/platform/no-access.svelte';
	import Panel from '$lib/components/platform/panel.svelte';
	import PermissionName from '$lib/components/admin/permission-name.svelte';
	import RoleSourceBadge from '$lib/components/admin/role-source-badge.svelte';
	import SourceChip from '$lib/components/admin/source-chip.svelte';
	import { CAPABILITIES, type Capability } from '$lib/api';
	import { builtInLabel, principalHref, roleHref, roleSummary, scopeWords } from '$lib/platform/access';
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
	const additionColumns: TableColumn[] = [
		{ id: 'action', label: 'Action', minWidth: 220, truncate: false },
		{ id: 'on', label: 'On', width: '30%' },
		{ id: 'matches', label: 'Matches today', width: '200px' }
	];
</script>

<AppTopbar title="Roles" subject="{name} · Roles">
	<RefreshButton onclick={() => role.refresh()} />
</AppTopbar>

{#if !app.can('role.assign')}
	<NoAccess what="roles" requires="role.assign" />
{:else}
	<div class="flex min-h-0 flex-1 flex-col">
		<div class="head">
			<a href={to(`/admin/roles`)} class="back">← Roles</a>
			<span class="title">{name}</span>
			{#if r}
				<RoleSourceBadge role={r} />
				<span class="muted">{roleSummary(r)}</span>
			{/if}
		</div>

		{#if role.data === null && !role.loading}
			<p class="muted px-6 py-4">No role named <code>{name}</code> in {app.location?.name}. Project roles only exist in the locations deployed from that project.</p>
		{:else if r}
			<PageBody padding="20px 24px 40px">
				<div class="columns">
					<div class="min-w-0">
						<p class="desc">{r.description}</p>

						{#if r.source === 'built_in'}
							<Panel title="Permissions" help="On every node, in the locations it's given.">
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
										Starts from <a class="link" href={to(roleHref(r.extends))}>{builtInLabel(r.extends)}</a>: {plural(baseActions.size, 'action')} on every node, then adds the actions below. Roles don't
										inherit further than one built-in role.
									</p>
								{:else}
									<p class="muted">No built-in base.</p>
								{/if}
								{#if compare}
									<!-- raw-table: a two-column comparison of a role with its base role, not a list -->
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

							<Panel title="Additions" help="Approved actions on top of the base role.">
								<DataTable class="static framed" embedded columns={additionColumns} rows={additions} key={(a) => a.action + a.selector} empty="No additions.">
									{#snippet row(a)}
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
									{/snippet}
								</DataTable>
							</Panel>

							{#if r.selectors.length}
								<Panel title="Matched nodes" help="Selectors can match more nodes as the project grows, so watch the growth.">
									<ul class="selectors">
										{#each r.selectors as s (s.selector)}
											<li>
												<DisclosureButton tone="default" open={!!open[s.selector]} onclick={() => (open = { ...open, [s.selector]: !open[s.selector] })}>
													<code>{s.selector}</code> → {plural(s.nodes.length, 'node')}
													{#if s.newSinceLastWeek.length}<span class="growth">+{s.newSinceLastWeek.length} since last week</span>{/if}
												</DisclosureButton>
												{#if open[s.selector]}
													<div class="nodes">
														{#each s.nodes as n (n)}
															<a href={to(`/catalog/${n}`)} class:new={s.newSinceLastWeek.includes(n)} title={s.newSinceLastWeek.includes(n) ? 'Started matching in the last week' : undefined}>{n}</a>
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
							<Panel title="Definition" help="As the location published it; change it in the project's roles file.">
								<CodePreview file={r.definedIn ?? 'published roles document'} code={r.definition} language="text" maxLines={24} />
							</Panel>
						{/if}
					</div>

					<div class="min-w-0">
						<Panel title="Holders" note={plural(r.holders, 'account')}>
							<ul class="holders">
								{#each r.assignments as a (a.id)}
									<li class:stale={a.stale}>
										{#if a.principal.kind === 'service'}
											<a class="holder" href={to(principalHref(a.principal))} title="Service account"><BotIcon size={13} strokeWidth={1.75} />{a.principal.displayName}</a>
											<span class="kind">service account</span>
										{:else}
											<a href={to(principalHref(a.principal))}>{a.principal.displayName}</a>
										{/if}
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
			</PageBody>
		{:else if role.error}
			<p class="err px-6 py-4">Couldn't load: {role.error}</p>
		{:else}
			<div class="px-6 py-4"><Loading /></div>
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
	.holder {
		display: inline-flex;
		align-items: center;
		gap: 5px;
	}
	.kind {
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
</style>
