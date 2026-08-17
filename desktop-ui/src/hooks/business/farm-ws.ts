import { onUnmounted, ref } from 'vue';
import { listenDesktopAppEvent } from '@/service/tauri/event';

export type FarmWsHandler = (type: string, payload: unknown, raw?: Api.Farm.WsMessage) => void;

export interface UseFarmWsOptions {
  path?: string;
  reconnect?: boolean;
  reconnectDelay?: number;
  maxReconnectDelay?: number;
  onMessage?: FarmWsHandler;
  onOpen?: () => void;
  onClose?: () => void;
  onError?: (event: Event) => void;
}

/**
 * Desktop realtime: thin bridge from Tauri `app-event` to the web WS handler shape.
 * Envelope is `{ type, payload, accountId }` — same as qq-farm-web Socket.IO.
 */
export function useFarmWs(options: UseFarmWsOptions = {}) {
  const connected = ref(false);
  const lastType = ref('');
  const lastPayload = ref<unknown>(null);
  let unlisten: (() => void) | undefined;

  async function connect() {
    if (unlisten) return;
    unlisten = await listenDesktopAppEvent(event => {
      const type = event.type || 'status:update';
      const payload = event.payload ?? event;
      const accountId = event.accountId ? Number(event.accountId) || undefined : undefined;
      const next = { type, payload, accountId } as Api.Farm.WsMessage;
      lastType.value = type;
      lastPayload.value = payload;
      options.onMessage?.(type, payload, next);
    });
    connected.value = true;
    options.onOpen?.();
  }

  function disconnect() {
    unlisten?.();
    unlisten = undefined;
    connected.value = false;
    options.onClose?.();
  }

  function send(_type: string, _payload?: unknown) {
    return false;
  }

  function destroy() {
    disconnect();
  }

  onUnmounted(() => {
    destroy();
  });

  void connect();

  return {
    connected,
    lastType,
    lastPayload,
    connect,
    disconnect,
    send,
    destroy
  };
}
