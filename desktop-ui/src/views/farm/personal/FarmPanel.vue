<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { NButton, NCard, NEmpty, NPopconfirm, NSpace, NSpin, NTag, useMessage } from 'naive-ui';
import { fetchFarmOperate, fetchGetFarmLands } from '@/service/api';
import LandCountdown from '@/views/farm/shared/LandCountdown.vue';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { useManagedInterval } from '@/hooks/common/use-managed-interval';
import { resolveCatalogImage } from '@/views/farm/game-config/shared';
import { formatCareerCount, formatCareerStealRatio } from '@/views/farm/shared/career';
import {
  landCardClass,
  landGridStyle,
  landIdLabel,
  soilLabel,
  soilLevelClass,
  tickMatureLands,
  visibleLands,
  visibleOwnFarmOps
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
const career = ref<Api.Farm.Career | null>(null);

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

const visibleOperateOptions = computed(() => {
  const available = visibleOwnFarmOps(lands.value);
  return operateOptions.filter(item => available.has(item.op));
});

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

/** 变异名称（有 mutantEffects 用名称，退回 mutantConfigIds 数字）。 */
function mutantNames(land: Api.Farm.LandRow) {
  const effects = (land.mutantEffects || []).map(effect => String(effect.name || '').trim()).filter(Boolean);
  if (effects.length) return effects;
  return (land.mutantConfigIds || []).map(String);
}

function mutantIconSrc(effect: Api.Farm.LandMutantEffect) {
  return resolveCatalogImage(effect.iconUrl);
}

/** 紫晶共鸣经验加成（万分值 → 百分比）。 */
function purpleCrystalPercent(land: Api.Farm.LandRow) {
  const bonus = Number(land.purpleCrystalResonanceExpBonus || 0);
  return bonus > 0 ? Math.round(bonus / 100) : 0;
}

/** 互动道具效果行（itemName 列表）。 */
function interactionNames(land: Api.Farm.LandRow) {
  return (land.interactionEffects || [])
    .map(effect => String(effect.itemName || '').trim())
    .filter(Boolean);
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
    career.value = null;
    return;
  }
  landsLoading.value = true;
  try {
    const { data, error } = await fetchGetFarmLands(farmAccountStore.currentAccountId);
    if (!error && data) {
      // 记录绝对成熟时间戳，倒计时由 LandCountdown 用共享时钟渲染
      const now = Math.floor(Date.now() / 1000);
      lands.value = (data.lands || []).map((land: Api.Farm.LandRow) => ({
        ...land,
        matureAt: now + Number(land.matureInSec || 0)
      }));
      summary.value = data.summary || null;
      career.value = data.career || null;
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

const refreshTimer = useManagedInterval();

function startTimers() {
  stopTimers();
  refreshTimer.start(() => void loadLands(), 60000);
}

function stopTimers() {
  refreshTimer.stop();
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
        <NPopconfirm v-for="item in visibleOperateOptions" :key="item.op" @positive-click="handleOperate(item.op)">
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
      <!-- 生涯统计 -->
      <div
        v-if="career"
        class="mb-12px flex flex-wrap items-center gap-16px rounded-8px bg-violet-50 px-16px py-10px text-13px dark:bg-violet-900/20"
      >
        <span class="text-gray-500">{{ $t('page.farm.personal.careerTitle') }}</span>
        <span>
          {{ $t('page.farm.personal.careerHarvest') }}
          <strong class="text-15px font-semibold">{{ formatCareerCount(career.harvest) }}</strong>
        </span>
        <span>
          {{ $t('page.farm.personal.careerSteal') }}
          <strong class="text-15px font-semibold">{{ formatCareerCount(career.steal) }}</strong>
        </span>
        <span>
          {{ $t('page.farm.personal.careerRatio') }}
          <strong class="text-15px font-semibold">{{ formatCareerStealRatio(career) }}</strong>
        </span>
      </div>

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
          <LandCountdown :at="land.matureAt || 0" :total="land.totalGrowTime || 0" :level="land.level" :phase="land.phaseName" />
          <div v-if="mutantNames(land).length || purpleCrystalPercent(land) > 0" class="flex-center flex-wrap gap-4px">
            <span
              class="flex-y-center gap-2px rounded-4px bg-amber-50 px-5px py-1px text-11px text-amber-700 dark:bg-amber-900/30 dark:text-amber-300"
              :title="(land.mutantEffects || []).map(effect => effect.description || effect.name).filter(Boolean).join('\n')"
            >
              <template v-for="(effect, idx) in land.mutantEffects || []" :key="`${effect.id}-${idx}`">
                <img
                  v-if="mutantIconSrc(effect)"
                  :src="mutantIconSrc(effect)"
                  class="h-14px w-14px object-contain"
                  loading="lazy"
                />
              </template>
              {{ $t('page.farm.personal.mutantBadge', { names: mutantNames(land).join('+') }) }}
            </span>
            <NTag v-if="purpleCrystalPercent(land) > 0" size="tiny" type="error" :bordered="false">
              {{ $t('page.farm.personal.purpleCrystalBadge', { percent: purpleCrystalPercent(land) }) }}
            </NTag>
          </div>
          <div v-if="interactionNames(land).length || land.needInteractionCleanup" class="flex-center flex-wrap gap-4px">
            <span
              class="rounded-4px bg-gray-100 px-5px py-1px text-11px text-gray-600 dark:bg-gray-800 dark:text-gray-300"
              :title="(land.interactionEffects || []).map(effect => `${effect.itemName || effect.itemId}`).join('、')"
            >
              {{ $t('page.farm.personal.interactionTitle') }}: {{ interactionNames(land).join('+') || '--' }}
            </span>
            <NTag
              v-if="land.needInteractionCleanup"
              size="tiny"
              type="warning"
              :bordered="false"
              :title="$t('page.farm.personal.interactionCleanupHint')"
            >
              {{ $t('page.farm.personal.interactionCleanup') }}
            </NTag>
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
