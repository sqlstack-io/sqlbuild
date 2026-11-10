import { browser } from '$app/environment';
import { logout } from '$lib/api/http-client';
import { MockApiClient } from '$lib/api/mock/mock-client';
import { bootFailure, type BootFailure } from '$lib/boot-failure';
import { appPath, to } from '$lib/paths';
import { withPrefetch } from './prefetch';
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
	type WarehouseSync
} from '$lib/api';

const SCOPE_KEY = 'sb-scope';
/** How often the shell rereads live problems and warehouse sync; locations report every 30 seconds. */
const POLL_MS = 30_000;

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
	navCounts = $state<NavCounts | null>(null);
	platformEvents = $state<PlatformEvent[]>([]);
	warehouseSync = $state<WarehouseSync | null>(null);

	readonly scope = $derived<Scope | null>(this.locationId ? { locationId: this.locationId } : null);
	readonly location = $derived(this.bootstrap?.locations.find((l) => l.id === this.locationId) ?? null);
	/** The selected location's one target. */
	readonly target = $derived(this.location?.target ?? null);
	readonly user = $derived(this.bootstrap?.user ?? null);
	/** Server-wide actions plus those allowed somewhere in the selected location. */
	readonly capabilities = $derived([...(this.bootstrap?.capabilities ?? []), ...(this.location?.capabilities ?? [])]);

	#started = false;
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
		if (path === '/login') return;
		window.location.assign(to(`/login?next=${encodeURIComponent(path + window.location.search)}`));
	}

	/** Ends the session; only the server client has one. */
	readonly signOut: (() => Promise<void>) | null;

	can(capability: Capability): boolean {
		return hasCapability(this.capabilities, capability);
	}

	async start(): Promise<void> {
		if (this.#started) return;
		this.#started = true;
		setInterval(() => (this.now = Date.now()), 1000);
		this.client.changes.onConnectionState((state) => (this.connection = state));
		this.client.changes.subscribe((signal) => {
			if (channelMatches(signal.channel, ['platform_events'])) void this.#loadPlatformEvents();
			if (channelMatches(signal.channel, ['warehouse_sync'])) void this.#loadWarehouseSync();
			if (channelMatches(signal.channel, ['runs', 'triggers', 'checks'])) void this.#loadNavCounts();
		});
		try {
			this.bootstrap = await this.client.bootstrap();
		} catch (error) {
			// A rejected session is already on its way to the login page.
			if (error instanceof ApiError && error.status === 401) return;
			this.bootError = bootFailure(error);
			return;
		}
		this.#restoreScope();
		this.client.changes.follow(this.scope);
		void this.#loadPlatformEvents();
		// Problems and sync reports come from time passing and location reports, so no change notice announces them.
		setInterval(() => {
			void this.#loadPlatformEvents();
			void this.#loadWarehouseSync();
		}, POLL_MS);
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

	#loadScoped() {
		void this.#loadNavCounts();
		void this.#loadWarehouseSync();
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
