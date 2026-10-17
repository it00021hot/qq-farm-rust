<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { NButton, NCard, NEmpty, NModal, NSelect, NSpace, NSpin, NTag, useMessage } from 'naive-ui';
import { fetchFarmFertilizeAll, fetchFarmOperate, fetchGetFarmLands } from '@/service/api';
import LandCountdown from '@/views/farm/shared/LandCountdown.vue';
import LandDetailModal from './LandDetailModal.vue';
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
/** 土地详情弹窗（bot e5f3552：LandCard「土地详情」入口） */
const detailLandId = ref<number | null>(null);
const detailLand = computed(() => lands.value.find(land => land.id === detailLandId.value) || null);

/** 一键施肥（bot e15e72e）：确认弹窗 + 化肥类型 */
const fertilizeAllOpen = ref(false);
const fertilizeAllType = ref<'normal' | 'organic'>('normal');
const fertilizeAllPending = ref(false);
const fertilizeAllOptions = computed(() => [
  { label: $t('page.farm.personal.fertilizeAllNormal'), value: 'normal' },
  { label: $t('page.farm.personal.fertilizeAllOrganic'), value: 'organic' }
]);

/** 可施肥候选（对齐 bot `isFertilizeCandidate`：已解锁、非副地块、生长中且未成熟） */
function isFertilizeCandidate(land: Api.Farm.LandRow) {
  return (
    !!land.unlocked &&
    !land.occupiedByMaster &&
    String(land.status || '') === 'growing' &&
    Number(land.matureInSec || 0) > 0
  );
}

const fertilizeCandidateCount = computed(() => lands.value.filter(isFertilizeCandidate).length);

/** 用回包 updatedLands 原地刷新（保留 LandCountdown 的绝对成熟时间戳） */
function applyFertilizeLands(updates: Api.Farm.LandRow[] | undefined) {
  if (!updates?.length) return;
  const now = Math.floor(Date.now() / 1000);
  const byId = new Map(updates.map(land => [land.id, land]));
  lands.value = lands.value.map(land => {
    const next = byId.get(land.id);
    if (!next) return land;
    return { ...next, matureAt: now + Number(next.matureInSec || 0) };
  });
}

async function handleFertilizeAll() {
  const accountId = farmAccountStore.currentAccountId;
  if (!accountId || fertilizeAllPending.value) return;
  if (!fertilizeCandidateCount.value) {
    fertilizeAllOpen.value = false;
    message.info($t('page.farm.personal.fertilizeAllEmpty'));
    return;
  }
  fertilizeAllPending.value = true;
  try {
    const { data, error } = await fetchFarmFertilizeAll(accountId, fertilizeAllType.value);
    fertilizeAllOpen.value = false;
    if (error || !data) {
      message.error(error?.message || $t('page.farm.personal.fertilizeAllFailed'));
      return;
    }
    if (!data.successCount) {
      message.info($t('page.farm.personal.fertilizeAllEmpty'));
      return;
    }
    applyFertilizeLands(data.updatedLands);
    let text = $t('page.farm.personal.fertilizeAllSuccess', { count: data.successCount });
    if (data.skippedCount) {
      text += $t('page.farm.personal.fertilizeAllSkipped', { count: data.skippedCount });
    }
    const remaining = Number(data.fertilizerRemainingSec || 0);
    if (remaining > 0) {
      text += $t('page.farm.personal.fertilizeAllRemaining', {
        hours: Math.floor(remaining / 3600),
        minutes: Math.floor((remaining % 3600) / 60)
      });
    }
    message.success(text);
  } finally {
    fertilizeAllPending.value = false;
  }
}

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
  return (land.interactionEffects || []).map(effect => String(effect.itemName || '').trim()).filter(Boolean);
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
    detailLandId.value = null;
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
        <NButton
          size="small"
          ghost
          :loading="fertilizeAllPending"
          :disabled="!connected || operating || !fertilizeCandidateCount"
          @click="fertilizeAllOpen = true"
        >
          {{ $t('page.farm.personal.fertilizeAll') }}
        </NButton>
        <NButton
          v-for="item in visibleOperateOptions"
          :key="item.op"
          size="small"
          :type="item.type"
          ghost
          :loading="operating"
          :disabled="!connected"
          @click="handleOperate(item.op)"
        >
          {{ $t(`page.farm.personal.op.${item.op}`) }}
        </NButton>
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
          <button
            type="button"
            class="self-start rounded-4px text-11px text-gray-500 underline decoration-dotted underline-offset-3 hover:text-primary"
            :aria-label="$t('page.farm.personal.landDetail.title')"
            @click.stop="detailLandId = land.id"
          >
            {{ $t('page.farm.personal.landDetail.title') }}
          </button>
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
          <LandCountdown
            :at="land.matureAt || 0"
            :total="land.totalGrowTime || 0"
            :level="land.level"
            :phase="land.phaseName"
          />
          <div v-if="mutantNames(land).length || purpleCrystalPercent(land) > 0" class="flex-center flex-wrap gap-4px">
            <span
              class="flex-y-center gap-2px rounded-4px bg-amber-50 px-5px py-1px text-11px text-amber-700 dark:bg-amber-900/30 dark:text-amber-300"
              :title="
                (land.mutantEffects || [])
                  .map(effect => effect.description || effect.name)
                  .filter(Boolean)
                  .join('\n')
              "
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
          <div
            v-if="interactionNames(land).length || land.needInteractionCleanup"
            class="flex-center flex-wrap gap-4px"
          >
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

    <LandDetailModal :land="detailLand" :connected="connected" @close="detailLandId = null" />

    <NModal :show="fertilizeAllOpen" @update:show="!$event && !fertilizeAllPending && (fertilizeAllOpen = false)">
      <NCard
        class="w-[calc(100vw-32px)] max-w-sm"
        :title="$t('page.farm.personal.fertilizeAllTitle')"
        closable
        @close="!fertilizeAllPending && (fertilizeAllOpen = false)"
      >
        <div class="space-y-4">
          <p class="text-sm text-gray-500">{{ $t('page.farm.personal.fertilizeAllMessage') }}</p>
          <div class="flex items-center justify-between gap-3">
            <span>{{ $t('page.farm.personal.fertilizeAllType') }}</span>
            <NSelect
              v-model:value="fertilizeAllType"
              class="w-40"
              :options="fertilizeAllOptions"
              :disabled="fertilizeAllPending"
            />
          </div>
          <p class="text-sm text-gray-500">
            {{ $t('page.farm.personal.fertilizeAllCount', { count: fertilizeCandidateCount }) }}
          </p>
          <div class="flex justify-end gap-2">
            <NButton secondary :disabled="fertilizeAllPending" @click="fertilizeAllOpen = false">
              {{ $t('common.cancel') }}
            </NButton>
            <NButton type="primary" :loading="fertilizeAllPending" @click="handleFertilizeAll">
              {{ $t('page.farm.personal.fertilizeAllConfirm') }}
            </NButton>
          </div>
        </div>
      </NCard>
    </NModal>
  </NCard>
</template>
