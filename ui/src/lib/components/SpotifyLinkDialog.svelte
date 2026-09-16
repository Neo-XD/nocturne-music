<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { browser } from '$app/environment';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Alert02Icon,
		InformationCircleIcon,
		HelpCircleIcon,
		ViewIcon,
		ViewOffIcon,
		LinkSquare01Icon
	} from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import { refreshSpotify, toast } from '$lib/player.svelte';

	let { open = $bindable(false) }: { open: boolean } = $props();

	type AuthTab = 'dev' | 'cookie';
	let activeTab = $state<AuthTab>('dev');

	// Developer API State
	let clientId = $state('');
	let clientSecret = $state('');
	let showSecretSection = $state(false);
	let showSecret = $state(false);
	let startingAuth = $state(false);
	let waitingForAuth = $state(false);
	let authUrl = $state('');
	let manualCodeOrUrl = $state('');
	let submittingManual = $state(false);
	let showManualInput = $state(false);

	// Cookie State
	let spDc = $state('');
	let connectingCookie = $state(false);

	let unlistenLinked: (() => void) | null = null;

	onMount(() => {
		if (browser) {
			clientId = localStorage.getItem('nocturne:spotify_client_id') || '';
			clientSecret = localStorage.getItem('nocturne:spotify_client_secret') || '';
			if (clientSecret) {
				showSecretSection = true;
			}
		}

		void api.onSpotifyLinked((status) => {
			if (!open) return;
			void refreshSpotify();
			toast.success(
				status.display_name
					? `Linked Spotify account as ${status.display_name}`
					: 'Spotify account linked successfully!'
			);
			waitingForAuth = false;
			open = false;
		}).then((un) => {
			unlistenLinked = un;
		});
	});

	onDestroy(() => {
		if (unlistenLinked) unlistenLinked();
	});

	async function handleStartDevAuth(e: Event) {
		e.preventDefault();
		const cid = clientId.trim();
		const csec = clientSecret.trim();
		if (!cid) {
			toast.error('Please enter your Spotify Client ID');
			return;
		}

		if (browser) {
			localStorage.setItem('nocturne:spotify_client_id', cid);
			if (csec) {
				localStorage.setItem('nocturne:spotify_client_secret', csec);
			} else {
				localStorage.removeItem('nocturne:spotify_client_secret');
			}
		}

		startingAuth = true;
		try {
			const url = await api.spotifyStartDevAuth(cid, csec || undefined);
			authUrl = url;
			waitingForAuth = true;
			toast.info('Opened Spotify in browser. Click Agree to link!');
		} catch (err) {
			toast.error(String(err));
		} finally {
			startingAuth = false;
		}
	}

	async function handleManualSubmit() {
		const cid = clientId.trim();
		const csec = clientSecret.trim();
		const input = manualCodeOrUrl.trim();
		if (!cid) {
			toast.error('Please enter your Spotify Client ID');
			return;
		}
		if (!input) {
			toast.error('Please paste the authorization code or redirected URL');
			return;
		}

		submittingManual = true;
		try {
			const res = await api.spotifyCompleteDevAuth(cid, csec || undefined, input);
			await refreshSpotify();
			toast.success(
				res.display_name
					? `Linked Spotify account as ${res.display_name}`
					: 'Spotify account linked successfully!'
			);
			waitingForAuth = false;
			open = false;
		} catch (err) {
			toast.error(String(err));
		} finally {
			submittingManual = false;
		}
	}

	async function submitCookie(e: Event) {
		e.preventDefault();
		const cookie = spDc.trim();
		if (!cookie) {
			toast.error('Please enter your sp_dc cookie');
			return;
		}
		connectingCookie = true;
		try {
			const res = await api.spotifyLink(cookie);
			await refreshSpotify();
			toast.success(
				res.display_name
					? `Linked Spotify account as ${res.display_name}`
					: 'Spotify account linked successfully!'
			);
			spDc = '';
			open = false;
		} catch (err) {
			toast.error(String(err));
		} finally {
			connectingCookie = false;
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-lg">
		<Dialog.Header>
			<div class="flex items-center gap-2.5">
				<div class="flex size-7 items-center justify-center rounded-full bg-emerald-500/20 text-emerald-400">
					<svg class="size-4" viewBox="0 0 24 24" fill="currentColor">
						<path d="M12 2C6.477 2 2 6.477 2 12c0 5.524 4.477 10 10 10 5.524 0 10-4.476 10-10 0-5.523-4.476-10-10-10zm4.586 14.424a.627.627 0 0 1-.86.208c-2.355-1.439-5.32-1.765-8.812-.966a.625.625 0 0 1-.277-1.22c3.824-.874 7.099-.508 9.74 1.107.292.179.387.568.209.871zm1.226-2.723a.784.784 0 0 1-1.077.26c-2.695-1.656-6.804-2.136-9.992-1.168a.785.785 0 1 1-.462-1.501c3.642-1.106 8.188-.574 11.27 1.321a.784.784 0 0 1 .261 1.088zm.105-2.833c-3.232-1.919-8.566-2.096-11.657-1.157a.94.94 0 1 1-.552-1.8c3.553-1.078 9.444-.87 13.14 1.323a.94.94 0 0 1-.931 1.634z"/>
					</svg>
				</div>
				<Dialog.Title>Link Spotify Account</Dialog.Title>
			</div>
			<Dialog.Description>
				Connect your Spotify account to view playlists, sync libraries, and transfer tracks.
			</Dialog.Description>
		</Dialog.Header>

		<!-- Mode Tabs -->
		<div class="flex items-center p-0.5 rounded-lg bg-muted/60 text-xs mt-1">
			<button
				type="button"
				class="flex-1 py-1.5 rounded-md font-medium text-center transition-all cursor-pointer {activeTab === 'dev'
					? 'bg-background shadow-xs text-foreground font-semibold'
					: 'text-muted-foreground hover:text-foreground'}"
				onclick={() => {
					activeTab = 'dev';
					waitingForAuth = false;
				}}
			>
				Spotify Developer API <span class="text-[10px] text-emerald-400 font-normal ml-1">Standard (Single Key)</span>
			</button>
			<button
				type="button"
				class="flex-1 py-1.5 rounded-md font-medium text-center transition-all cursor-pointer {activeTab === 'cookie'
					? 'bg-background shadow-xs text-foreground font-semibold'
					: 'text-muted-foreground hover:text-foreground'}"
				onclick={() => {
					activeTab = 'cookie';
					waitingForAuth = false;
				}}
			>
				Cookie (sp_dc) <span class="text-[10px] text-amber-400 font-normal ml-1">Discontinued</span>
			</button>
		</div>

		<!-- Spotify Premium Warning Banner -->
		<div class="rounded-xl border border-amber-500/40 bg-amber-500/10 p-3 text-xs text-amber-200 flex items-start gap-2.5">
			<HugeiconsIcon icon={Alert02Icon} class="h-4 w-4 shrink-0 text-amber-400 mt-0.5" />
			<div class="space-y-0.5">
				<p class="font-semibold text-amber-300">Spotify Premium Notice</p>
				<p class="text-[11px] text-amber-200/85 leading-relaxed">
					Direct Web Player audio streaming and Spotify Connect device control are exclusive to <strong>Spotify Premium</strong> accounts. Free accounts can still import and sync playlists.
				</p>
			</div>
		</div>

		{#if activeTab === 'dev'}
			<!-- DEVELOPER API FLOW -->
			{#if !waitingForAuth}
				<form class="space-y-3.5" onsubmit={handleStartDevAuth}>
					<!-- Instructions Card -->
					<div class="rounded-lg border border-border/50 bg-muted/30 p-3 text-[11px] text-muted-foreground space-y-1.5">
						<div class="flex items-center gap-1.5 font-semibold text-foreground text-xs">
							<HugeiconsIcon icon={InformationCircleIcon} class="h-4 w-4 text-emerald-400" />
							<span>How to get your free Spotify Client ID:</span>
						</div>
						<ol class="list-decimal pl-4 space-y-1.5 leading-relaxed">
							<li>
								Open the <button
									type="button"
									class="text-primary hover:underline inline-flex items-center gap-0.5 font-medium cursor-pointer p-0 bg-transparent border-none text-[11px]"
									onclick={() => api.openExternal('https://developer.spotify.com/dashboard')}
								>
									<span>Spotify Developer Dashboard</span>
									<HugeiconsIcon icon={LinkSquare01Icon} class="h-2.5 w-2.5 inline" />
								</button> and log in.
							</li>
							<li>Click <strong>Create app</strong> (name: <code>Nocturne</code>, description: <code>Desktop player</code>).</li>
							<li>In <strong>Redirect URIs</strong>, enter <code class="px-1 py-0.2 bg-muted rounded font-mono text-[10px] text-emerald-400 select-all">http://127.0.0.1:8888/callback</code> and click <strong>Add</strong>.</li>
							<li>
								Under <strong>Which API/SDKs are you planning to use?</strong>: check <strong>Web API</strong> (and optionally <strong>Web Playback SDK</strong>). Leave iOS and Android unchecked.
							</li>
							<li>Agree to Spotify Developer Terms and click <strong>Save</strong>.</li>
							<li>In your app settings, copy your <strong>Client ID</strong> (the only key provided for desktop apps) and paste it below.</li>
						</ol>
					</div>

					<div class="space-y-1.5">
						<div class="flex items-center justify-between">
							<label for="dev-client-id" class="text-xs font-semibold text-foreground">Spotify Client ID</label>
							<span class="text-[10px] text-emerald-400">Single key required</span>
						</div>
						<Input
							id="dev-client-id"
							type="text"
							bind:value={clientId}
							placeholder="Paste your Spotify App Client ID (e.g. 7c32...)"
							class="font-mono text-xs"
							required
						/>
					</div>

					<!-- Optional Client Secret for Legacy Confidential Apps -->
					<div class="pt-0.5">
						<button
							type="button"
							class="text-[11px] text-muted-foreground hover:text-foreground inline-flex items-center gap-1 cursor-pointer transition-colors"
							onclick={() => (showSecretSection = !showSecretSection)}
						>
							<span>{showSecretSection ? '− Hide optional Client Secret' : '+ Advanced: Have a confidential app with a Client Secret? (Optional)'}</span>
						</button>

						{#if showSecretSection}
							<div class="space-y-1.5 mt-2 pt-2 border-t border-border/40">
								<div class="flex items-center justify-between">
									<label for="dev-client-secret" class="text-xs font-semibold text-muted-foreground">Client Secret (Optional)</label>
									<button
										type="button"
										class="text-[11px] text-muted-foreground hover:text-foreground inline-flex items-center gap-1 cursor-pointer"
										onclick={() => (showSecret = !showSecret)}
									>
										<HugeiconsIcon icon={showSecret ? ViewOffIcon : ViewIcon} class="h-3 w-3" />
										<span>{showSecret ? 'Hide' : 'Show'}</span>
									</button>
								</div>
								<Input
									id="dev-client-secret"
									type={showSecret ? 'text' : 'password'}
									bind:value={clientSecret}
									placeholder="Leave empty unless you created a confidential app"
									class="font-mono text-xs"
								/>
							</div>
						{/if}
					</div>

					<div class="flex items-center justify-between pt-1">
						<Button type="button" variant="outline" onclick={() => (open = false)}>
							Cancel
						</Button>
						<Button
							type="submit"
							disabled={startingAuth || !clientId.trim()}
							class="gap-1.5 bg-emerald-600 hover:bg-emerald-500 text-white font-medium cursor-pointer"
						>
							{#if startingAuth}
								<div class="h-3.5 w-3.5 animate-spin rounded-full border-2 border-white border-t-transparent"></div>
								<span>Opening Browser…</span>
							{:else}
								<span>Authorize with Spotify</span>
							{/if}
						</Button>
					</div>
				</form>
			{:else}
				<!-- WAITING FOR BROWSER AUTH CALLBACK -->
				<div class="space-y-4 py-2">
					<div class="rounded-xl border border-emerald-500/30 bg-emerald-500/10 p-4 text-center space-y-2">
						<div class="flex justify-center">
							<div class="relative flex items-center justify-center">
								<span class="animate-ping absolute inline-flex h-8 w-8 rounded-full bg-emerald-400 opacity-60"></span>
								<div class="relative inline-flex rounded-full h-8 w-8 bg-emerald-500 text-white items-center justify-center">
									<svg class="size-4 animate-spin" viewBox="0 0 24 24" fill="none" stroke="currentColor">
										<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
										<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
									</svg>
								</div>
							</div>
						</div>
						<h4 class="font-semibold text-sm text-foreground">Waiting for Spotify Authorization</h4>
						<p class="text-xs text-muted-foreground leading-relaxed max-w-sm mx-auto">
							Log in and click <strong>Agree</strong> on the opened Spotify web page. Nocturne is listening on <code class="text-emerald-400 font-mono">127.0.0.1:8888</code> to automatically complete linking.
						</p>
						{#if authUrl}
							<div class="pt-1">
								<button
									type="button"
									class="text-xs text-primary underline hover:text-primary/80 cursor-pointer"
									onclick={() => api.openExternal(authUrl)}
								>
									Click here to reopen Spotify in browser
								</button>
							</div>
						{/if}
					</div>

					<!-- Manual Callback Code Fallback -->
					<div class="rounded-lg border border-border/60 p-3 space-y-2 text-xs">
						<button
							type="button"
							class="flex items-center justify-between w-full text-left font-medium text-foreground hover:text-primary transition-colors cursor-pointer"
							onclick={() => (showManualInput = !showManualInput)}
						>
							<span class="flex items-center gap-1.5">
								<HugeiconsIcon icon={HelpCircleIcon} class="h-3.5 w-3.5 text-muted-foreground" />
								<span>Firewall blocking or redirect didn't close?</span>
							</span>
							<span class="text-[10px] text-muted-foreground">{showManualInput ? 'Hide' : 'Manual Code'}</span>
						</button>

						{#if showManualInput}
							<div class="space-y-2 pt-1">
								<p class="text-[11px] text-muted-foreground">
									If your browser redirected to <code>http://127.0.0.1:8888/callback?code=...</code> but didn't link automatically, paste the full URL or the <code>code</code> here:
								</p>
								<div class="flex gap-1.5">
									<Input
										type="text"
										bind:value={manualCodeOrUrl}
										placeholder="Paste redirect URL or authorization code..."
										class="font-mono text-xs flex-1"
									/>
									<Button
										type="button"
										size="sm"
										disabled={submittingManual || !manualCodeOrUrl.trim()}
										onclick={handleManualSubmit}
										class="bg-emerald-600 hover:bg-emerald-500 text-white cursor-pointer shrink-0 text-xs"
									>
										{submittingManual ? 'Verifying…' : 'Submit'}
									</Button>
								</div>
							</div>
						{/if}
					</div>

					<div class="flex justify-end pt-1">
						<Button
							type="button"
							variant="outline"
							onclick={() => {
								waitingForAuth = false;
								manualCodeOrUrl = '';
							}}
						>
							Back to credentials
						</Button>
					</div>
				</div>
			{/if}
		{:else}
			<!-- COOKIE (SP_DC) FLOW -->
			<div class="space-y-4 py-1">
				<div class="rounded-xl border border-destructive/40 bg-destructive/10 p-3.5 text-xs text-red-200 flex items-start gap-2.5">
					<HugeiconsIcon icon={Alert02Icon} class="h-4 w-4 shrink-0 text-red-400 mt-0.5" />
					<div class="space-y-1.5">
						<p class="font-semibold text-red-300">Cookie Auth Discontinued by Spotify</p>
						<p class="text-[11px] text-red-200/90 leading-relaxed">
							Spotify has permanently blocked third-party web token endpoints (<code class="font-mono text-red-300">HTTP 403 URL Blocked</code>). Legacy <code class="font-mono text-red-300">sp_dc</code> cookie scraping is no longer supported by Spotify servers.
						</p>
						<p class="text-[11px] text-red-200/80 leading-relaxed">
							Please use the <strong>Spotify Developer API</strong> tab instead. It only requires a free <strong>Client ID</strong> (no secret needed) and connects seamlessly via Spotify's modern PKCE authorization.
						</p>
						<div class="pt-1">
							<Button
								type="button"
								size="sm"
								class="bg-emerald-600 hover:bg-emerald-500 text-white cursor-pointer text-xs h-7 px-3 font-medium"
								onclick={() => {
									activeTab = 'dev';
									waitingForAuth = false;
								}}
							>
								Switch to Spotify Developer API (Recommended)
							</Button>
						</div>
					</div>
				</div>

				<form class="space-y-3 opacity-60 pointer-events-none" onsubmit={submitCookie}>
					<div class="space-y-1.5">
						<label for="sp-dc-input" class="text-xs font-semibold text-foreground">Legacy sp_dc Cookie</label>
						<Input
							id="sp-dc-input"
							type="password"
							bind:value={spDc}
							disabled
							placeholder="Legacy cookie login is discontinued..."
							class="font-mono text-xs"
						/>
					</div>

					<div class="flex justify-end gap-2 pt-1">
						<Button type="button" variant="outline" onclick={() => (open = false)}>
							Close
						</Button>
					</div>
				</form>
			</div>
		{/if}
	</Dialog.Content>
</Dialog.Root>
