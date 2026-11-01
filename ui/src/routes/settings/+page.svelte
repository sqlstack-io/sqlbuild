<script lang="ts">
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import AccessChecker from '$lib/components/admin/access-checker.svelte';
	import PermissionName from '$lib/components/admin/permission-name.svelte';
	import KeyValues from '$lib/components/platform/key-values.svelte';
	import Panel from '$lib/components/platform/panel.svelte';
	import type { AuthMode } from '$lib/api';
	import { PERMISSIONS_BY_RISK, RISK_LABEL } from '$lib/platform/permission-catalog';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';

	const settings = createResource((client) => client.serverSettings());
	const s = $derived(settings.data ?? null);

	const authLabel: Record<AuthMode, string> = {
		oidc: 'Single sign-on (OIDC)',
		trusted_proxy: 'Company proxy sign-in',
		password: 'Password',
		disabled: 'Off: only reachable from this machine'
	};
</script>

<AppTopbar title="Settings" />

<div class="min-h-0 flex-1 overflow-auto px-6 pt-5">
	<div class="columns">
		<div>
			<Panel title="Server" note="Read-only; set in the server config and at start-up">
				{#if settings.error}
					<p class="err">Couldn't load: {settings.error}</p>
				{:else if s}
					<KeyValues
						rows={[
							{ label: 'Version', value: s.serverVersion },
							{ label: 'Storage', value: s.storage === 'postgresql' ? 'PostgreSQL' : 'SQLite' },
							{
								label: 'Database',
								value: s.schemaVersion === s.latestSchemaVersion ? `Up to date (schema ${s.schemaVersion}), updated ${formatAgo(s.schemaMigratedAt, app.now)}` : `Needs upgrading: schema ${s.schemaVersion} of ${s.latestSchemaVersion}`,
								title: formatStamp(s.schemaMigratedAt),
								tone: s.schemaVersion === s.latestSchemaVersion ? undefined : 'var(--sb-warning)'
							},
							{ label: 'Authentication', value: authLabel[s.authMode] },
							{
								label: 'Scheduler',
								value: s.schedulerEnabled ? 'Runs on this server' : 'Not on this server (started with --no-scheduler)',
								tone: s.schedulerEnabled ? undefined : 'var(--sb-warning)'
							}
						]}
					/>
				{:else}
					<p class="muted">Loading…</p>
				{/if}
			</Panel>

			<Panel title="How long things are kept">
				<table class="sb-list static framed">
					<tbody>
						{#each s?.retention ?? [] as r (r.name)}
							<tr>
								<td>
									{r.name}
									<div class="cell-desc">{r.description}</div>
								</td>
								<td class="num">{r.value}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</Panel>
		</div>

		<div>
			<Panel title="Your access" note="What your roles allow; the server checks every action">
				{#if app.user}
					<KeyValues
						rows={[
							{ label: 'Signed in as', value: `${app.user.displayName} (${app.user.name})` },
							{ label: 'Role', value: app.user.role }
						]}
					/>
					<table class="caps">
						<tbody>
							{#each PERMISSIONS_BY_RISK as group (group.risk)}
								{#each group.actions as c, i (c)}
									<tr class:off={!app.can(c)}>
										<td class="risk">{i === 0 ? RISK_LABEL[group.risk] : ''}</td>
										<td class="mark" title={app.can(c) ? 'You can' : 'You can’t'}>{app.can(c) ? '✓' : '—'}</td>
										<td><PermissionName action={c} /></td>
									</tr>
								{/each}
							{/each}
						</tbody>
					</table>
					<p class="cli">On the command line, sign in with <code>sqb login</code>: it opens this browser and keeps a short-lived session, so you don't need an API token.</p>
				{/if}
			</Panel>
			<Panel title="Why can or can't I?" note="Paste a denial ID from an error, or pick an action">
				<AccessChecker sampleDecision="dec-5r2w9j" />
			</Panel>
		</div>
	</div>
</div>

<style>
	.cli {
		margin-top: 12px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.cli code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.columns {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
		gap: 0 40px;
		max-width: 1280px;
	}
	.err {
		color: var(--sb-error);
		font-size: 12.5px;
	}
	.muted {
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.framed {
		border-top: 1px solid var(--border);
	}
	:global(.sb-list.static tbody tr) {
		cursor: default;
	}
	.caps {
		width: 100%;
		margin-top: 12px;
		border-collapse: collapse;
		font-size: 12.5px;
	}
	.caps td {
		padding: 3px 8px 3px 0;
		vertical-align: baseline;
	}
	.caps .risk {
		width: 100px;
		color: var(--sb-text-faint);
		font-size: 11.5px;
	}
	.caps .mark {
		width: 18px;
		color: var(--sb-success);
	}
	.caps tr.off td {
		color: var(--sb-text-faint);
	}
</style>
