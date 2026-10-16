<script setup lang="ts">
import { computed } from 'vue';
import { $t } from '@/locales';

/**
 * 等级升级时间预测（bot 62e1880 LevelForecast.vue）。
 * 数据来自服务端 status.levelForecast：按当前作物排期推算，不足时用会话历史速率外推。
 */
const props = defineProps<{ forecast?: Api.Farm.Status['levelForecast'] }>();

const value = computed(() => props.forecast);

const timeText = computed(() => {
  const f = value.value;
  if (!f || !f.estimatedAt) {
    return f?.plotCount
      ? $t('page.farm.dashboard.forecastNotEnough')
      : $t('page.farm.dashboard.forecastWaiting');
  }
  if (f.source === 'ready') return $t('page.farm.dashboard.forecastReady');
  const minutes = Math.max(1, Math.ceil((Number(f.estimatedAt) - Number(f.generatedAt)) / 60000));
  const duration = minutes < 60 ? `${minutes} min` : `${(minutes / 60).toFixed(1)} h`;
  return f.source === 'planted'
    ? $t('page.farm.dashboard.forecastPlanted', { duration })
    : $t('page.farm.dashboard.forecastHistory', { duration });
});

const basis = computed(() => {
  switch (value.value?.source) {
    case 'planted':
      return $t('page.farm.dashboard.forecastBasisPlanted');
    case 'ready':
      return $t('page.farm.dashboard.forecastBasisReady');
    case 'mixed':
      return $t('page.farm.dashboard.forecastBasisMixed');
    case 'history':
      return $t('page.farm.dashboard.forecastBasisHistory');
    default:
      return '';
  }
});
</script>

<template>
  <div class="mt-8px flex flex-col gap-2px text-12px" aria-label="升级预测">
    <div class="flex flex-wrap items-center justify-between gap-4px">
      <span class="text-gray-700 dark:text-gray-200">{{ timeText }}</span>
      <span class="text-gray-400">{{ basis }}</span>
    </div>
    <div
      v-if="value?.plotCount"
      class="text-gray-500"
      title="按当前季作物和土地经验加成估算，不计未确认的随机加成；经验需收获后获得。"
    >
      {{ $t('page.farm.dashboard.forecastPlots', { count: value.plotCount, exp: Number(value.scheduledExp || 0).toLocaleString() }) }}
    </div>
    <div v-if="value?.plotCount && !value.automatic" class="text-amber-600 dark:text-amber-400">
      {{ $t('page.farm.dashboard.forecastManualHint') }}
    </div>
  </div>
</template>
