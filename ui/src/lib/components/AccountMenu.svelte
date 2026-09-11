<script lang="ts">
	// Account control for the titlebar (context/15) — moved out of the sidebar so sign-in lives in the
	// top bar. Supports multiple signed-in Google accounts and 1-click switching.
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		UserCircleIcon,
		Logout01Icon,
		ArrowDown01Icon,
		Add01Icon,
		Tick02Icon,
		Delete02Icon
	} from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import * as api from '$lib/api';
	import type { SavedAccount } from '$lib/api';
	import { auth, openChannelPicker, toast } from '$lib/player.svelte';
	import { thumb } from '$lib/thumb';
	import { anchorMenu, fitMenu, NO_ANCHOR } from '$lib/menu';

	let menuOpen = $state(false);
	let anchor = $state(NO_ANCHOR);
	let savedAccounts = $state<SavedAccount[]>([]);
	let loadingAccounts = $state(false);

	async function loadAccounts() {
		try {
			loadingAccounts = true;
			savedAccounts = await api.getSavedAccounts();
		} catch {
			savedAccounts = [];
		} finally {
			loadingAccounts = false;
		}
	}

	// Right-anchored under the trigger, like the Last.fm menu next to it.
	function openMenu(e: MouseEvent) {
		anchor = anchorMenu(e, { align: 'right' });
		menuOpen = !menuOpen;
		if (menuOpen) {
			loadAccounts();
		}
	}

	// Sign-in/out state arrives via the `auth-changed` event (player.svelte.ts), which also reloads
	// the library and remounts the page — nothing to assign here.
	async function doSignOut() {
		menuOpen = false;
		await api.signOut();
	}

	function signInGoogle() {
		api.loginWebview(); // native sign-in window takes over; result arrives via auth-changed
		menuOpen = false;
	}

	function switchChannel() {
		menuOpen = false;
		openChannelPicker();
	}

	async function switchAccount(id: string) {
		try {
			menuOpen = false;
			toast.info('Switching account...');
			await api.switchSavedAccount(id);
			toast.success('Switched account');
		} catch (e) {
			toast.error(`Could not switch account: ${e}`);
		}
	}

	async function removeAccount(e: MouseEvent, id: string) {
		e.stopPropagation();
		try {
			await api.removeSavedAccount(id);
			savedAccounts = savedAccounts.filter((a) => a.id !== id);
			toast.info('Account removed');
		} catch (e) {
			toast.error(`Could not remove account: ${e}`);
		}
	}
</script>

<button
	onclick={openMenu}
	title={auth.account?.signedIn ? (auth.account.name ?? 'Account') : 'Sign in'}
	aria-expanded={menuOpen}
	class="flex h-full cursor-pointer items-center gap-2 px-2.5 text-xs transition-colors hover:bg-muted aria-expanded:bg-muted"
>
	{#if auth.account?.signedIn && auth.account.thumbnail}
		<!-- max-width:none defeats Tailwind Preflight's `img{max-width:100%}`, which in a tight box
		     clamps width to the content-box while height stays fixed → a vertical oval. Inline so it's
		     immune to Preflight and to stale dev CSS. -->
		<img
			src={thumb(auth.account.thumbnail, 64)}
			alt=""
			style="width:1.25rem;height:1.25rem;max-width:none"
			class="shrink-0 rounded-full object-cover ring-1 ring-border"
		/>
	{:else}
		<HugeiconsIcon icon={UserCircleIcon} class="h-5 w-5 shrink-0 text-muted-foreground" />
	{/if}
	<span class="hidden max-w-28 truncate font-medium lg:block">
		{auth.account?.signedIn ? (auth.account.name ?? 'Account') : 'Sign in'}
	</span>
	<HugeiconsIcon
		icon={ArrowDown01Icon}
		class="hidden h-3.5 w-3.5 shrink-0 text-muted-foreground transition-transform duration-200 lg:block {menuOpen
			? 'rotate-180'
			: ''}"
	/>
</button>

