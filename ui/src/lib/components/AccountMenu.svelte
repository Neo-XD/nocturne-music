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
		Delete02Icon,
		Settings01Icon
	} from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import * as api from '$lib/api';
	import type { SavedAccount } from '$lib/api';
	import {
		auth,
		openChannelPicker,
		toast,
		ui,
		spotify,
		refreshSpotify,
		setSpotifySyncMode,
		prefs,
		setAudioStreamSource,
		setListeningHistoryTarget
	} from '$lib/player.svelte';
	import { thumb } from '$lib/thumb';
	import { anchorMenu, fitMenu, NO_ANCHOR, toBody } from '$lib/menu';
	import SpotifyLinkDialog from './SpotifyLinkDialog.svelte';
	import SpotifyTransferDialog from './SpotifyTransferDialog.svelte';

	let menuOpen = $state(false);
	let anchor = $state(NO_ANCHOR);
	let savedAccounts = $state<SavedAccount[]>([]);
	let loadingAccounts = $state(false);
	let spotifyModalOpen = $state(false);
	let transferModalOpen = $state(false);

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

	async function unlinkSpotify() {
		try {
			await api.spotifyUnlink();
			await refreshSpotify();
			toast.success('Spotify account unlinked');
		} catch (e) {
			toast.error(String(e));
		}
	}

	// Right-anchored under the trigger, like the Last.fm menu next to it.
	function openMenu(e: MouseEvent) {
		anchor = anchorMenu(e, { align: 'right' });
		menuOpen = !menuOpen;
		if (menuOpen) {
			loadAccounts();
			refreshSpotify();
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
		{@attach toBody}
	></button>
	<div
		class="fixed z-50 w-80 animate-in rounded-xl border border-border/80 bg-popover/90 p-4 text-popover-foreground shadow-2xl backdrop-blur-2xl duration-150 fade-in-0 zoom-in-95"
		style={anchor.style}
		{@attach toBody}
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

		<div class="my-2 h-px bg-border/60"></div>

		<!-- Spotify Account Section -->
		<div class="mb-2">
			<div class="mb-1.5 flex items-center justify-between">
				<span class="text-[11px] font-semibold uppercase tracking-wider text-emerald-500">Spotify Account</span>
				{#if spotify.status.linked}
					<span class="rounded bg-emerald-500/15 px-1.5 py-0.2 text-[9px] font-semibold text-emerald-400">
						{spotify.status.product ? spotify.status.product.toUpperCase() : 'LINKED'}
					</span>
				{/if}
			</div>

			{#if spotify.status.linked}
				<div class="rounded-lg border border-border/60 bg-muted/40 p-2.5 text-left mb-2">
					<div class="flex items-center gap-2.5">
						{#if spotify.status.avatar_url}
							<img
								src={spotify.status.avatar_url}
								alt=""
								class="size-8 shrink-0 rounded-full object-cover ring-1 ring-emerald-500/50"
							/>
						{:else}
							<div class="flex size-8 shrink-0 items-center justify-center rounded-full bg-emerald-500/20 text-emerald-400">
								<svg class="size-4" viewBox="0 0 24 24" fill="currentColor">
									<path d="M12 2C6.477 2 2 6.477 2 12c0 5.524 4.477 10 10 10 5.524 0 10-4.476 10-10 0-5.523-4.476-10-10-10zm4.586 14.424a.627.627 0 0 1-.86.208c-2.355-1.439-5.32-1.765-8.812-.966a.625.625 0 0 1-.277-1.22c3.824-.874 7.099-.508 9.74 1.107.292.179.387.568.209.871zm1.226-2.723a.784.784 0 0 1-1.077.26c-2.695-1.656-6.804-2.136-9.992-1.168a.785.785 0 1 1-.462-1.501c3.642-1.106 8.188-.574 11.27 1.321a.784.784 0 0 1 .261 1.088zm.105-2.833c-3.232-1.919-8.566-2.096-11.657-1.157a.94.94 0 1 1-.552-1.8c3.553-1.078 9.444-.87 13.14 1.323a.94.94 0 0 1-.931 1.634z"/>
								</svg>
							</div>
						{/if}
						<div class="min-w-0 flex-1">
							<div class="truncate text-xs font-semibold">{spotify.status.display_name || spotify.status.username || 'Spotify User'}</div>
							<p class="truncate text-[10px] text-muted-foreground">Linked via sp_dc</p>
						</div>
					</div>

					<!-- Playlist Mode Selector: Separate (Default), In Sync, Transfer -->
					<div class="mt-2.5 border-t border-border/40 pt-2">
						<div class="flex items-center justify-between mb-1">
							<span class="text-[10px] font-medium text-muted-foreground">Playlist Mode:</span>
							<span class="text-[10px] font-bold text-foreground capitalize">{spotify.syncMode}</span>
						</div>
						<div class="grid grid-cols-3 gap-1 rounded-md bg-muted/60 p-0.5">
							<button
								type="button"
								class="rounded py-1 text-[10px] font-semibold transition cursor-pointer {spotify.syncMode === 'seperate' ? 'bg-background text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground'}"
								onclick={() => setSpotifySyncMode('seperate')}
								title="Keep YTM and Spotify playlists in separate tabs"
							>
								Separate
							</button>
							<button
								type="button"
								class="rounded py-1 text-[10px] font-semibold transition cursor-pointer {spotify.syncMode === 'sync' ? 'bg-background text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground'}"
								onclick={() => setSpotifySyncMode('sync')}
								title="Keep playlists synchronized between services"
							>
								In Sync
							</button>
							<button
								type="button"
								class="rounded py-1 text-[10px] font-semibold transition cursor-pointer {spotify.syncMode === 'transfer' ? 'bg-background text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground'}"
								onclick={() => setSpotifySyncMode('transfer')}
								title="Transfer playlists between YTM and Spotify"
							>
								Transfer
							</button>
						</div>

						{#if spotify.syncMode === 'transfer'}
							<button
								type="button"
								class="mt-1.5 flex w-full items-center justify-center gap-1 rounded bg-muted/80 hover:bg-muted py-1 text-[10px] font-medium text-foreground transition cursor-pointer"
								onclick={() => {
									menuOpen = false;
									transferModalOpen = true;
								}}
							>
								Transfer Playlists…
							</button>
						{/if}
					</div>

					{#if auth.account?.signedIn && spotify.status.linked}
						<!-- Audio Stream Source & Listening History Target (Dual-Logged in only) -->
						<div class="mt-2.5 border-t border-border/40 pt-2">
							<div class="flex items-center justify-between mb-1">
								<span class="text-[10px] font-medium text-muted-foreground">Audio Stream Source:</span>
								<span class="text-[10px] font-bold text-foreground uppercase">{prefs.audioStreamSource === 'spotify' ? 'Spotify' : 'YouTube Music'}</span>
							</div>
							<div class="grid grid-cols-2 gap-1 rounded-md bg-muted/60 p-0.5 mb-2">
								<button
									type="button"
									class="rounded py-1 text-[10px] font-semibold transition cursor-pointer {prefs.audioStreamSource === 'ytm' ? 'bg-background text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground'}"
									onclick={() => setAudioStreamSource('ytm')}
									title="Stream audio from YouTube Music"
								>
									YouTube Music
								</button>
								<button
									type="button"
									class="rounded py-1 text-[10px] font-semibold transition cursor-pointer {prefs.audioStreamSource === 'spotify' ? 'bg-emerald-500/20 text-emerald-400 font-bold shadow-xs' : 'text-muted-foreground hover:text-foreground'}"
									onclick={() => setAudioStreamSource('spotify')}
									title="Stream audio from Spotify"
								>
									Spotify
								</button>
							</div>

							<div class="flex items-center justify-between mb-1">
								<span class="text-[10px] font-medium text-muted-foreground">Listening History:</span>
								<span class="text-[10px] font-bold text-foreground">
									{prefs.listeningHistoryTarget === 'spotify' ? 'Spot Only' : prefs.listeningHistoryTarget === 'ytm' ? 'YTM Only' : 'Both'}
								</span>
							</div>
							<div class="grid grid-cols-3 gap-1 rounded-md bg-muted/60 p-0.5">
								<button
									type="button"
									class="rounded py-1 text-[10px] font-semibold transition cursor-pointer {prefs.listeningHistoryTarget === 'ytm' ? 'bg-background text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground'}"
									onclick={() => setListeningHistoryTarget('ytm')}
									title="Save listening history to YouTube Music only"
								>
									YTM Only
								</button>
								<button
									type="button"
									class="rounded py-1 text-[10px] font-semibold transition cursor-pointer {prefs.listeningHistoryTarget === 'spotify' ? 'bg-background text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground'}"
									onclick={() => setListeningHistoryTarget('spotify')}
									title="Save listening history to Spotify only"
								>
									Spot Only
								</button>
								<button
									type="button"
									class="rounded py-1 text-[10px] font-semibold transition cursor-pointer {prefs.listeningHistoryTarget === 'both' ? 'bg-background text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground'}"
									onclick={() => setListeningHistoryTarget('both')}
									title="Save listening history to both services"
								>
									Both
								</button>
							</div>
						</div>
					{/if}

					<Button
						variant="ghost"
						size="sm"
						class="mt-2 h-7 w-full text-xs text-destructive hover:bg-destructive/10 hover:text-destructive cursor-pointer"
						onclick={unlinkSpotify}
					>
						Unlink Spotify
					</Button>
				</div>
			{:else}
				<Button
					variant="outline"
					size="sm"
					class="w-full gap-2 border-emerald-500/40 text-emerald-400 hover:bg-emerald-500/10 hover:text-emerald-300 cursor-pointer"
					onclick={() => {
						menuOpen = false;
						spotifyModalOpen = true;
					}}
				>
					<svg class="h-4 w-4" viewBox="0 0 24 24" fill="currentColor">
						<path d="M12 2C6.477 2 2 6.477 2 12c0 5.524 4.477 10 10 10 5.524 0 10-4.476 10-10 0-5.523-4.476-10-10-10zm4.586 14.424a.627.627 0 0 1-.86.208c-2.355-1.439-5.32-1.765-8.812-.966a.625.625 0 0 1-.277-1.22c3.824-.874 7.099-.508 9.74 1.107.292.179.387.568.209.871zm1.226-2.723a.784.784 0 0 1-1.077.26c-2.695-1.656-6.804-2.136-9.992-1.168a.785.785 0 1 1-.462-1.501c3.642-1.106 8.188-.574 11.27 1.321a.784.784 0 0 1 .261 1.088zm.105-2.833c-3.232-1.919-8.566-2.096-11.657-1.157a.94.94 0 1 1-.552-1.8c3.553-1.078 9.444-.87 13.14 1.323a.94.94 0 0 1-.931 1.634z"/>
					</svg>
					<span>Link Spotify Account</span>
				</Button>
			{/if}
		</div>

		<div class="my-2 h-px bg-border/60"></div>
		<Button
			variant="ghost"
			size="sm"
			class="w-full justify-start gap-2 text-foreground/80 hover:text-foreground cursor-pointer"
			onclick={() => {
				menuOpen = false;
				ui.settingsOpen = true;
			}}
		>
			<HugeiconsIcon icon={Settings01Icon} class="h-4 w-4" />
			<span>Settings</span>
			<span class="ml-auto text-[10px] text-muted-foreground">Ctrl+;</span>
		</Button>
	</div>
{/if}

<SpotifyLinkDialog bind:open={spotifyModalOpen} />
<SpotifyTransferDialog bind:open={transferModalOpen} />
