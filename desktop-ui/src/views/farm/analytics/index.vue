<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import {
  NButton,
  NCard,
  NEmpty,
  NGi,
  NGrid,
  NInputNumber,
  NSelect,
  NSpace,
  NSpin,
  NTabPane,
  NTabs,
  NTag
} from 'naive-ui';
import {
  fetchGetFarmAnalyticsDetail,
  fetchGetFarmAutomationDetail,
  fetchGetFarmStatusDetail,
  fetchModifyFarmAutomation
} from '@/service/api';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { resolveCatalogImage } from '@/views/farm/game-config/shared';
import { $t } from '@/locales';

defineOptions({
  name: 'FarmAnalytics'
});

type RankingRow = NonNullable<Api.Farm.AnalyticsDetail['rankings']>[number];

interface StrategyDef {
  key: string;
  labelKey: App.I18n.I18nKey;
  metricKey: keyof RankingRow;
  unit: string;
  descKey: App.I18n.I18nKey;
}

const farmAccountStore = useFarmAccountStore();

const loading = ref(false);
const batchLoading = ref(false);
const activeTab = ref<'strategy' | 'blacklist'>('strategy');
const strategyLevel = ref(1);
const rankings = ref<RankingRow[]>([]);
const plantingStrategy = ref('max_exp');
const preferredSeedId = ref(0);
const plantBlacklist = ref<number[]>([]);

const strategyLabelMap: Record<string, App.I18n.I18nKey> = {
  preferred: 'page.farm.settings.preferredSeedId',
  level: 'page.farm.analytics.strategyLevel',
  max_exp: 'page.farm.analytics.strategyMaxExp',
  max_fert_exp: 'page.farm.analytics.strategyMaxFertExp',
  max_profit: 'page.farm.analytics.strategyMaxProfit',
  max_fert_profit: 'page.farm.analytics.strategyMaxFertProfit',
  bag_priority: 'page.farm.analytics.strategyBagPriority'
};

const strategies: StrategyDef[] = [
  {
    key: 'max_exp',
    labelKey: 'page.farm.analytics.metricExp',
    metricKey: 'expPerHour',
    unit: 'EXP',
    descKey: 'page.farm.analytics.metricExpDesc'
  },
  {
    key: 'max_profit',
    labelKey: 'page.farm.analytics.metricProfit',
    metricKey: 'profitPerHour',
    unit: '金币',
    descKey: 'page.farm.analytics.metricProfitDesc'
  },
  {
    key: 'max_fert_exp',
    labelKey: 'page.farm.analytics.metricFertExp',
    metricKey: 'normalFertilizerExpPerHour',
    unit: 'EXP',
    descKey: 'page.farm.analytics.metricFertExpDesc'
  },
  {
    key: 'max_fert_profit',
    labelKey: 'page.farm.analytics.metricFertProfit',
    metricKey: 'normalFertilizerProfitPerHour',
    unit: '金币',
    descKey: 'page.farm.analytics.metricFertProfitDesc'
  },
  {
    key: 'level',
    labelKey: 'page.farm.analytics.metricLevel',
    metricKey: 'level',
    unit: 'Lv',
    descKey: 'page.farm.analytics.metricLevelDesc'
  }
];

const plantableRankings = computed(() =>
  (rankings.value || []).filter(row => {
    const level = row.level;
    if (level === null || level === undefined) return true;
    return Number(level) <= Number(strategyLevel.value || 0);
  })
);

const availableCount = computed(() => plantableRankings.value.length);

const currentStrategyLabel = computed(() => {
  const key = strategyLabelMap[plantingStrategy.value];
  return key ? $t(key) : plantingStrategy.value;
});

const currentStrategyBestPlant = computed(() => {
  if (plantingStrategy.value === 'preferred' && preferredSeedId.value > 0) {
    return rankings.value.find(item => Number(item.seedId) === preferredSeedId.value) || null;
  }
  if (plantingStrategy.value === 'bag_priority') {
    return null;
  }
  return getStrategyBestPlant(plantingStrategy.value);
});

const blacklistRows = computed(() =>
  plantBlacklist.value.map(seedId => {
    const plant = rankings.value.find(item => Number(item.seedId) === seedId);
    return {
      seedId,
      name: plant?.name || `种子ID:${seedId}`,
      level: plant?.level,
      image: plantImage(plant?.image)
    };
  })
);

const plantSelectOptions = computed(() => {
  const known = new Map<number, RankingRow>();
  for (const row of rankings.value) {
    const id = Number(row.seedId);
    if (Number.isFinite(id) && id > 0) known.set(id, row);
  }
  const options = [...known.entries()].map(([seedId, row]) => ({
    label: `Lv${formatLv(row.level)} ${row.name || seedId} (${seedId})`,
    value: seedId
  }));
  for (const id of plantBlacklist.value) {
    if (!known.has(id)) {
      options.push({ label: `种子 ${id}`, value: id });
    }
  }
  return options;
});

