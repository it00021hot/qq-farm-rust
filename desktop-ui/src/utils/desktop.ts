/** Desktop (Tauri) shell helpers — align chrome with qq-farm-desktop / qq-farm-web. */

import { computed, ref } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { isTauriRuntime } from '@/service/tauri/client';
import { getAppInfo } from '@/service/api/application';

const desktopShellActive = ref(false);
const nativePlatform = ref('');

export const isDesktopShell = computed(() => desktopShellActive.value);

/** Detect shell + apply window chrome classes. */
export async function bootstrapDesktopShell(): Promise<boolean> {
  if (typeof window === 'undefined' || !isTauriRuntime()) {
    desktopShellActive.value = false;
    return false;
  }
  // Metadata comes from the native target, independently of viewport size.
  try {
    nativePlatform.value = (await getAppInfo()).platform;
  } catch {
    nativePlatform.value = /android/i.test(navigator.userAgent) ? 'android' : '';
  }
  desktopShellActive.value = !['android', 'ios'].includes(nativePlatform.value);
  if (desktopShellActive.value) installDesktopContextMenuGuard();
  document.documentElement.classList.toggle('desktop-shell', desktopShellActive.value);
  document.documentElement.classList.toggle('mobile-shell', !desktopShellActive.value);
  document.documentElement.classList.toggle('desktop-windows', isDesktopWindows());
  document.documentElement.classList.toggle('desktop-mac', isDesktopMac());
  return desktopShellActive.value;
}

export function isDesktopWindows(): boolean {
  if (!desktopShellActive.value || typeof navigator === 'undefined') return false;
  return nativePlatform.value === 'windows' || /windows/i.test(navigator.userAgent);
}

export function isDesktopMac(): boolean {
  if (!desktopShellActive.value || typeof navigator === 'undefined') return false;
  return nativePlatform.value === 'macos' || /mac/i.test(navigator.userAgent);
}

export async function desktopStartDragging(): Promise<void> {
  if (!desktopShellActive.value) return;
  await getCurrentWindow().startDragging();
}

export async function desktopMinimise(): Promise<void> {
  if (!desktopShellActive.value) return;
  await getCurrentWindow().minimize();
}

export async function desktopToggleMaximise(): Promise<void> {
  if (!desktopShellActive.value) return;
  const win = getCurrentWindow();
  if (await win.isMaximized()) {
    await win.unmaximize();
  } else {
    await win.maximize();
  }
}

export async function desktopClose(): Promise<void> {
  if (!desktopShellActive.value) return;
  await getCurrentWindow().close();
}

export async function desktopIsFullscreen(): Promise<boolean> {
  if (!desktopShellActive.value) return false;
  return getCurrentWindow().isFullscreen();
}

/** WKWebView `requestFullscreen` is a no-op; toggle the native Tauri window. */
export async function desktopToggleFullscreen(): Promise<boolean> {
  if (!desktopShellActive.value) return false;
  const win = getCurrentWindow();
  const next = !(await win.isFullscreen());
  await win.setFullscreen(next);
  return next;
}

let contextMenuGuardInstalled = false;

/** Block WebView “View Source / Inspect”; keep copy-paste on inputs and Vue custom menus. */
export function installDesktopContextMenuGuard(): void {
  if (typeof document === 'undefined' || contextMenuGuardInstalled) return;
  contextMenuGuardInstalled = true;
  document.addEventListener(
    'contextmenu',
    event => {
      if (!desktopShellActive.value) return;
      const target = event.target;
      if (target instanceof Element && target.closest('input, textarea, [contenteditable="true"]')) {
        return;
      }
      event.preventDefault();
    },
    true
  );
}
