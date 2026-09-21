<script lang="ts">
	import { onMount } from 'svelte';
	import { listen } from '@tauri-apps/api/event';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Exchange01Icon, Cancel01Icon, Tick02Icon } from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import { lt } from '$lib/lt.svelte';
	import { ui, toast } from '$lib/player.svelte';

	interface IncomingRequest {
		username: string;
		userId: string;
	}

	let request = $state<IncomingRequest | null>(null);
	let open = $state(false);
	let busy = $state(false);

	onMount(() => {
		// Listen for Tauri incoming-lt-request events from deep links or single-instance
		const unlistenPromise = listen<IncomingRequest>('incoming-lt-request', (e) => {
			request = e.payload;
			open = true;
		});

		// Custom window event listener for in-app trigger
		function handleLocalTrigger(e: Event) {
			const detail = (e as CustomEvent<IncomingRequest>).detail;
			request = detail || { username: 'Friend', userId: `user_${Date.now()}` };
			open = true;
		}

		window.addEventListener('nocturne:incoming-lt-request', handleLocalTrigger);

		return () => {
			unlistenPromise.then((fn) => fn());
			window.removeEventListener('nocturne:incoming-lt-request', handleLocalTrigger);
		};
	});

	// Also watch pendingJoins when in host mode
	$effect(() => {
		if (lt.role === 'host' && lt.pendingJoins.length > 0 && !open) {
			const first = lt.pendingJoins[0];
			request = { userId: first.userId, username: first.username };
			open = true;
		}
	});

	async function handleAccept() {
		if (!request) return;
		busy = true;
		try {
			if (lt.role === 'none') {
				// No session active: Start a Listen Together session and accept invite
				const myName = localStorage.getItem('lt_name')?.trim() || 'Host';
				await api.ltCreateRoom(myName);
				if (request.userId) {
					await api.ltApproveJoin(request.userId).catch(() => {});
				}
				toast.success(`Started Listen Together session and accepted invite from ${request.username}!`);
				ui.ltOpen = true;
			} else if (lt.role === 'host') {
				if (request.userId) {
					await api.ltApproveJoin(request.userId);
				}
				toast.success(`Accepted ${request.username} into Listen Together!`);
				ui.ltOpen = true;
			}
			open = false;
			request = null;
		} catch (e) {
			toast.error(`Failed to accept request: ${String(e)}`);
		} finally {
			busy = false;
		}
	}

	async function handleDecline() {
		if (request && lt.role === 'host' && request.userId) {
			await api.ltRejectJoin(request.userId).catch(() => {});
		}
		toast.info('Declined join request');
		open = false;
		request = null;
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="w-[92vw] max-w-md overflow-hidden rounded-2xl border border-border/80 bg-card/95 backdrop-blur-xl shadow-2xl p-5 sm:p-6">
		<div class="flex items-start gap-3.5">
			<div class="flex size-10 shrink-0 items-center justify-center rounded-xl bg-primary/15 text-primary ring-1 ring-primary/25">
				<HugeiconsIcon icon={Exchange01Icon} size={20} />
			</div>
			<div class="min-w-0 flex-1">
				<Dialog.Title class="text-base font-bold text-foreground">
					Listen Together Request
				</Dialog.Title>
				<Dialog.Description class="mt-1 text-xs sm:text-sm text-muted-foreground leading-relaxed">
					<strong class="text-foreground font-semibold">{request?.username ?? 'A friend'}</strong>
					wants to join your Listen Together session.
				</Dialog.Description>
			</div>
		</div>

		{#if lt.role === 'none'}
			<div class="mt-4 rounded-xl border border-border/60 bg-muted/40 p-3 text-xs text-muted-foreground leading-relaxed break-words">
				You do not currently have an active Listen Together session running. Accepting will automatically start hosting a synchronized session with your current track.
			</div>
		{/if}

		<div class="mt-5 flex flex-row items-center justify-end gap-2.5">
			<Button
				variant="outline"
				class="h-9 px-4 text-xs font-medium cursor-pointer"
				onclick={handleDecline}
				disabled={busy}
			>
				<HugeiconsIcon icon={Cancel01Icon} size={14} class="mr-1.5" />
				Decline
			</Button>

			<Button
				class="h-9 px-4 text-xs font-semibold cursor-pointer bg-primary text-primary-foreground shadow-md hover:bg-primary/90"
				onclick={handleAccept}
				disabled={busy}
			>
				<HugeiconsIcon icon={Tick02Icon} size={14} class="mr-1.5" />
				Accept Invite
			</Button>
		</div>
	</Dialog.Content>
</Dialog.Root>
