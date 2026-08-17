<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { NButton, NCard, NEmpty, NPopconfirm, NSpace, NSpin, NTag, useMessage } from 'naive-ui';
import { fetchFarmOperate, fetchGetFarmLands } from '@/service/api';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { resolveCatalogImage } from '@/views/farm/game-config/shared';
import {
  landCardClass,
  landGridStyle,
  landIdLabel,
  soilLabel,
  soilLevelClass,
  visibleLands
} from '@/views/farm/shared/land-display';
import { $t } from '@/locales';

defineOptions({ name: 'FarmPersonalFarmPanel' });

const props = defineProps<{
  connected: boolean;
}>();

type OperateOp = Api.Farm.OperateParams['op'];

const farmAccountStore = useFarmAccountStore();
const message = useMessage();

const landsLoading = ref(false);
const operating = ref(false);
const lands = ref<Api.Farm.LandRow[]>([]);
const summary = ref<Api.Farm.LandSummary | null>(null);

const operateOptions: { op: OperateOp; type: NaiveUI.ThemeColor }[] = [
  { op: 'harvest', type: 'success' },
  { op: 'clear', type: 'info' },
  { op: 'plant', type: 'primary' },
  { op: 'upgrade', type: 'warning' },
  { op: 'all', type: 'error' }
];

const summaryTags = computed(() => {
  const s = summary.value;
  if (!s) return [];
  return [
    {
      key: 'harvestable',
      label: $t('page.farm.personal.summaryHarvestable'),
      value: s.harvestable,
      type: 'warning' as const
    },
    { key: 'growing', label: $t('page.farm.personal.summaryGrowing'), value: s.growing, type: 'success' as const },
    { key: 'empty', label: $t('page.farm.personal.summaryEmpty'), value: s.empty, type: 'default' as const },
    { key: 'dead', label: $t('page.farm.personal.summaryDead'), value: s.dead, type: 'error' as const }
  ];
});

const displayLands = computed(() => visibleLands(lands.value));

function landStatusTag(status: string, occupiedByMaster?: boolean): { type: NaiveUI.ThemeColor; label: string } {
  if (occupiedByMaster) {
    return { type: 'info', label: $t('page.farm.personal.statusOccupied') };
  }
  const map: Record<string, { type: NaiveUI.ThemeColor; label: string }> = {
    harvestable: { type: 'warning', label: $t('page.farm.personal.statusHarvestable') },
    growing: { type: 'success', label: $t('page.farm.personal.statusGrowing') },
    empty: { type: 'default', label: $t('page.farm.personal.statusEmpty') },
    dead: { type: 'error', label: $t('page.farm.personal.statusDead') },
    locked: { type: 'default', label: $t('page.farm.personal.statusLocked') },
    stealable: { type: 'info', label: $t('page.farm.personal.statusStealable') }
  };
  return map[status] || { type: 'default', label: status || '-' };
}

function landImageSrc(land: Api.Farm.LandRow) {
  return resolveCatalogImage(land.seedImage);
}

function growProgress(land: Api.Farm.LandRow) {
  const mature = Number(land.matureInSec || 0);
  const total = Number(land.totalGrowTime || 0);
  if (total <= 0) return 0;
  // matureInSec = remaining; progress = elapsed / total
  return Math.max(0, Math.min(100, Math.round(((total - mature) / total) * 100)));
}

