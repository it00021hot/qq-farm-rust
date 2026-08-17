/** Format Tauri / IPC invoke errors for toasts (avoid `[object Object]`). */
export function formatInvokeError(err: unknown): string {
  if (err == null) return '未知错误';
  if (typeof err === 'string') {
    const trimmed = err.trim();
    if ((trimmed.startsWith('{') && trimmed.endsWith('}')) || (trimmed.startsWith('[') && trimmed.endsWith(']'))) {
      try {
        return formatInvokeError(JSON.parse(trimmed));
      } catch {
        return trimmed;
      }
    }
    return trimmed || '未知错误';
  }
  if (err instanceof Error) {
    const msg = err.message?.trim();
    if (msg && msg !== '[object Object]') {
      if ((msg.startsWith('{') && msg.endsWith('}')) || msg.includes('"message"')) {
        try {
          return formatInvokeError(JSON.parse(msg));
        } catch {
          /* fall through */
        }
      }
      return msg;
    }
  }
  if (typeof err === 'object') {
    const o = err as Record<string, unknown>;
    if (o.message != null && o.message !== err) {
      return formatInvokeError(o.message);
    }
    if (typeof o.error === 'string' || typeof o.error === 'object') {
      return formatInvokeError(o.error);
    }
    try {
      return JSON.stringify(o);
    } catch {
      return '未知错误';
    }
  }
  return String(err);
}
