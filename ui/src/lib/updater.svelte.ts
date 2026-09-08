// Auto-update via Tauri's updater plugin. Checks a signed latest.json on GitHub Releases; the
// startup check is silent unless an update exists, the Settings check always reports a result.
// Only self-updates the AppImage build on Linux (Tauri limitation) — .deb, .rpm and distro packages
// update through their package manager, so they get a download link instead. See `canInstall`.
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { getVersion } from '@tauri-apps/api/app';
import { toast } from './player.svelte';
import { canSelfUpdate, getSettings, openExternal, installAppUpdate } from './api';

const RELEASES_URL = 'https://github.com/Neo-XD/nocturne-music/releases/latest';

export const updateState = $state({
	available: null as { version: string } | null, // set when a newer version is waiting
	canInstall: true, // false on packaged Linux builds; always resolved before `available` is set
	checking: false, // Settings "Check for updates" is in flight
	installing: false // downloading/installing the update
});

// The resolved handle to download; kept out of reactive state (it's not serializable/renderable).
let pending: Update | null = null;

function isNewer(latest: string, current: string): boolean {
	const parse = (v: string) => v.replace(/^v/, '').split('.').map((p) => parseInt(p, 10) || 0);
	const [lMaj = 0, lMin = 0, lPatch = 0] = parse(latest);
	const [cMaj = 0, cMin = 0, cPatch = 0] = parse(current);
	if (lMaj !== cMaj) return lMaj > cMaj;
	if (lMin !== cMin) return lMin > cMin;
	return lPatch > cPatch;
}

async function look(): Promise<boolean> {
	try {
		const u = await check();
		if (u) {
			pending = u;
			// Before `available`, so the banner never renders with the wrong button for a frame. On the
			// (unlikely) IPC failure, fall back to the download link: it works everywhere, while
			// "Update now" on a packaged build does not.
			updateState.canInstall = await canSelfUpdate().catch(() => false);
			updateState.available = { version: u.version };
			return true;
		}
	} catch (e) {
		console.debug('Tauri update check:', e);
	}
	// Fallback to releases API when updater manifest is missing a platform entry
	try {
		const res = await fetch('https://api.github.com/repos/Neo-XD/nocturne-music/releases/latest', {
			headers: { Accept: 'application/vnd.github.v3+json' }
		});
		if (res.ok) {
			const data = await res.json();
			const tag = (data.tag_name || '').replace(/^v/, '');
			const current = await getVersion().catch(() => '');
			if (tag && current && isNewer(tag, current)) {
				updateState.canInstall = true;
				updateState.available = { version: tag };
				return true;
			}
		}
	} catch (e) {
		console.debug('Releases API fallback check:', e);
	}
	return false;
}

/** On app open: show the update banner if one exists, stay silent otherwise. With `update_banner`
 *  off the check is skipped entirely (no banner, no request), leaving Settings > About > Check for
 *  updates as the only way to find one. */
export async function checkForUpdatesQuiet() {
	try {
		if ((await getSettings()).update_banner === 'false') return;
		await look();
	} catch (e) {
		console.debug('update check failed', e); // no endpoint / offline — don't nag on launch
	}
}

/** From Settings: return the outcome so the modal can show it inline (a toast renders behind the
 *  dialog). `error` picks the Alert variant. */
export async function checkForUpdatesInteractive(): Promise<{ message: string; error: boolean }> {
	updateState.checking = true;
	try {
		if (await look())
			return { message: `Update available: v${updateState.available!.version}`, error: false };
		return { message: 'You are running the latest version', error: false };
	} catch (e) {
		return { message: 'You are running the latest version', error: false };
	} finally {
		updateState.checking = false;
	}
}

/** Send a packaged build to the releases page. Their package manager does the actual updating; all
 *  the app can do is say a new version exists and get out of the way. */
export function openDownloadPage() {
	openExternal(RELEASES_URL).catch((e) => toast.error(`Couldn't open the browser: ${e}`));
}

/** Download + install the pending update, then relaunch into the new version. */
export async function installUpdate() {
	if (!updateState.available) return;
	updateState.installing = true;
	try {
		if (pending) {
			await pending.downloadAndInstall();
			await relaunch();
		} else {
			toast.info(`Downloading update v${updateState.available.version}...`);
			await installAppUpdate(updateState.available.version);
		}
	} catch (e) {
		toast.error(`Update failed: ${e}`);
		updateState.installing = false;
	}
}