function formatDuration(sec: number) {
  if (sec <= 0) return '';
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = sec % 60;
  return `${h > 0 ? `${h}:` : ''}${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
}

async function loadLands() {
  if (!farmAccountStore.currentAccountId || !props.connected) {
    lands.value = [];
    summary.value = null;
    return;
  }
  landsLoading.value = true;
  try {
    const { data, error } = await fetchGetFarmLands(farmAccountStore.currentAccountId);
    if (!error && data) {
      lands.value = data.lands || [];
      summary.value = data.summary || null;
    }
  } finally {
    landsLoading.value = false;
  }
}

async function handleOperate(op: OperateOp) {
  if (!farmAccountStore.currentAccountId) return;
  operating.value = true;
  try {
    const { error } = await fetchFarmOperate({ accountId: farmAccountStore.currentAccountId, op });
    if (error) {
      message.error(error.message || $t('page.farm.personal.operateFailed'));
      return;
    }
    message.success($t('page.farm.personal.operateSuccess'));
    await loadLands();
  } finally {
    operating.value = false;
  }
}

let tickTimer: ReturnType<typeof setInterval> | null = null;
let refreshTimer: ReturnType<typeof setInterval> | null = null;

function startTimers() {
  stopTimers();
  tickTimer = setInterval(() => {
    lands.value = lands.value.map(land =>
      land.matureInSec && land.matureInSec > 0 ? { ...land, matureInSec: land.matureInSec - 1 } : land
    );
  }, 1000);
  refreshTimer = setInterval(() => {
    void loadLands();
  }, 60000);
}

function stopTimers() {
  if (tickTimer) {
    clearInterval(tickTimer);
    tickTimer = null;
  }
  if (refreshTimer) {
    clearInterval(refreshTimer);
    refreshTimer = null;
  }
}

watch(
  () => [farmAccountStore.currentAccountId, props.connected] as const,
  () => {
    void loadLands();
  },
  { immediate: true }
);

onMounted(() => {
  startTimers();
});

onUnmounted(() => {
  stopTimers();
});

defineExpose({ refresh: loadLands });
</script>

<template>
  <NCard :title="$t('page.farm.personal.landsTitle')" :bordered="false" size="small" class="card-wrapper">
    <template #header-extra>
      <NSpace size="small">
        <NPopconfirm v-for="item in operateOptions" :key="item.op" @positive-click="handleOperate(item.op)">
          <template #trigger>
            <NButton size="small" :type="item.type" ghost :loading="operating" :disabled="!connected">
              {{ $t(`page.farm.personal.op.${item.op}`) }}
            </NButton>
          </template>
          {{ $t(`page.farm.personal.confirm.${item.op}`) }}
        </NPopconfirm>
      </NSpace>
    </template>

    <NSpin :show="landsLoading">
      <NSpace v-if="summaryTags.length" class="mb-12px" size="small">
        <NTag v-for="tag in summaryTags" :key="tag.key" :type="tag.type" size="small">
          {{ tag.label }}: {{ tag.value }}
        </NTag>
      </NSpace>

      <div v-if="displayLands.length === 0" class="py-24px">
        <NEmpty :description="$t('common.noData')" />
      </div>

      <div v-else class="farm-land-grid">
        <div v-for="land in displayLands" :key="land.id" :class="landCardClass(land)" :style="landGridStyle(land)">
          <div class="flex-y-center justify-between gap-4px">
            <span class="text-12px opacity-50">{{ landIdLabel(land) }}</span>
            <div class="flex-y-center flex-wrap justify-end gap-4px">
              <NTag v-if="land.plantSize && land.plantSize > 1" size="tiny" type="warning">
                {{ $t('page.farm.personal.plantSizeBadge', { size: land.plantSize }) }}
              </NTag>
              <NTag size="small" :type="landStatusTag(land.status, land.occupiedByMaster).type">
                {{ landStatusTag(land.status, land.occupiedByMaster).label }}
              </NTag>
            </div>
          </div>
          <div class="farm-land-crop flex-center min-h-48px">
            <img
              v-if="landImageSrc(land)"
              :src="landImageSrc(land)"
              :alt="land.plantName"
              class="max-h-48px max-w-full object-contain"
              loading="lazy"
              referrerpolicy="no-referrer"
            />
            <span v-else class="text-24px opacity-40">🌱</span>
          </div>
          <div class="truncate text-center text-13px font-medium" :title="land.plantName">
            {{ land.plantName || '-' }}
          </div>
          <div class="text-center text-12px opacity-70">
            <span v-if="land.matureInSec && land.matureInSec > 0" class="text-orange-500">
              {{ formatDuration(land.matureInSec) }}
            </span>
            <span v-else>{{ land.phaseName || '-' }}</span>
          </div>
          <div
            v-if="land.matureInSec && land.matureInSec > 0 && land.totalGrowTime"
            class="farm-progress"
            :class="soilLevelClass(land.level)"
          >
            <div class="farm-progress-fill" :style="{ width: `${growProgress(land)}%` }" />
          </div>
          <div class="flex-center flex-wrap gap-4px">
            <span v-if="soilLabel(land.level)" class="farm-soil-badge" :class="soilLevelClass(land.level)">
              {{ soilLabel(land.level) }}
            </span>
            <NTag v-if="land.totalSeason && land.totalSeason > 1" size="tiny" type="info">
              {{
                $t('page.farm.personal.seasonBadge', {
                  current: land.currentSeason || 1,
                  total: land.totalSeason
                })
              }}
            </NTag>
            <NTag v-if="land.needWater" size="tiny" type="info">💧</NTag>
            <NTag v-if="land.needWeed" size="tiny" type="success">🌿</NTag>
            <NTag v-if="land.needBug" size="tiny" type="error">🐛</NTag>
          </div>
        </div>
      </div>
    </NSpin>
  </NCard>
</template>