{#if menuOpen}
	<button
		class="fixed inset-0 z-40 cursor-default"
		onclick={() => (menuOpen = false)}
		aria-label="Close menu"
	></button>
	<div
		class="fixed z-50 w-80 animate-in rounded-xl border border-border/80 bg-popover/90 p-4 text-popover-foreground shadow-2xl backdrop-blur-2xl duration-150 fade-in-0 zoom-in-95"
		style={anchor.style}
		{@attach fitMenu(anchor)}
	>
		{#if savedAccounts.length > 0}
			<div class="mb-2 flex items-center justify-between">
				<span class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Google Accounts</span>
				<span class="text-[10px] text-muted-foreground/80">{savedAccounts.length} saved</span>
			</div>
			<div class="mb-3 flex max-h-56 flex-col gap-1.5 overflow-y-auto pr-1">
				{#each savedAccounts as acc (acc.id)}
					<div
						role="button"
						tabindex="0"
						class="group flex cursor-pointer items-center justify-between rounded-lg border p-2 text-left transition {acc.isActive
							? 'border-primary/50 bg-primary/10'
							: 'border-border/60 hover:bg-muted/70'}"
						onclick={() => {
							if (!acc.isActive) switchAccount(acc.id);
						}}
						onkeydown={(e) => {
							if (e.key === 'Enter' && !acc.isActive) switchAccount(acc.id);
						}}
					>
						<div class="flex min-w-0 items-center gap-2.5">
							{#if acc.thumbnail}
								<img
									src={thumb(acc.thumbnail, 48)}
									alt=""
									class="size-8 shrink-0 rounded-full object-cover ring-1 ring-border"
								/>
							{:else}
								<div class="flex size-8 shrink-0 items-center justify-center rounded-full bg-muted text-muted-foreground">
									<HugeiconsIcon icon={UserCircleIcon} class="size-5" />
								</div>
							{/if}
							<div class="min-w-0">
								<div class="flex items-center gap-1.5">
									<span class="truncate text-xs font-semibold">{acc.name ?? 'Account'}</span>
									{#if acc.isActive}
										<span class="rounded bg-primary/20 px-1 py-0.2 text-[9px] font-medium text-primary">Active</span>
									{/if}
								</div>
								{#if acc.email || acc.handle}
									<p class="truncate text-[11px] text-muted-foreground">{acc.email ?? acc.handle}</p>
								{/if}
							</div>
						</div>
						<div class="flex items-center gap-1">
							{#if acc.isActive}
								<HugeiconsIcon icon={Tick02Icon} class="size-4 text-primary" />
							{/if}
							<button
								type="button"
								class="opacity-0 group-hover:opacity-100 p-1 text-muted-foreground hover:text-destructive transition rounded"
								title="Remove account"
								onclick={(e) => removeAccount(e, acc.id)}
							>
								<HugeiconsIcon icon={Delete02Icon} class="size-3.5" />
							</button>
						</div>
					</div>
				{/each}
			</div>

			<Button variant="outline" size="sm" class="mb-2 w-full gap-2" onclick={signInGoogle}>
				<HugeiconsIcon icon={Add01Icon} class="h-4 w-4" />
				Add another Google account
			</Button>

			{#if auth.account?.signedIn}
				<Button variant="outline" size="sm" class="mb-2 w-full gap-2" onclick={switchChannel}>
					<HugeiconsIcon icon={UserCircleIcon} class="h-4 w-4" />
					Switch channel
				</Button>
				<Button variant="outline" size="sm" class="w-full gap-2 text-destructive hover:text-destructive" onclick={doSignOut}>
					<HugeiconsIcon icon={Logout01Icon} class="h-4 w-4" />
					Sign out
				</Button>
			{/if}
		{:else if auth.account?.signedIn}
			<div class="mb-3">
				<div class="truncate text-sm font-medium">{auth.account.name ?? 'Account'}</div>
				{#if auth.account.handle || auth.account.email}
					<div class="truncate text-xs text-muted-foreground">
						{auth.account.handle ?? auth.account.email}
					</div>
				{/if}
			</div>
			<Button variant="outline" size="sm" class="mb-2 w-full gap-2" onclick={signInGoogle}>
				<HugeiconsIcon icon={Add01Icon} class="h-4 w-4" />
				Add another Google account
			</Button>
			<Button variant="outline" size="sm" class="mb-2 w-full gap-2" onclick={switchChannel}>
				<HugeiconsIcon icon={UserCircleIcon} class="h-4 w-4" />
				Switch channel
			</Button>
			<Button variant="outline" size="sm" class="w-full gap-2 text-destructive hover:text-destructive" onclick={doSignOut}>
				<HugeiconsIcon icon={Logout01Icon} class="h-4 w-4" />
				Sign out
			</Button>
		{:else}
			<p class="text-sm font-medium">Sign in</p>
			<p class="mt-1 text-xs text-muted-foreground">
				Sign in with your Google account to reach your YouTube Music library and playlists.
			</p>
			<Button class="mt-3 w-full" onclick={signInGoogle}>Sign in with Google</Button>
		{/if}
	</div>
{/if}
