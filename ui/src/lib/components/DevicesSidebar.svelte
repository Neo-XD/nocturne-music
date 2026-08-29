<script lang="ts">
	import { fade, fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { onMount, onDestroy } from 'svelte';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Cancel01Icon,
		ComputerIcon,
		SmartPhone01Icon,
		CheckmarkCircle02Icon,
		RefreshIcon
	} from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import * as api from '$lib/api';
	import type { RemoteSyncInfo } from '$lib/api';

	let {
		onClose
	}: {
		onClose: () => void;
	} = $props();

	let syncInfo = $state<RemoteSyncInfo | null>(null);
	let loading = $state(true);
	let pollTimer: any;

	async function fetchStatus() {
		try {
			syncInfo = await api.getRemoteSyncStatus();
		} catch (e) {
			console.error('Failed to get remote sync status', e);
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		fetchStatus();
		pollTimer = setInterval(fetchStatus, 2000);
	});

	onDestroy(() => {
		if (pollTimer) clearInterval(pollTimer);
	});
</script>

<aside
	class="absolute right-0 top-0 bottom-0 z-30 flex w-80 max-w-[90vw] flex-col border-l bg-background/95 backdrop-blur-xl shadow-2xl"
	transition:fly={{ x: 320, duration: 250, easing: cubicOut }}
>
	<!-- Header -->
	<div class="flex items-center justify-between border-b px-4 py-3.5">
		<div class="flex items-center gap-2">
			<HugeiconsIcon icon={ComputerIcon} class="h-5 w-5 text-primary" />
			<h2 class="text-sm font-semibold">Select Playback Device</h2>
		</div>
		<Button variant="ghost" size="icon-sm" onclick={onClose} aria-label="Close">
			<HugeiconsIcon icon={Cancel01Icon} class="h-4 w-4" />
		</Button>
	</div>

	<!-- Scrollable content -->
	<div class="flex-1 space-y-5 overflow-y-auto p-4">
		<!-- Current Device Section -->
		<div>
			<div class="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
				Current Output Device
			</div>
			<div
				class="flex items-center justify-between rounded-xl border border-primary/30 bg-primary/10 p-3.5 cursor-default"
			>
				<div class="flex items-center gap-3">
					<div class="flex h-10 w-10 items-center justify-center rounded-lg bg-primary text-primary-foreground">
						<HugeiconsIcon icon={ComputerIcon} class="h-5 w-5" />
					</div>
					<div>
						<div class="text-sm font-semibold text-foreground">
							{syncInfo?.device_name || 'This Computer'}
						</div>
						<div class="flex items-center gap-1.5 text-xs text-primary font-medium">
							<span class="relative flex h-2 w-2">
								<span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-primary opacity-75"></span>
								<span class="relative inline-flex h-2 w-2 rounded-full bg-primary"></span>
							</span>
							Active Playback Device
						</div>
					</div>
				</div>
			</div>
		</div>

		<!-- Connected Mobile Devices Section -->
		<div>
			<div class="mb-2 flex items-center justify-between">
				<span class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
					Connected Mobile Devices
				</span>
				<Button variant="ghost" size="icon-sm" onclick={fetchStatus} aria-label="Refresh" class="h-6 w-6">
					<HugeiconsIcon icon={RefreshIcon} class="h-3.5 w-3.5 text-muted-foreground" />
				</Button>
			</div>

			{#if syncInfo && syncInfo.connected_clients.length > 0}
				<div class="space-y-2">
					{#each syncInfo.connected_clients as client (client.id)}
						<div
							class="flex items-center justify-between rounded-xl border bg-muted/40 p-3.5 transition-colors hover:bg-muted/70"
						>
							<div class="flex items-center gap-3">
								<div class="flex h-9 w-9 items-center justify-center rounded-lg bg-secondary text-foreground">
									<HugeiconsIcon icon={SmartPhone01Icon} class="h-4 w-4" />
								</div>
								<div>
									<div class="text-sm font-medium text-foreground">{client.name}</div>
									<div class="text-xs text-muted-foreground">IP: {client.ip}</div>
								</div>
							</div>
							<span class="inline-flex items-center gap-1 rounded-full bg-primary/20 px-2.5 py-0.5 text-[11px] font-semibold text-primary">
								<HugeiconsIcon icon={CheckmarkCircle02Icon} class="h-3 w-3" />
								Connected
							</span>
						</div>
					{/each}
				</div>
			{:else}
				<div class="rounded-xl border border-dashed p-4 text-center">
					<div class="mx-auto mb-1.5 flex h-8 w-8 items-center justify-center rounded-full bg-muted text-muted-foreground">
						<HugeiconsIcon icon={SmartPhone01Icon} class="h-4 w-4" />
					</div>
					<div class="text-xs font-medium text-foreground">No mobile device connected</div>
					<div class="mt-0.5 text-[11px] text-muted-foreground">
						Open Nocturne on your phone to automatically connect over Wi-Fi
					</div>
				</div>
			{/if}
		</div>
	</div>
</aside>