function plantImage(path?: unknown) {
  return resolveCatalogImage(typeof path === 'string' ? path : '');
}

function formatLv(level: unknown) {
  if (level === null || level === undefined || level === '' || Number(level) < 0) {
    return '未知';
  }
  return String(level);
}

function getStrategyBestPlant(strategyKey: string): RankingRow | null {
  const strategy = strategies.find(item => item.key === strategyKey);
  if (!strategy) return null;
  const filtered = plantableRankings.value;
  if (!filtered.length) return null;

  const metric = strategy.metricKey;
  return [...filtered].sort((a, b) => {
    const av = Number(a[metric] ?? -1);
    const bv = Number(b[metric] ?? -1);
    if (!Number.isFinite(av) && !Number.isFinite(bv)) return 0;
    if (!Number.isFinite(av)) return 1;
    if (!Number.isFinite(bv)) return -1;
    return bv - av;
  })[0];
}

function metricDisplay(strategyKey: string): string {
  const plant = getStrategyBestPlant(strategyKey);
  const strategy = strategies.find(item => item.key === strategyKey);
  if (!plant || !strategy) return '-';
  const value = plant[strategy.metricKey];
  if (strategyKey === 'level') return formatLv(value);
  const num = Number(value);
  return Number.isFinite(num) ? String(num) : '-';
}

async function syncStrategyLevel(accountId: number) {
  const { error, data } = await fetchGetFarmStatusDetail(accountId);
  const level = !error ? Number(data?.level || 0) : 0;
  if (level > 0) {
    strategyLevel.value = level;
  }
}

async function loadAutomation(accountId: number) {
  const { error, data } = await fetchGetFarmAutomationDetail(accountId);
  if (error || !data) {
    plantingStrategy.value = 'max_exp';
    preferredSeedId.value = 0;
    plantBlacklist.value = [];
    return;
  }
  plantingStrategy.value = data.plantingStrategy || 'max_exp';
  preferredSeedId.value = Number(data.preferredSeedId || 0);
  plantBlacklist.value = (data.plantBlacklist || []).map(id => Number(id)).filter(id => Number.isFinite(id) && id > 0);
}

async function savePlantBlacklist(next: number[]) {
  if (!farmAccountStore.currentAccountId) return;
  batchLoading.value = true;
  try {
    const { error } = await fetchModifyFarmAutomation({
      accountId: farmAccountStore.currentAccountId,
      plantBlacklist: next
    });
    if (!error) {
      plantBlacklist.value = next;
      window.$message?.success($t('page.farm.analytics.blacklistUpdated'));
    }
  } finally {
    batchLoading.value = false;
  }
}

async function handleAddAllToBlacklist() {
  const allSeedIds = rankings.value.map(item => Number(item.seedId)).filter(id => Number.isFinite(id) && id > 0);
  const merged = [...new Set([...plantBlacklist.value, ...allSeedIds])];
  await savePlantBlacklist(merged);
}

async function handleClearBlacklist() {
  await savePlantBlacklist([]);
}

async function handleRemoveFromBlacklist(seedId: number) {
  await savePlantBlacklist(plantBlacklist.value.filter(id => id !== seedId));
}

async function handleBlacklistSelectUpdate(value: number[]) {
  const next = [...new Set((value || []).map(id => Number(id)).filter(id => Number.isFinite(id) && id > 0))];
  await savePlantBlacklist(next);
}

async function loadPage() {
  if (!farmAccountStore.currentAccountId) {
    rankings.value = [];
    plantingStrategy.value = 'max_exp';
    preferredSeedId.value = 0;
    plantBlacklist.value = [];
    strategyLevel.value = 1;
    return;
  }
  loading.value = true;
  try {
    const accountId = farmAccountStore.currentAccountId;
    await Promise.all([syncStrategyLevel(accountId), loadAutomation(accountId)]);
    const { error, data } = await fetchGetFarmAnalyticsDetail({
      accountId,
      days: 7,
      sort: 'exp'
    });
    rankings.value = !error && data ? data.rankings || [] : [];
  } finally {
    loading.value = false;
  }
}

watch(
  () => farmAccountStore.currentAccountId,
  () => {
    void loadPage();
  }
);

onMounted(async () => {
  if (!farmAccountStore.accounts.length) {
    await farmAccountStore.loadAccounts();
  }
  await loadPage();
});
</script>

