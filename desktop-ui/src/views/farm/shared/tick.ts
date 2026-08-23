import { ref } from 'vue';

/**
 * 全局唯一的秒级时钟（共享单例）。
 *
 * 取代各页面自己的 1s setInterval + 整表 map 重建（每秒为所有地块
 * 创建新对象会让 v-for 全量 re-render，是滚动卡顿的主因）。
 * 倒计时展示组件（LandCountdown）只订阅这一个 ref，父级数据不再变化。
 */
const nowSec = ref(Math.floor(Date.now() / 1000));
let timer: ReturnType<typeof setInterval> | null = null;

export function useSharedTick() {
  if (!timer) {
    timer = setInterval(() => {
      nowSec.value = Math.floor(Date.now() / 1000);
    }, 1000);
  }
  return { nowSec };
}
