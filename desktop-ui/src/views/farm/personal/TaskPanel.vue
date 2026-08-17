<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NCard, NEmpty, NSpin, NTag } from 'naive-ui';
import { fetchGetFarmDailyGifts } from '@/service/api';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { $t } from '@/locales';
import DailyOverview from './DailyOverview.vue';

defineOptions({ name: 'FarmPersonalTaskPanel' });

const props = defineProps<{
  connected: boolean;
}>();

const farmAccountStore = useFarmAccountStore();
const loading = ref(false);
const dailyGifts = ref<Api.Farm.DailyGiftsResponse | null>(null);

const growth = computed(() => dailyGifts.value?.growth || null);
const growthTasks = computed(() => growth.value?.tasks || []);

async function loadDailyGifts() {
  if (!farmAccountStore.currentAccountId || !props.connected) {
    dailyGifts.value = null;
    return;
  }
  loading.value = true;
  try {
    const { data, error } = await fetchGetFarmDailyGifts(farmAccountStore.currentAccountId);
    if (!error && data) {
      dailyGifts.value = data;
    }
  } finally {
    loading.value = false;
  }
}

function formatTaskProgress(task: Api.Farm.GrowthTaskRow) {
  if (task.isCompleted) return $t('page.farm.personal.growthCompleted');
  if (!task.progress && !task.totalProgress) return $t('page.farm.personal.growthInProgress');
  return $t('page.farm.personal.growthProgress', {
    current: task.progress,
    total: task.totalProgress
  });
}

watch(
  () => [farmAccountStore.currentAccountId, props.connected] as const,
  () => {
    void loadDailyGifts();
  },
  { immediate: true }
);

defineExpose({ refresh: loadDailyGifts });
</script>

<template>
  <div class="flex-col-stretch gap-16px">
    <NSpin :show="loading">
      <DailyOverview :daily-gifts="dailyGifts" />
    </NSpin>

    <NCard :title="$t('page.farm.personal.growthTasksTitle')" :bordered="false" size="small" class="card-wrapper">
      <template #header-extra>
        <NTag v-if="growth" size="small" :type="growth.doneToday ? 'success' : 'info'" :bordered="false">
          {{
            growth.doneToday
              ? $t('page.farm.personal.growthDone')
              : $t('page.farm.personal.growthProgress', {
                  current: growth.completedCount,
                  total: growth.totalCount
                })
          }}
        </NTag>
      </template>

      <div v-if="growthTasks.length === 0" class="py-16px">
        <NEmpty :description="$t('page.farm.personal.growthEmpty')" />
      </div>
      <div v-else class="flex-col gap-8px">
        <div v-for="task in growthTasks" :key="task.id" class="farm-growth-row">
          <div class="min-w-0 flex-1 truncate text-14px">{{ task.desc }}</div>
          <div class="flex-y-center gap-8px">
            <NTag size="small" :type="task.isCompleted ? 'success' : 'info'" :bordered="false">
              {{
                task.isCompleted ? $t('page.farm.personal.growthCompleted') : $t('page.farm.personal.growthInProgress')
              }}
            </NTag>
            <span class="text-12px opacity-55 whitespace-nowrap">{{ formatTaskProgress(task) }}</span>
          </div>
        </div>
      </div>
    </NCard>
  </div>
</template>

<style scoped>
.farm-growth-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  border: 1px solid rgba(0, 0, 0, 0.06);
  border-radius: 10px;
}
</style>