<template>
  <div class="min-h-500px flex-col-stretch gap-16px overflow-auto">
    <NEmpty
      v-if="!farmAccountStore.currentAccountId"
      class="py-48px"
      :description="$t('page.farm.common.selectAccount')"
    />

    <template v-else>
      <NTabs v-model:value="activeTab" type="line" animated>
        <NTabPane name="strategy" :tab="$t('page.farm.analytics.tabStrategy')">
          <NSpin :show="loading">
            <div class="flex-col gap-16px">
              <NCard :bordered="false" size="small" class="card-wrapper">
                <div class="mb-8px text-15px font-medium">
                  {{ $t('page.farm.analytics.currentStrategy') }}: {{ currentStrategyLabel }}
                </div>
                <div class="mb-12px text-12px text-gray-500">
                  {{ $t('page.farm.analytics.currentStrategyHint') }}
                </div>

                <div
                  v-if="plantingStrategy === 'bag_priority'"
                  class="rounded-6px bg-amber-50 px-12px py-10px text-13px text-amber-700 dark:bg-amber-900/20 dark:text-amber-300"
                >
                  {{ $t('page.farm.analytics.bagPriorityHint') }}
                </div>
                <div v-else-if="currentStrategyBestPlant" class="flex items-center gap-12px">
                  <div
                    class="h-48px w-48px flex shrink-0 items-center justify-center overflow-hidden rounded-8px border border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-800"
                  >
                    <img
                      v-if="plantImage(currentStrategyBestPlant.image)"
                      :src="plantImage(currentStrategyBestPlant.image)"
                      class="h-40px w-40px object-contain"
                      loading="lazy"
                    />
                    <span v-else class="text-22px text-gray-400">🌱</span>
                  </div>
                  <div class="min-w-0 flex-1">
                    <div class="text-15px font-medium">
                      {{ currentStrategyBestPlant.name }}
                      <span class="ml-6px text-12px text-gray-500">
                        Lv{{ formatLv(currentStrategyBestPlant.level) }}
                      </span>
                      <span class="ml-6px text-12px text-gray-400">{{ currentStrategyBestPlant.seasons || 1 }}季</span>
                    </div>
                    <div class="mt-6px flex flex-wrap gap-12px text-12px text-gray-500">
                      <span>经验/时: {{ currentStrategyBestPlant.expPerHour ?? '-' }}</span>
                      <span>利润/时: {{ currentStrategyBestPlant.profitPerHour ?? '-' }}</span>
                      <span>普肥经验/时: {{ currentStrategyBestPlant.normalFertilizerExpPerHour ?? '-' }}</span>
                      <span>
                        普肥利润/时:
                        {{ currentStrategyBestPlant.normalFertilizerProfitPerHour ?? '-' }}
                      </span>
                    </div>
                  </div>
                </div>
                <div v-else class="text-13px text-gray-400">
                  {{ $t('page.farm.analytics.noPlantable') }}
                </div>
              </NCard>

              <NCard :bordered="false" size="small" class="card-wrapper">
                <template #header>
                  <div>
                    <div class="text-15px font-medium">{{ $t('page.farm.analytics.strategyCompare') }}</div>
                    <div class="mt-2px text-12px text-gray-500">
                      {{ $t('page.farm.analytics.strategyCompareHint') }}
                    </div>
                  </div>
                </template>
                <template #header-extra>
                  <NSpace align="center" size="small">
                    <span class="text-13px text-gray-500">{{ $t('page.farm.analytics.refLevel') }}</span>
                    <NInputNumber v-model:value="strategyLevel" class="w-90px" size="small" :min="1" :max="999" />
                  </NSpace>
                </template>

                <NGrid cols="1 s:2 m:3 l:5" responsive="screen" :x-gap="12" :y-gap="12">
                  <NGi v-for="strategy in strategies" :key="strategy.key">
                    <div
                      class="h-full rounded-8px border px-12px py-12px"
                      :class="
                        plantingStrategy === strategy.key
                          ? 'border-primary ring-1 ring-primary'
                          : 'border-gray-200 dark:border-gray-700'
                      "
                    >
                      <div class="mb-8px flex items-center justify-between gap-8px">
                        <div class="text-13px font-medium">{{ $t(strategy.labelKey) }}</div>
                        <NTag v-if="plantingStrategy === strategy.key" size="tiny" type="info" :bordered="false">
                          {{ $t('page.farm.analytics.currentTag') }}
                        </NTag>
                      </div>
                      <div class="mb-8px text-12px text-gray-400">{{ $t(strategy.descKey) }}</div>

                      <template v-if="getStrategyBestPlant(strategy.key)">
                        <div class="mb-8px flex items-center gap-8px">
                          <div
                            class="h-40px w-40px flex shrink-0 items-center justify-center overflow-hidden rounded-6px border border-gray-200 bg-gray-50 dark:border-gray-600 dark:bg-gray-800"
                          >
                            <img
                              v-if="plantImage(getStrategyBestPlant(strategy.key)?.image)"
                              :src="plantImage(getStrategyBestPlant(strategy.key)?.image)"
                              class="h-32px w-32px object-contain"
                              loading="lazy"
                            />
                            <span v-else class="text-16px text-gray-400">🌱</span>
                          </div>
                          <div class="min-w-0 flex-1">
                            <div class="truncate text-13px font-medium">
                              {{ getStrategyBestPlant(strategy.key)?.name }}
                            </div>
                            <div class="text-12px text-gray-500">
                              Lv{{ formatLv(getStrategyBestPlant(strategy.key)?.level) }}
                            </div>
                          </div>
                        </div>
                        <div
                          class="flex items-baseline justify-between rounded-6px bg-gray-50 px-8px py-6px dark:bg-gray-900/40"
                        >
                          <span class="text-12px text-gray-500">
                            {{ strategy.unit }}{{ strategy.key === 'level' ? '' : '/时' }}
                          </span>
                          <span class="text-15px font-semibold">{{ metricDisplay(strategy.key) }}</span>
                        </div>
                      </template>
                      <div v-else class="py-16px text-center text-12px text-gray-400">
                        {{ $t('page.farm.analytics.noPlantable') }}
                      </div>
                    </div>
                  </NGi>
                </NGrid>

                <div class="mt-12px text-12px text-gray-500">
                  {{
                    $t('page.farm.analytics.availableHint', {
                      available: availableCount,
                      total: rankings.length
                    })
                  }}
                </div>
              </NCard>
            </div>
          </NSpin>
        </NTabPane>

        <NTabPane name="blacklist">
          <template #tab>
            <NSpace :size="6" align="center">
              <span>{{ $t('page.farm.analytics.tabBlacklist') }}</span>
              <NTag v-if="plantBlacklist.length" size="tiny" type="error" :bordered="false" round>
                {{ plantBlacklist.length }}
              </NTag>
            </NSpace>
          </template>

          <NCard :bordered="false" size="small" class="card-wrapper">
            <template #header>
              <div>
                <div class="text-15px font-medium">{{ $t('page.farm.analytics.stealBlacklist') }}</div>
                <div class="mt-2px text-12px text-gray-500">
                  {{ $t('page.farm.analytics.stealBlacklistHint') }}
                </div>
              </div>
            </template>
            <template #header-extra>
              <NSpace>
                <NButton
                  size="small"
                  :loading="batchLoading"
                  :disabled="!rankings.length"
                  @click="handleAddAllToBlacklist"
                >
                  {{ $t('page.farm.analytics.addAllBlacklist') }}
                </NButton>
                <NButton
                  v-if="plantBlacklist.length"
                  size="small"
                  type="error"
                  secondary
                  :loading="batchLoading"
                  @click="handleClearBlacklist"
                >
                  {{ $t('page.farm.analytics.clearBlacklist') }}
                </NButton>
              </NSpace>
            </template>

            <div class="mb-16px">
              <div class="mb-8px text-13px text-gray-500">{{ $t('page.farm.analytics.addBlacklist') }}</div>
              <NSelect
                :value="plantBlacklist"
                class="w-full"
                multiple
                filterable
                clearable
                :loading="batchLoading"
                :options="plantSelectOptions"
                :placeholder="$t('page.farm.settings.blacklistSelectPlaceholder')"
                :max-tag-count="6"
                @update:value="handleBlacklistSelectUpdate"
              />
            </div>

            <NEmpty
              v-if="!plantBlacklist.length"
              class="py-32px"
              :description="$t('page.farm.analytics.blacklistEmpty')"
            />
            <div v-else class="flex-col gap-10px">
              <div
                v-for="row in blacklistRows"
                :key="row.seedId"
                class="flex items-center justify-between rounded-8px bg-gray-50 px-12px py-10px dark:bg-gray-800/60"
              >
                <div class="flex items-center gap-12px">
                  <div
                    class="h-40px w-40px flex shrink-0 items-center justify-center overflow-hidden rounded-6px border border-gray-200 bg-white dark:border-gray-600 dark:bg-gray-700"
                  >
                    <img v-if="row.image" :src="row.image" class="h-32px w-32px object-contain" loading="lazy" />
                    <span v-else class="text-16px text-gray-400">🌱</span>
                  </div>
                  <div>
                    <div class="text-13px font-medium">{{ row.name }}</div>
                    <div class="text-12px text-gray-400">ID: {{ row.seedId }}</div>
                  </div>
                </div>
                <NButton
                  size="small"
                  type="error"
                  quaternary
                  :loading="batchLoading"
                  @click="handleRemoveFromBlacklist(row.seedId)"
                >
                  {{ $t('page.farm.analytics.removeBlacklist') }}
                </NButton>
              </div>
            </div>
          </NCard>
        </NTabPane>
      </NTabs>
    </template>
  </div>
</template>

<style scoped></style>
