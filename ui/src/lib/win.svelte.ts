// Shared window-maximized state: the resize borders hide when maximized, and the root container
// drops its rounded corners. One listener, initialized once by the root layout.
import { getCurrentWindow } from '@tauri-apps/api/window';

export const win = $state({ maximized: false });

let started = false;

export function initWin(): () => void {
	if (started) return () => {};
	started = true;
	const w = getCurrentWindow();
	// The window is created hidden (tauri.conf.json) so the window-state plugin can restore the
	// saved size before anything is on screen: it only restores once the webview is ready, so a
	// visible window would flash at the config's 1200x800, white, then snap (#45). By here the SPA
	// has mounted, so showing it now costs nothing and skips the flash.
	w.show().catch(() => {});
	const sync = () =>
		w
			.isMaximized()
			.then((m) => (win.maximized = m))
			.catch(() => {});
	sync();
	const un = w.onResized(sync);
	return () => un.then((u) => u());
}
