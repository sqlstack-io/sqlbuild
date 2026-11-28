import { throttled } from './throttle';
import { browser } from '$app/environment';
import { logout } from '$lib/api/http-client';
import { MockApiClient } from '$lib/api/mock/mock-client';
import { bootFailure, type BootFailure } from '$lib/boot-failure';
import { appPath, to } from '$lib/paths';
import { withPrefetch } from './prefetch';
import { health } from './health.svelte';
import {
	ApiError,
	channelMatches,
	createClient,
	hasCapability,
	type ApiClient,
	type Bootstrap,
	type Capability,
	type ConnectionState,
	type NavCounts,
	type PlatformEvent,
	type Scope,
	type Snapshot,
	type WarehouseSync
} from '$lib/api';

const SCOPE_KEY = 'sb-scope';
/** How often the shell rereads live problems and warehouse sync; locations report every 30 seconds. */
const POLL_MS = 30_000;
/** The least time between reloads of the sidebar's counts while changes stream in. */
const NAV_COUNTS_MS = 10_000;

/**
 * App-wide state: the bootstrap, the selected location, and the
 * shell's live data (navigation counts, platform events, warehouse sync).
 */
class AppState {
	client: ApiClient;
	bootstrap = $state<Bootstrap | null>(null);
	bootError = $state<BootFailure | null>(null);
	locationId = $state('');
	/** Wall clock, ticking every second, for ages and elapsed times. */
	now = $state(Date.now());
	connection = $state<ConnectionState>('connecting');
	/** When live updates last reached the page; while they flow, pages are current, so it only shows once they drop. */
	updatedAt = $state(Date.now());
	/** What the page shown reads again on Reconnect; set by the page's `RefreshButton`. */
	#pageRefresh: (() => void) | null = null;
	navCounts = $state<NavCounts | null>(null);
	platformEvents = $state<PlatformEvent[]>([]);
	warehouseSync = $state<WarehouseSync | null>(null);
	/** The selected location's newest deployed version: undefined until read, null before its first deploy. */
	snapshot = $state<Snapshot | null | undefined>(undefined);

	readonly scope = $derived<Scope | null>(this.locationId ? { locationId: this.locationId } : null);
	readonly location = $derived(this.bootstrap?.locations.find((l) => l.id === this.locationId) ?? null);
	/** The selected location's one target. */
	readonly target = $derived(this.location?.target ?? null);
	readonly user = $derived(this.bootstrap?.user ?? null);
	/** Server-wide actions plus those allowed somewhere in the selected location. */
	readonly capabilities = $derived([...(this.bootstrap?.capabilities ?? []), ...(this.location?.capabilities ?? [])]);

	/** The bootstrap failed because the server or its control database is unavailable; it is read again on recovery. */
	bootWaiting = $state(false);

	#started = false;
	#polling = false;
	#booting = false;
	#syncRefused = new Set<string>();

