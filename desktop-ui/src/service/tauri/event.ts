import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { isTauriRuntime } from './client';

/** 订阅 Rust `app-event` */
export async function listenDesktopAppEvent(
  handler: (payload: Desktop.AppEventPayload) => void
): Promise<UnlistenFn> {
  if (!isTauriRuntime()) {
    return () => undefined;
  }
  return listen<Desktop.AppEventPayload>('app-event', event => {
    handler(event.payload);
  });
}
