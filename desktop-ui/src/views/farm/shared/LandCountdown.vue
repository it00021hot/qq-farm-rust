<script setup lang="ts">
import { computed } from 'vue';
import { soilLevelClass } from './land-display';
import { useSharedTick } from './tick';

/**
 * 地块倒计时文本 + 生长进度条。
 *
 * `at` 是绝对成熟时间戳（秒）。组件内部用共享时钟计算剩余，
 * 每秒只有这个小组件自身重渲染，父级的地块卡片完全不刷新。
 */
const props = defineProps<{
  at: number;
  total?: number;
  level?: number;
  phase?: string;
}>();

const { nowSec } = useSharedTick();

const remain = computed(() => Math.max(0, Math.floor(props.at || 0) - nowSec.value));

const text = computed(() => {
  const sec = remain.value;
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = sec % 60;
  return `${h > 0 ? `${h}:` : ''}${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
});

const progress = computed(() => {
  const total = Number(props.total || 0);
  if (!total) return 0;
  return Math.min(100, Math.max(0, ((total - remain.value) / total) * 100));
});
</script>

<template>
  <div v-if="remain > 0">
    <div class="text-center text-12px opacity-70">
      <span class="text-orange-500">{{ text }}</span>
    </div>
    <div v-if="total" class="farm-progress" :class="soilLevelClass(Number(level || 0))">
      <div class="farm-progress-fill" :style="{ width: `${progress}%` }" />
    </div>
  </div>
  <div v-else class="text-center text-12px opacity-70">{{ phase || '-' }}</div>
</template>
