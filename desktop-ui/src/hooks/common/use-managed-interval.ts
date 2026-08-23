import { onUnmounted } from 'vue';

/**
 * 组件级受管 interval：自动在 onUnmounted 清理，杜绝页面切换后定时器泄漏。
 * 取代各页面散落的 `let xxTimer` + 手工 clearInterval 模式。
 *
 * const timer = useManagedInterval();
 * timer.start(() => void load(), 60_000);  // start 会先清旧
 * timer.stop();
 */
export function useManagedInterval() {
  let timer: ReturnType<typeof setInterval> | null = null;

  function start(fn: () => void, ms: number) {
    stop();
    timer = setInterval(fn, ms);
  }

  function stop() {
    if (timer) {
      clearInterval(timer);
      timer = null;
    }
  }

  onUnmounted(stop);

  return { start, stop, isRunning: () => timer !== null };
}