	constructor() {
		this.client = withPrefetch(createClient(new URLSearchParams(browser ? window.location.search : ''), () => this.#signIn()));
		this.signOut = this.client instanceof MockApiClient ? null : async () => {
			await logout();
			this.#signIn();
		};
	}

	/** Sends the browser to the login page, which returns here after sign-in. */
	#signIn() {
		if (!browser) return;
		const path = appPath(window.location.pathname);
		// Status is public: it shows without a session.
		if (path === '/login' || path === '/status') return;
		window.location.assign(to(`/login?next=${encodeURIComponent(path + window.location.search)}`));
	}

	/** Ends the session; only the server client has one. */
	readonly signOut: (() => Promise<void>) | null;

	/** The page's own reread, for Reconnect; null when the page leaves. */
	setPageRefresh(refresh: (() => void) | null): void {
		this.#pageRefresh = refresh;
	}

	/** Follow the change feed again now instead of waiting for its next retry, and read the page again, as it may be behind. */
	reconnect(): void {
		this.client.changes.follow(this.scope);
		this.#pageRefresh?.();
	}

	can(capability: Capability): boolean {
		return hasCapability(this.capabilities, capability);
	}

	async start(): Promise<void> {
		if (this.#started) return;
		this.#started = true;
		setInterval(() => (this.now = Date.now()), 1000);
		health.start(this.client);
		health.onRecovered(() => {
			if (!this.bootstrap && !this.#booting) void this.#boot();
			// The change feed backed off while the server couldn't answer; follow it again now.
			else if (this.bootstrap) this.client.changes.follow(this.scope);
		});
		this.client.changes.onConnectionState((state) => {
			if (this.connection === 'live' && state !== 'live') this.updatedAt = Date.now();
			this.connection = state;
		});
		this.client.changes.subscribe((signal) => {
			if (channelMatches(signal.channel, ['platform_events', 'warehouse_sync', 'location_health'])) void this.#loadPlatformEvents();
			if (channelMatches(signal.channel, ['warehouse_sync'])) void this.#loadWarehouseSync();
			if (channelMatches(signal.channel, ['snapshot'])) void this.#loadSnapshot();
			if (channelMatches(signal.channel, ['runs', 'triggers', 'checks'])) this.#navCounts.request();
		});
		await this.#boot();
	}

	/** Read who the person is and where they may look; while the server can't answer, the shell shows why and waits. */
	async #boot(): Promise<void> {
		this.#booting = true;
		try {
			this.bootstrap = await this.client.bootstrap();
		} catch (error) {
			this.#booting = false;
			// A rejected session is already on its way to the login page.
			if (error instanceof ApiError && error.status === 401) return;
			if (unavailable(error)) {
				this.bootWaiting = true;
				void health.refresh();
				// Health also reboots on recovery; this covers a refusal health never saw.
				setTimeout(() => {
					if (!this.bootstrap) void this.#boot();
				}, health.retrySeconds * 1000);
				return;
			}
			this.bootError = bootFailure(error);
			return;
		}
		this.#booting = false;
		this.bootWaiting = false;
		this.#restoreScope();
		this.client.changes.follow(this.scope);
		void this.#loadPlatformEvents();
		health.readDetails(this.can('user.manage'));
		if (!this.#polling) {
			this.#polling = true;
			// Problems and sync reports come from time passing and location reports, so no change notice announces them.
			setInterval(() => {
				void this.#loadPlatformEvents();
				void this.#loadWarehouseSync();
			}, POLL_MS);
		}
		this.#loadScoped();
	}

	selectLocation(locationId: string): void {
		const location = this.bootstrap?.locations.find((l) => l.id === locationId);
		if (!location) return;
		this.locationId = location.id;
		this.#scopeChanged();
	}

	refreshNavCounts(): void {
		void this.#loadNavCounts();
	}

	#scopeChanged() {
		if (browser) localStorage.setItem(SCOPE_KEY, JSON.stringify(this.scope));
		this.client.changes.follow(this.scope);
		this.navCounts = null;
		this.warehouseSync = null;
		this.snapshot = undefined;
		this.#loadScoped();
	}

	#restoreScope() {
		const locations = this.bootstrap?.locations ?? [];
		const search = browser ? new URLSearchParams(window.location.search) : new URLSearchParams();
		let saved: Partial<Scope> = {};
		try {
			saved = browser ? JSON.parse(localStorage.getItem(SCOPE_KEY) ?? '{}') ?? {} : {};
		} catch {
			saved = {};
		}
		const wantedLocation = search.get('location') ?? saved.locationId;
		const location =
			locations.find((l) => l.id === wantedLocation || l.name === wantedLocation) ?? locations[0];
		if (!location) return;
		this.locationId = location.id;
	}

	/**
	 * The sidebar's counts read whole listings (every check's result, every run), so a running run's
	 * stream of changes reloads them at most every `NAV_COUNTS_MS`, and once more after the last change.
	 */
	#navCounts = throttled(() => this.#loadNavCounts(), NAV_COUNTS_MS);

	#loadScoped() {
		void this.#loadNavCounts();
		void this.#loadWarehouseSync();
		void this.#loadSnapshot();
	}

	async #loadSnapshot() {
		if (!this.client.supports('getLatestSnapshot')) return;
		const scope = this.scope;
		if (!scope) return;
		try {
			const snapshot = await this.client.getLatestSnapshot(scope);
			if (this.scope === scope) this.snapshot = snapshot;
		} catch {
			// The version is decoration; a person who may not read it sees the bar without it.
		}
	}

	async #loadNavCounts() {
		if (!this.client.supports('navCounts')) return;
		const scope = this.scope;
		if (!scope) return;
		try {
			const counts = await this.client.navCounts(scope);
			if (this.scope === scope) this.navCounts = counts;
		} catch {
			// Counts are decoration; the pages report their own errors.
		}
	}

	async #loadWarehouseSync() {
		if (!this.client.supports('getWarehouseSync')) return;
		const scope = this.scope;
		if (!scope) return;
		// Each refusal is audited, so a target the person may not list is asked once per page load, not every poll.
		if (this.#syncRefused.has(scope.locationId)) {
			this.warehouseSync = null;
			return;
		}
		try {
			const sync = await this.client.getWarehouseSync(scope);
			if (this.scope === scope) this.warehouseSync = sync;
		} catch (error) {
			if (error instanceof ApiError && error.status === 403) this.#syncRefused.add(scope.locationId);
			if (this.scope === scope) this.warehouseSync = null;
		}
	}

	async #loadPlatformEvents() {
		if (!this.client.supports('listPlatformEvents')) return;
		try {
			this.platformEvents = await this.client.listPlatformEvents({ activeOnly: true });
		} catch {
			this.platformEvents = [];
		}
	}
}

export const app = new AppState();

/** The server answered that it can't serve, or didn't answer at all. */
function unavailable(error: unknown): boolean {
	return error instanceof ApiError ? error.status === 503 : true;
}
