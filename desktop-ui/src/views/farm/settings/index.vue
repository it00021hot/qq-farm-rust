<script setup lang="ts">
import { openUrl } from '@tauri-apps/plugin-opener';
import { computed, onMounted, onUnmounted, reactive, ref, watch } from 'vue';
import {
  NButton,
  NCard,
  NDivider,
  NEmpty,
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NSelect,
  NSpace,
  NSpin,
  NSwitch,
  NTabPane,
  NTabs,
  NTag,
  NText,
  NTimePicker
} from 'naive-ui';
import {
  farmAllFertilizerLandTypes,
  farmFertilizerLandTypeOptions,
  farmFertilizerModeOptions,
  translateStringOptions
} from '@/constants/business';
import {
  fetchFertilizerCheckAndBuy,
  fetchGetDevicePresets,
  fetchGetFarmAnalyticsDetail,
  fetchGetFarmAutomationDetail,
  fetchGetFarmBagSeeds,
  fetchGetFarmSeeds,
  fetchGetOfflineReminder,
  fetchGetQqBotBindStatus,
  fetchGetSystemConfig,
  fetchModifyFarmAutomation,
  fetchPollQqBotBind,
  fetchResetSystemConfig,
  fetchSaveOfflineReminder,
  fetchSetSystemConfig,
  fetchStartQqBotBind,
  fetchTestOfflineReminder,
  fetchUnbindQqBot,
  type SystemConfigPayload
} from '@/service/api';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { $t } from '@/locales';

defineOptions({
  name: 'FarmSettings'
});

interface SeedOptionItem {
  seedId: number;
  name: string;
  requiredLevel: number;
  price: number;
  size: number;
  locked?: boolean;
  soldOut?: boolean;
}

interface BagSeedItem {
  seedId: number;
  name: string;
  count: number;
  requiredLevel: number;
  plantSize: number;
}

const farmAccountStore = useFarmAccountStore();

const loading = ref(false);
const strategySaving = ref(false);
const automationSaving = ref(false);
const offlineSaving = ref(false);
const offlineTesting = ref(false);
const activeTab = ref<'strategy' | 'automation' | 'offline' | 'system'>('strategy');

const systemConfigLoading = ref(false);
const systemConfigSaving = ref(false);
const devicePresets = ref<Array<{ id: string; name: string; description?: string; deviceInfo?: Record<string, unknown> }>>(
  []
);
const selectedPresetId = ref('');
const defaultSystemConfig = ref<SystemConfigPayload>(createDefaultSystemConfig());
const localSystemConfig = ref<SystemConfigPayload>(createDefaultSystemConfig());

const platformOptions = [
  { label: 'QQ', value: 'qq' },
  { label: '微信', value: 'wx' }
];
const osOptions = [
  { label: 'Windows', value: 'Windows' },
  { label: 'iOS', value: 'iOS' },
  { label: 'Android', value: 'Android' }
];

function createDefaultSystemConfig(): SystemConfigPayload {
  return {
    serverUrl: '',
    clientVersion: '',
    platform: 'qq',
    os: 'Windows',
    deviceInfo: {
      os: 'Windows',
      clientVersion: '',
      sysSoftware: 'Windows',
      network: 'wifi',
      memory: '16384',
      deviceId: 'DESKTOP-PC<WPC>',
      userAgent: ''
    }
  };
}

const plantingStrategy = ref('preferred');
const preferredSeedId = ref<number | null>(0);
const bagSeedPriority = ref<number[]>([]);
const bagSeedFallbackStrategy = ref('level');
const plantOrderRandom = ref(false);
const plantDelaySeconds = ref(0);
const stealDelaySeconds = ref(1);
const plantBlacklist = ref<number[]>([]);
const intervals = reactive<Api.Farm.IntervalsConfig>({
  farmMin: 20,
  farmMax: 25,
  helpMin: 20,
  helpMax: 25,
  stealMin: 10,
  stealMax: 15
});
const quietHours = reactive<Api.Farm.QuietHoursConfig>({
  enabled: false,
  start: '01:00',
  end: '07:30'
});

const fertilizerBuy = reactive({
  organicCount: 1,
  organicThresholdHours: 10,
  normalCount: 1,
  normalThresholdHours: 10,
  checkIntervalMinutes: 60
});

const offline = reactive<Api.Farm.OfflineReminder>({
  provider: 'none',
  qqBot: {
    appId: '',
    clientSecret: ''
  },
  qqBotBinding: {
    userOpenid: '',
    boundAt: 0,
    nickname: ''
  },
  wechatBot: {},
  title: '账号下线提醒',
  msg: '账号下线',
  offlineDeleteSec: 0
});

const qqBotBindStatus = reactive<Api.Farm.QqBotBindStatus>({
  credentialsConfigured: false,
  bound: false,
  binding: { userOpenid: '' },
  botInviteUrl: ''
});
const qqBotBindSessionId = ref('');
const qqBotBindQrDataUrl = ref('');
const qqBotBindLoading = ref(false);
const qqBotBindPolling = ref(false);
let qqBotBindPollTimer: ReturnType<typeof setInterval> | null = null;

const qqBotCredentialsReady = computed(
  () => Boolean(offline.qqBot.appId.trim()) && Boolean(offline.qqBot.clientSecret.trim())
);

const qqBotBindStateLabel = computed(() => {
  if (qqBotBindPolling.value) return $t('page.farm.settings.qqBotBindWaiting');
  if (qqBotBindStatus.bound || offline.qqBotBinding.userOpenid) return $t('page.farm.settings.qqBotBindBound');
  return $t('page.farm.settings.qqBotBindUnbound');
});

const providerOptions = computed(() => [
  { label: $t('page.farm.settings.providerNone'), value: 'none' },
  { label: $t('page.farm.settings.providerQqBot'), value: 'qq_bot' },
  { label: $t('page.farm.settings.providerWechatBot'), value: 'wechat_bot', disabled: true }
]);

const currentProviderDocUrl = computed(() => (offline.provider === 'qq_bot' ? 'https://bot.q.qq.com/wiki/' : ''));

/** Bot AutomationConfig keys only (qq-farm-bot Settings) */
const automation = reactive<Api.Farm.AutomationConfig>({
  farm: true,
  farm_push: true,
  land_upgrade: true,
  friend: true,
  friend_steal: true,
  friend_help: false,
  friend_bad: false,
  friend_help_exp_limit: false,
  task: true,
  sell: true,
  fertilizer: 'smart',
  fertilizer_gift: true,
  fertilizer_buy_organic: false,
  fertilizer_buy_normal: false,
  fertilizer_multi_season: true,
  fertilizer_land_types: [...farmAllFertilizerLandTypes],
  fertilizer_smart_seconds: 360,
  skip_own_weed_bug: true
});

const showFertilizerBuyPanel = computed(
  () => !!automation.fertilizer_buy_organic || !!automation.fertilizer_buy_normal
);

const seedOptions = ref<SeedOptionItem[]>([]);
const bagSeeds = ref<BagSeedItem[]>([]);
const bagSeedsLoading = ref(false);
const bagSeedsError = ref<string | null>(null);
const draggingBagSeedId = ref<number | null>(null);
const strategyPreviewLabel = ref<string | null>(null);

let seedOptionsRequestRevision = 0;
let bagSeedsRequestRevision = 0;
let bagSortRequestRevision = 0;
let previewRequestRevision = 0;

const fertilizerOptions = computed(() => translateStringOptions(farmFertilizerModeOptions));
const fertilizerLandTypeOptions = computed(() => translateStringOptions(farmFertilizerLandTypeOptions));
const showSmartSeconds = computed(() => automation.fertilizer === 'smart');
const accountRunning = computed(() => farmAccountStore.currentAccount?.runStatus === 1);

const strategyOptions = computed(() => [
  { label: $t('page.farm.settings.strategyPreferred'), value: 'preferred' },
  { label: $t('page.farm.settings.strategyLevel'), value: 'level' },
  { label: $t('page.farm.settings.strategyMaxExp'), value: 'max_exp' },
  { label: $t('page.farm.settings.strategyMaxFertExp'), value: 'max_fert_exp' },
  { label: $t('page.farm.settings.strategyMaxProfit'), value: 'max_profit' },
  { label: $t('page.farm.settings.strategyMaxFertProfit'), value: 'max_fert_profit' },
  { label: $t('page.farm.settings.strategyBagPriority'), value: 'bag_priority' }
]);

const bagFallbackStrategyOptions = computed(() => [
  { label: $t('page.farm.settings.strategyLevel'), value: 'level' },
  { label: $t('page.farm.settings.strategyMaxExp'), value: 'max_exp' },
  { label: $t('page.farm.settings.strategyMaxFertExp'), value: 'max_fert_exp' },
  { label: $t('page.farm.settings.strategyMaxProfit'), value: 'max_profit' },
  { label: $t('page.farm.settings.strategyMaxFertProfit'), value: 'max_fert_profit' },
  { label: $t('page.farm.settings.strategyPreferred'), value: 'preferred' }
]);

const analyticsSortByMap: Record<string, string> = {
  max_exp: 'exp',
  max_fert_exp: 'fert',
  max_profit: 'profit',
  max_fert_profit: 'fert_profit'
};

const showPreferredSeedSelect = computed(
  () =>
    plantingStrategy.value === 'preferred' ||
    (plantingStrategy.value === 'bag_priority' && bagSeedFallbackStrategy.value === 'preferred')
);

const preferredSeedSelectOptions = computed(() => {
  const options: { label: string; value: number; disabled?: boolean }[] = [
    { label: $t('page.farm.settings.preferredSeedAuto'), value: 0 }
  ];
  options.push(
    ...seedOptions.value.map(seed => ({
      label: `Lv${seed.requiredLevel} ${seed.name}`,
      value: seed.seedId,
      disabled: !!seed.locked || !!seed.soldOut
    }))
  );
  return options;
});

const visibleBagSeedIds = computed(() => bagSeeds.value.map(seed => seed.seedId));

const sortedBagSeeds = computed(() => {
  const itemMap = new Map(bagSeeds.value.map(seed => [seed.seedId, seed]));
  return normalizeVisibleBagSeedOrder()
    .map(seedId => itemMap.get(seedId))
    .filter((seed): seed is BagSeedItem => !!seed);
});

function normalizeIDList(ids: Array<number | string> | null | undefined): number[] {
  if (!Array.isArray(ids)) return [];
  const out: number[] = [];
  for (const raw of ids) {
    const id = Number(raw);
    if (Number.isFinite(id) && id > 0 && !out.includes(id)) out.push(id);
  }
  return out;
}

function normalizeVisibleBagSeedOrder(priority: number[] = bagSeedPriority.value) {
  const visibleIds = visibleBagSeedIds.value;
  const visibleSet = new Set(visibleIds);
  const normalized: number[] = [];
  for (const seedId of priority || []) {
    const id = Number(seedId);
    if (visibleSet.has(id) && !normalized.includes(id)) normalized.push(id);
  }
  for (const seedId of visibleIds) {
    if (!normalized.includes(seedId)) normalized.push(seedId);
  }
  return normalized;
}

function mergeVisibleBagSeedOrder(visibleOrder: number[]) {
  const visibleIds = visibleBagSeedIds.value;
  const visibleSet = new Set(visibleIds);
  const normalizedVisible = normalizeVisibleBagSeedOrder(visibleOrder);
  const existing = [...new Set((bagSeedPriority.value || []).map(Number).filter(id => Number.isFinite(id) && id > 0))];
  const merged: number[] = [];
  let visibleIndex = 0;

  for (const seedId of existing) {
    if (visibleSet.has(seedId)) {
      const replacement = normalizedVisible[visibleIndex++];
      if (replacement !== undefined && !merged.includes(replacement)) merged.push(replacement);
    } else if (!merged.includes(seedId)) {
      // Keep offline priority IDs so they retain order when restocked.
      merged.push(seedId);
    }
  }
  while (visibleIndex < normalizedVisible.length) {
    const seedId = normalizedVisible[visibleIndex++]!;
    if (!merged.includes(seedId)) merged.push(seedId);
  }
  return merged;
}

function materializeVisibleBagSeedOrder() {
  return normalizeVisibleBagSeedOrder();
}

function saveVisibleBagSeedOrder(visibleOrder: number[]) {
  bagSortRequestRevision += 1;
  bagSeedPriority.value = mergeVisibleBagSeedOrder(visibleOrder);
}

function compareBagSeedsByLevel(a: BagSeedItem, b: BagSeedItem) {
  if (a.requiredLevel !== b.requiredLevel) return b.requiredLevel - a.requiredLevel;
  return a.seedId - b.seedId;
}

async function fetchSeedOptions(accountId = farmAccountStore.currentAccountId) {
  if (!accountId) {
    seedOptions.value = [];
    return;
  }
  const requestRevision = ++seedOptionsRequestRevision;
  try {
    const { error, data } = await fetchGetFarmSeeds(accountId);
    if (requestRevision !== seedOptionsRequestRevision || accountId !== farmAccountStore.currentAccountId) return;
    if (!error && data) {
      seedOptions.value = data.map((seed: any) => ({
        seedId: seed.seedId,
        name: seed.name,
        requiredLevel: Number(seed.requiredLevel) || 0,
        price: Number(seed.price) || 0,
        size: Number(seed.size) || 1,
        locked: !!seed.locked,
        soldOut: !!seed.soldOut
      }));
    } else {
      seedOptions.value = [];
    }
  } catch {
    if (requestRevision === seedOptionsRequestRevision && accountId === farmAccountStore.currentAccountId) {
      seedOptions.value = [];
    }
  }
}

async function fetchBagSeeds(accountId = farmAccountStore.currentAccountId) {
  if (!accountId) return;
  const requestRevision = ++bagSeedsRequestRevision;
  bagSeedsLoading.value = true;
  bagSeedsError.value = null;
  try {
    const { error, data } = await fetchGetFarmBagSeeds(accountId);
    if (requestRevision !== bagSeedsRequestRevision || accountId !== farmAccountStore.currentAccountId) return;
    if (error || !data) {
      bagSeeds.value = [];
      const msg = error?.message || $t('page.farm.settings.bagSeedOrderError');
      bagSeedsError.value =
        !accountRunning.value || /未运行|not running/i.test(msg)
          ? $t('page.farm.settings.bagSeedOrderNeedRunning')
          : msg;
      return;
    }
    bagSeeds.value = (data as BagSeedItem[])
      .map((item: any) => ({
        seedId: Number(item.seedId),
        name: item.name || String(item.seedId),
        count: Number(item.count) || 0,
        requiredLevel: Number(item.requiredLevel) || 0,
        plantSize: Number(item.plantSize) || 1
      }))
      .filter(seed => seed.seedId > 0 && seed.count > 0 && seed.plantSize >= 1);
  } catch (e: any) {
    if (requestRevision === bagSeedsRequestRevision && accountId === farmAccountStore.currentAccountId) {
      bagSeeds.value = [];
      bagSeedsError.value = e?.message || $t('page.farm.settings.bagSeedOrderError');
    }
  } finally {
    if (requestRevision === bagSeedsRequestRevision && accountId === farmAccountStore.currentAccountId) {
      bagSeedsLoading.value = false;
    }
  }
}

async function sortBagSeedsByFallbackStrategy(strategy: string, accountId = farmAccountStore.currentAccountId) {
  if (!accountId || accountId !== farmAccountStore.currentAccountId) return;
  const requestRevision = ++bagSortRequestRevision;
  const strategyAtRequest = strategy;
  const items = [...bagSeeds.value];
  const ordered = [...items].sort(compareBagSeedsByLevel);

  if (strategyAtRequest === 'preferred') {
    const preferred = Number(preferredSeedId.value || 0);
    ordered.sort((a, b) => {
      const aPreferred = a.seedId === preferred ? 0 : 1;
      const bPreferred = b.seedId === preferred ? 0 : 1;
      return aPreferred - bPreferred || compareBagSeedsByLevel(a, b);
    });
  } else if (strategyAtRequest !== 'level') {
    const sortBy = analyticsSortByMap[strategyAtRequest];
    if (sortBy) {
      try {
        const { error, data } = await fetchGetFarmAnalyticsDetail({ accountId, sort: sortBy });
        if (
          requestRevision !== bagSortRequestRevision ||
          accountId !== farmAccountStore.currentAccountId ||
          bagSeedFallbackStrategy.value !== strategyAtRequest
        ) {
          return;
        }
        const rankMap = new Map<number, number>();
        const rankings = !error && data?.rankings ? data.rankings : [];
        rankings.forEach((item: any, index: number) => rankMap.set(Number(item.seedId), index));
        ordered.sort((a, b) => {
          const aRank = rankMap.get(a.seedId) ?? Number.MAX_SAFE_INTEGER;
          const bRank = rankMap.get(b.seedId) ?? Number.MAX_SAFE_INTEGER;
          return aRank - bRank || compareBagSeedsByLevel(a, b);
        });
      } catch {
        // Keep stable level/id order when ranking fails.
      }
    }
  }

  if (
    requestRevision !== bagSortRequestRevision ||
    accountId !== farmAccountStore.currentAccountId ||
    bagSeedFallbackStrategy.value !== strategyAtRequest
  ) {
    return;
  }
  saveVisibleBagSeedOrder(ordered.map(seed => seed.seedId));
}

async function ensureBagSeedsForUserSort(accountId: number) {
  if (bagSeedsLoading.value || bagSeeds.value.length === 0) await fetchBagSeeds(accountId);
}

async function handleBagFallbackStrategyChange(value: string) {
  const accountId = farmAccountStore.currentAccountId;
  if (!accountId) return;
  await ensureBagSeedsForUserSort(accountId);
  if (accountId === farmAccountStore.currentAccountId && bagSeedFallbackStrategy.value === value) {
    await sortBagSeedsByFallbackStrategy(value, accountId);
  }
}

async function handlePreferredSeedChange(value: number | null) {
  preferredSeedId.value = value ?? 0;
  const accountId = farmAccountStore.currentAccountId;
  if (!accountId || plantingStrategy.value !== 'bag_priority' || bagSeedFallbackStrategy.value !== 'preferred') {
    return;
  }
  await ensureBagSeedsForUserSort(accountId);
  if (accountId === farmAccountStore.currentAccountId && bagSeedFallbackStrategy.value === 'preferred') {
    await sortBagSeedsByFallbackStrategy('preferred', accountId);
  }
}

async function resetBagSeedPriority() {
  const accountId = farmAccountStore.currentAccountId;
  const strategy = bagSeedFallbackStrategy.value;
  if (!accountId) return;
  await ensureBagSeedsForUserSort(accountId);
  if (accountId === farmAccountStore.currentAccountId && bagSeedFallbackStrategy.value === strategy) {
    await sortBagSeedsByFallbackStrategy(strategy, accountId);
  }
}

function moveBagSeed(seedId: number, direction: -1 | 1) {
  const nextOrder = materializeVisibleBagSeedOrder();
  const index = nextOrder.indexOf(seedId);
  const targetIndex = index + direction;
  if (index < 0 || targetIndex < 0 || targetIndex >= nextOrder.length) return;
  const temp = nextOrder[index]!;
  nextOrder[index] = nextOrder[targetIndex]!;
  nextOrder[targetIndex] = temp;
  saveVisibleBagSeedOrder(nextOrder);
}

function startBagSeedDrag(seedId: number, event: DragEvent) {
  materializeVisibleBagSeedOrder();
  draggingBagSeedId.value = seedId;
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('text/plain', String(seedId));
  }
}

function endBagSeedDrag() {
  draggingBagSeedId.value = null;
}

function dragOverBagSeed(_seedId: number, event: DragEvent) {
  if (draggingBagSeedId.value === null) return;
  event.preventDefault();
  if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
}

function dropBagSeed(seedId: number, event: DragEvent) {
  event.preventDefault();
  const sourceSeedId = draggingBagSeedId.value ?? Number(event.dataTransfer?.getData('text/plain') || '');
  if (!sourceSeedId || sourceSeedId === seedId) {
    endBagSeedDrag();
    return;
  }
  const nextOrder = materializeVisibleBagSeedOrder();
  const sourceIndex = nextOrder.indexOf(sourceSeedId);
  const targetIndex = nextOrder.indexOf(seedId);
  if (sourceIndex < 0 || targetIndex < 0) {
    endBagSeedDrag();
    return;
  }
  const [source] = nextOrder.splice(sourceIndex, 1);
  const newTargetIndex = sourceIndex < targetIndex ? targetIndex - 1 : targetIndex;
  nextOrder.splice(newTargetIndex, 0, source!);
  saveVisibleBagSeedOrder(nextOrder);
  endBagSeedDrag();
}

function applyDetail(data: Api.Farm.AccountAutomationDetail) {
  const src = data.automation || {};
  automation.farm = !!src.farm;
  automation.farm_push = !!src.farm_push;
  automation.land_upgrade = !!src.land_upgrade;
  automation.friend = !!src.friend;
  automation.friend_steal = !!src.friend_steal;
  automation.friend_help = !!src.friend_help;
  automation.friend_bad = !!src.friend_bad;
  automation.friend_help_exp_limit = !!src.friend_help_exp_limit;
  automation.task = !!src.task;
  automation.sell = !!src.sell;
  automation.fertilizer = src.fertilizer || 'smart';
  automation.fertilizer_gift = !!src.fertilizer_gift;
  automation.fertilizer_buy_organic = !!src.fertilizer_buy_organic;
  automation.fertilizer_buy_normal = !!src.fertilizer_buy_normal;
  automation.fertilizer_multi_season = src.fertilizer_multi_season !== false;
  automation.fertilizer_land_types =
    Array.isArray(src.fertilizer_land_types) && src.fertilizer_land_types.length > 0
      ? [...src.fertilizer_land_types]
      : [...farmAllFertilizerLandTypes];
  automation.fertilizer_smart_seconds = src.fertilizer_smart_seconds ?? 360;
  automation.skip_own_weed_bug = !!src.skip_own_weed_bug;

  if (data.intervals) Object.assign(intervals, data.intervals);
  if (data.friendQuietHours) Object.assign(quietHours, data.friendQuietHours);
  plantingStrategy.value = data.plantingStrategy || 'preferred';
  preferredSeedId.value = data.preferredSeedId ?? 0;
  bagSeedPriority.value = [...(data.bagSeedPriority || [])];
  bagSeedFallbackStrategy.value = data.bagSeedFallbackStrategy || 'level';
  plantOrderRandom.value = !!data.plantOrderRandom;
  plantDelaySeconds.value = data.plantDelaySeconds ?? 0;
  stealDelaySeconds.value = data.stealDelaySeconds ?? 1;
  plantBlacklist.value = normalizeIDList(data.plantBlacklist);
  fertilizerBuy.organicCount = data.fertilizerBuyOrganicCount ?? 1;
  fertilizerBuy.organicThresholdHours = data.fertilizerBuyOrganicThresholdHours ?? 10;
  fertilizerBuy.normalCount = data.fertilizerBuyNormalCount ?? 1;
  fertilizerBuy.normalThresholdHours = data.fertilizerBuyNormalThresholdHours ?? 10;
  fertilizerBuy.checkIntervalMinutes = data.fertilizerBuyCheckIntervalMinutes ?? 60;
}

async function loadConfig() {
  if (!farmAccountStore.currentAccountId) return;
  loading.value = true;
  draggingBagSeedId.value = null;
  bagSeeds.value = [];
  bagSeedsError.value = null;
  strategyPreviewLabel.value = null;
  try {
    const [{ error, data }] = await Promise.all([
      fetchGetFarmAutomationDetail(farmAccountStore.currentAccountId),
      fetchSeedOptions(farmAccountStore.currentAccountId)
    ]);
    if (!error && data) {
      applyDetail(data);
    }
    if (plantingStrategy.value === 'bag_priority') {
      await fetchBagSeeds(farmAccountStore.currentAccountId);
    }
  } finally {
    loading.value = false;
  }
}

async function handleSaveStrategy() {
  if (!farmAccountStore.currentAccountId) return;
  strategySaving.value = true;
  try {
    let priority = [...bagSeedPriority.value];
    if (plantingStrategy.value === 'bag_priority') {
      bagSortRequestRevision += 1;
      priority = mergeVisibleBagSeedOrder(normalizeVisibleBagSeedOrder(priority));
      bagSeedPriority.value = priority;
    }
    const { error } = await fetchModifyFarmAutomation({
      accountId: farmAccountStore.currentAccountId,
      intervals: { ...intervals },
      plantingStrategy: plantingStrategy.value,
      preferredSeedId: preferredSeedId.value ?? 0,
      bagSeedPriority: priority,
      bagSeedFallbackStrategy: bagSeedFallbackStrategy.value,
      plantOrderRandom: plantOrderRandom.value,
      plantDelaySeconds: plantDelaySeconds.value,
      stealDelaySeconds: stealDelaySeconds.value,
      plantBlacklist: [...plantBlacklist.value],
      friendQuietHours: { ...quietHours }
    });
    if (!error) {
      window.$message?.success($t('page.farm.settings.saveStrategySuccess'));
    }
  } finally {
    strategySaving.value = false;
  }
}

async function handleSaveAutomation() {
  if (!farmAccountStore.currentAccountId) return;
  automationSaving.value = true;
  try {
    const { error } = await fetchModifyFarmAutomation({
      accountId: farmAccountStore.currentAccountId,
      automation: {
        farm: automation.farm,
        farm_push: automation.farm_push,
        land_upgrade: automation.land_upgrade,
        friend: automation.friend,
        friend_steal: automation.friend_steal,
        friend_help: automation.friend_help,
        friend_bad: automation.friend_bad,
        friend_help_exp_limit: automation.friend_help_exp_limit,
        task: automation.task,
        sell: automation.sell,
        fertilizer: automation.fertilizer,
        fertilizer_gift: automation.fertilizer_gift,
        fertilizer_buy_organic: automation.fertilizer_buy_organic,
        fertilizer_buy_normal: automation.fertilizer_buy_normal,
        fertilizer_multi_season: automation.fertilizer_multi_season,
        fertilizer_land_types: [...(automation.fertilizer_land_types || [])],
        fertilizer_smart_seconds: automation.fertilizer_smart_seconds,
        skip_own_weed_bug: automation.skip_own_weed_bug
      },
      fertilizerBuyOrganicCount: fertilizerBuy.organicCount,
      fertilizerBuyOrganicThresholdHours: fertilizerBuy.organicThresholdHours,
      fertilizerBuyNormalCount: fertilizerBuy.normalCount,
      fertilizerBuyNormalThresholdHours: fertilizerBuy.normalThresholdHours,
      fertilizerBuyCheckIntervalMinutes: fertilizerBuy.checkIntervalMinutes
    });
    if (!error) {
      window.$message?.success($t('page.farm.settings.saveAutomationSuccess'));
      if (automation.fertilizer_buy_organic || automation.fertilizer_buy_normal) {
        const check = await fetchFertilizerCheckAndBuy(farmAccountStore.currentAccountId);
        if (check.error) {
          window.$message?.warning($t('page.farm.settings.fertilizerCheckFailed'));
        }
      }
    }
  } finally {
    automationSaving.value = false;
  }
}

async function loadDevicePresets() {
  const { error, data } = await fetchGetDevicePresets();
  if (!error && Array.isArray(data)) {
    devicePresets.value = data as typeof devicePresets.value;
  }
}

async function loadSystemConfig() {
  systemConfigLoading.value = true;
  try {
    const { error, data } = await fetchGetSystemConfig();
    if (!error && data) {
      defaultSystemConfig.value = { ...data.default };
      localSystemConfig.value = { ...(data.saved || data.default) };
    }
  } finally {
    systemConfigLoading.value = false;
  }
}

function applyDevicePreset(presetId: string) {
  const preset = devicePresets.value.find(item => item.id === presetId);
  if (!preset) return;
  const deviceInfo = {
    ...createDefaultSystemConfig().deviceInfo,
    ...(preset.deviceInfo || {})
  } as SystemConfigPayload['deviceInfo'];
  localSystemConfig.value = {
    ...localSystemConfig.value,
    os: deviceInfo.os || 'Windows',
    clientVersion: deviceInfo.clientVersion || '',
    deviceInfo
  };
  selectedPresetId.value = presetId;
}

async function handleSaveSystemConfig() {
  systemConfigSaving.value = true;
  try {
    localSystemConfig.value.clientVersion = localSystemConfig.value.deviceInfo.clientVersion;
    localSystemConfig.value.os = localSystemConfig.value.deviceInfo.os;
    const { error } = await fetchSetSystemConfig(localSystemConfig.value);
    if (!error) {
      window.$message?.success($t('page.farm.settings.saveSystemConfigSuccess'));
    }
  } finally {
    systemConfigSaving.value = false;
  }
}

async function handleResetSystemConfig() {
  systemConfigSaving.value = true;
  try {
    const { error, data } = await fetchResetSystemConfig();
    if (!error && data) {
      defaultSystemConfig.value = { ...data.default };
      localSystemConfig.value = { ...(data.saved || data.default) };
      selectedPresetId.value = '';
      window.$message?.success($t('page.farm.settings.resetSystemConfigSuccess'));
    }
  } finally {
    systemConfigSaving.value = false;
  }
}

function applyOffline(data: Api.Farm.OfflineReminder) {
  offline.provider = data.provider || 'none';
  offline.qqBot.appId = data.qqBot?.appId || '';
  offline.qqBot.clientSecret = data.qqBot?.clientSecret || '';
  offline.qqBotBinding = {
    userOpenid: data.qqBotBinding?.userOpenid || '',
    boundAt: Number(data.qqBotBinding?.boundAt || 0),
    nickname: data.qqBotBinding?.nickname || ''
  };
  offline.title = data.title || '';
  offline.msg = data.msg || '';
  offline.offlineDeleteSec = Number(data.offlineDeleteSec || 0);
}

async function loadQqBotBindStatus() {
  const { error, data } = await fetchGetQqBotBindStatus();
  if (error || !data) return;
  qqBotBindStatus.credentialsConfigured = data.credentialsConfigured;
  qqBotBindStatus.bound = data.bound;
  qqBotBindStatus.binding = data.binding;
  qqBotBindStatus.botInviteUrl = data.botInviteUrl;
  if (data.bound && data.binding?.userOpenid) {
    offline.qqBotBinding = { ...data.binding };
    if (offline.provider === 'none') offline.provider = 'qq_bot';
  }
}

function stopQqBotBindPolling() {
  if (qqBotBindPollTimer) {
    clearInterval(qqBotBindPollTimer);
    qqBotBindPollTimer = null;
  }
  qqBotBindPolling.value = false;
}

async function pollQqBotBindOnce() {
  if (!qqBotBindSessionId.value) return;
  const { error, data } = await fetchPollQqBotBind(qqBotBindSessionId.value);
  if (error || !data) return;
    if (data.status === 'bound' && data.binding?.userOpenid) {
    stopQqBotBindPolling();
    offline.provider = 'qq_bot';
    offline.qqBotBinding = { ...data.binding };
    qqBotBindStatus.bound = true;
    qqBotBindStatus.binding = { ...data.binding };
    qqBotBindSessionId.value = '';
    qqBotBindQrDataUrl.value = '';
    window.$message?.success($t('page.farm.settings.qqBotBindSuccess'));
    const { error, data: reminder } = await fetchGetOfflineReminder();
    if (!error && reminder) applyOffline(reminder);
    await loadQqBotBindStatus();
    return;
  }
  if (data.status === 'expired') {
    stopQqBotBindPolling();
    qqBotBindSessionId.value = '';
    qqBotBindQrDataUrl.value = '';
    window.$message?.warning($t('page.farm.settings.qqBotBindExpired'));
  }
}

function startQqBotBindPolling() {
  stopQqBotBindPolling();
  qqBotBindPolling.value = true;
  void pollQqBotBindOnce();
  qqBotBindPollTimer = setInterval(() => {
    void pollQqBotBindOnce();
  }, 2000);
}

async function handleStartQqBotBind() {
  if (qqBotBindPolling.value || qqBotBindLoading.value) return;
  if (!qqBotCredentialsReady.value) {
    window.$message?.warning($t('page.farm.settings.qqBotCredentialsMissing'));
    return;
  }
  qqBotBindLoading.value = true;
  try {
    const { error: saveError } = await fetchSaveOfflineReminder(offlinePayload());
    if (saveError) return;
    const { error, data } = await fetchStartQqBotBind();
    if (error || !data?.sessionId) return;
    offline.provider = 'qq_bot';
    qqBotBindSessionId.value = data.sessionId;
    qqBotBindQrDataUrl.value = data.qrDataUrl || '';
    qqBotBindStatus.botInviteUrl = data.botInviteUrl || qqBotBindStatus.botInviteUrl;
    startQqBotBindPolling();
  } finally {
    qqBotBindLoading.value = false;
  }
}

async function handleUnbindQqBot() {
  qqBotBindLoading.value = true;
  try {
    stopQqBotBindPolling();
    const { error, data } = await fetchUnbindQqBot();
    if (!error && data) applyOffline(data);
    qqBotBindStatus.bound = false;
    qqBotBindStatus.binding = { userOpenid: '' };
    qqBotBindSessionId.value = '';
    qqBotBindQrDataUrl.value = '';
    window.$message?.success($t('page.farm.settings.qqBotUnbindSuccess'));
  } finally {
    qqBotBindLoading.value = false;
  }
}

async function openBotInvite() {
  const url = qqBotBindStatus.botInviteUrl;
  if (!url) return;
  try {
    await openUrl(url);
  } catch (error) {
    window.$message?.error(`${$t('page.farm.settings.botDocsOpenFail')}: ${String(error)}`);
  }
}

async function loadOffline() {
  const { error, data } = await fetchGetOfflineReminder();
  if (!error && data) {
    applyOffline(data);
  }
  await loadQqBotBindStatus();
}

function offlinePayload(): Api.Farm.OfflineReminder {
  return {
    provider: offline.provider || 'none',
    qqBot: {
      appId: offline.qqBot.appId || '',
      clientSecret: offline.qqBot.clientSecret || ''
    },
    qqBotBinding: {
      userOpenid: offline.qqBotBinding.userOpenid || '',
      boundAt: Number(offline.qqBotBinding.boundAt || 0),
      nickname: offline.qqBotBinding.nickname || ''
    },
    wechatBot: {},
    title: offline.title || '',
    msg: offline.msg || '',
    offlineDeleteSec: Number(offline.offlineDeleteSec || 0)
  };
}

async function handleSaveOffline() {
  offlineSaving.value = true;
  try {
    const { error, data } = await fetchSaveOfflineReminder(offlinePayload());
    if (!error) {
      if (data) applyOffline(data);
      window.$message?.success($t('page.farm.settings.saveOfflineSuccess'));
    }
  } finally {
    offlineSaving.value = false;
  }
}

async function handleTestOffline() {
  offlineTesting.value = true;
  try {
    const { error, data } = await fetchTestOfflineReminder(offlinePayload());
    if (error) return;
    if (data?.ok) {
      window.$message?.success($t('page.farm.settings.testOfflineSuccess'));
    } else {
      window.$message?.error(`${$t('page.farm.settings.testOfflineFail')}: ${data?.msg || 'unknown'}`);
    }
  } finally {
    offlineTesting.value = false;
  }
}

async function openProviderDocs() {
  const url = currentProviderDocUrl.value;
  if (!url) return;
  try {
    await openUrl(url);
  } catch (error) {
    window.$message?.error(`${$t('page.farm.settings.botDocsOpenFail')}: ${String(error)}`);
  }
}

watch(
  () => [plantingStrategy.value, farmAccountStore.currentAccountId] as const,
  ([strategy, accountId], previous) => {
    if (strategy === 'bag_priority' && accountId && (previous?.[0] !== strategy || previous?.[1] !== accountId)) {
      void fetchBagSeeds(accountId);
    }
  }
);

watch(
  () =>
    [
      plantingStrategy.value,
      bagSeedFallbackStrategy.value,
      preferredSeedId.value,
      seedOptions.value,
      farmAccountStore.currentAccountId
    ] as const,
  async ([strategyValue, fallbackStrategy, preferredId, currentSeeds, accountId]) => {
    const requestRevision = ++previewRequestRevision;
    let strategy = strategyValue;
    if (strategy === 'preferred') {
      strategyPreviewLabel.value = null;
      return;
    }
    if (strategy === 'bag_priority') {
      strategy = fallbackStrategy || 'level';
      if (strategy === 'preferred') {
        const seed =
          preferredId && preferredId > 0 ? currentSeeds.find(item => item.seedId === preferredId) : undefined;
        strategyPreviewLabel.value = seed
          ? `Lv${seed.requiredLevel} ${seed.name}`
          : $t('page.farm.settings.strategyPreviewNoPreferred');
        return;
      }
    }
    if (currentSeeds.length === 0) {
      strategyPreviewLabel.value = null;
      return;
    }
    const available = currentSeeds.filter(seed => !seed.locked && !seed.soldOut);
    if (available.length === 0) {
      strategyPreviewLabel.value = $t('page.farm.settings.strategyPreviewEmpty');
      return;
    }
    if (strategy === 'level') {
      const best = [...available].sort((a, b) => b.requiredLevel - a.requiredLevel || a.seedId - b.seedId)[0];
      strategyPreviewLabel.value = best ? `Lv${best.requiredLevel} ${best.name}` : null;
      return;
    }
    const sortBy = analyticsSortByMap[strategy];
    if (!sortBy) return;

    try {
      const { error, data } = await fetchGetFarmAnalyticsDetail({
        accountId: accountId || undefined,
        sort: sortBy
      });
      if (requestRevision !== previewRequestRevision || accountId !== farmAccountStore.currentAccountId) return;
      const rankings = !error && data?.rankings ? data.rankings : [];
      const availableIds = new Set(available.map((seed: any) => seed.seedId));
      const match = rankings.find((item: any) => availableIds.has(Number(item.seedId)));
      const seed = match ? available.find((item: any) => item.seedId === Number(match.seedId)) : undefined;
      strategyPreviewLabel.value = seed
        ? `Lv${seed.requiredLevel} ${seed.name}`
        : $t('page.farm.settings.strategyPreviewNoMatch');
    } catch {
      if (requestRevision === previewRequestRevision && accountId === farmAccountStore.currentAccountId) {
        strategyPreviewLabel.value = null;
      }
    }
  },
  { immediate: true }
);

watch(
  () => farmAccountStore.currentAccountId,
  () => {
    void loadConfig();
  }
);

onMounted(async () => {
  if (!farmAccountStore.accounts.length) {
    await farmAccountStore.loadAccounts();
  }
  await Promise.all([loadConfig(), loadOffline(), loadDevicePresets(), loadSystemConfig()]);
});

onUnmounted(() => {
  stopQqBotBindPolling();
});
</script>

<template>
  <div class="min-h-500px flex-col-stretch gap-16px overflow-auto">
    <NTabs v-model:value="activeTab" type="line" animated>
      <NTabPane name="strategy" :tab="$t('page.farm.settings.strategy')">
        <NEmpty
          v-if="!farmAccountStore.currentAccountId"
          class="py-48px"
          :description="$t('page.farm.common.selectAccount')"
        />
        <NCard v-else :bordered="false" size="small" class="card-wrapper">
          <NForm label-placement="left" :label-width="140">
            <div class="grid gap-12px md:grid-cols-2">
              <NFormItem :label="$t('page.farm.settings.plantingStrategy')">
                <NSelect v-model:value="plantingStrategy" class="w-full" :options="strategyOptions" />
              </NFormItem>
              <NFormItem v-if="showPreferredSeedSelect" :label="$t('page.farm.settings.preferredSeedId')">
                <NSelect
                  :value="preferredSeedId ?? 0"
                  class="w-full"
                  filterable
                  :options="preferredSeedSelectOptions"
                  @update:value="handlePreferredSeedChange"
                />
              </NFormItem>
              <NFormItem
                v-else
                :label="
                  plantingStrategy === 'bag_priority'
                    ? $t('page.farm.settings.strategyFallbackPreview')
                    : $t('page.farm.settings.strategyPreview')
                "
              >
                <div
                  class="w-full flex items-center justify-between rounded-6px border border-[var(--n-border-color)] bg-[var(--n-color)] px-12px py-6px text-13px text-[var(--n-text-color-3)]"
                >
                  <span class="truncate">
                    {{ strategyPreviewLabel ?? $t('page.farm.settings.strategyPreviewLoading') }}
                  </span>
                </div>
              </NFormItem>
            </div>

            <template v-if="plantingStrategy === 'bag_priority'">
              <NFormItem :label="$t('page.farm.settings.bagSeedFallback')">
                <NSelect
                  v-model:value="bagSeedFallbackStrategy"
                  class="max-w-320px"
                  :options="bagFallbackStrategyOptions"
                  @update:value="handleBagFallbackStrategyChange"
                />
              </NFormItem>

              <div
                class="mb-16px rounded-8px border border-[var(--n-border-color)] bg-[var(--n-color-embedded)] p-12px"
              >
                <div class="mb-12px flex flex-wrap items-start justify-between gap-12px">
                  <div class="min-w-0 flex-1">
                    <div class="mb-4px text-14px font-medium">{{ $t('page.farm.settings.bagSeedOrder') }}</div>
                    <NText depth="3" class="text-12px leading-18px">
                      {{ $t('page.farm.settings.bagSeedOrderHint') }}
                    </NText>
                  </div>
                  <NButton size="tiny" secondary @click="resetBagSeedPriority">
                    {{ $t('page.farm.settings.bagSeedOrderReset') }}
                  </NButton>
                </div>

                <div v-if="bagSeedsLoading" class="py-16px text-center text-13px text-[var(--n-text-color-3)]">
                  {{ $t('page.farm.settings.bagSeedOrderLoading') }}
                </div>
                <div v-else-if="bagSeedsError" class="py-16px text-center text-13px text-[var(--n-error-color)]">
                  {{ bagSeedsError }}
                </div>
                <div
                  v-else-if="bagSeeds.length === 0"
                  class="py-16px text-center text-13px text-[var(--n-text-color-3)]"
                >
                  {{ $t('page.farm.settings.bagSeedOrderEmpty') }}
                </div>
                <div v-else class="grid gap-8px sm:grid-cols-2 xl:grid-cols-3">
                  <div
                    v-for="(seed, index) in sortedBagSeeds"
                    :key="seed.seedId"
                    class="bag-seed-item flex items-center gap-8px rounded-8px border border-[var(--n-border-color)] bg-[var(--n-color)] px-10px py-8px"
                    :class="{ 'bag-seed-item--dragging': draggingBagSeedId === seed.seedId }"
                    draggable="true"
                    @dragstart="startBagSeedDrag(seed.seedId, $event)"
                    @dragend="endBagSeedDrag"
                    @dragover.prevent="dragOverBagSeed(seed.seedId, $event)"
                    @drop="dropBagSeed(seed.seedId, $event)"
                  >
                    <div
                      class="h-28px w-28px flex shrink-0 items-center justify-center rounded-6px bg-[var(--n-color-embedded)] text-12px font-semibold"
                    >
                      {{ index + 1 }}
                    </div>
                    <div class="min-w-0 flex-1">
                      <div class="flex items-center gap-6px">
                        <span class="truncate text-13px font-medium">{{ seed.name }}</span>
                        <NTag size="tiny" :bordered="false">{{ seed.plantSize }}x{{ seed.plantSize }}</NTag>
                      </div>
                      <div class="mt-2px text-12px text-[var(--n-text-color-3)]">
                        {{ $t('page.farm.settings.bagSeedStock') }} {{ seed.count }} · Lv{{ seed.requiredLevel }} · ID
                        {{ seed.seedId }}
                      </div>
                    </div>
                    <div class="flex shrink-0 flex-col gap-2px">
                      <NButton size="tiny" quaternary :disabled="index === 0" @click="moveBagSeed(seed.seedId, -1)">
                        ▲
                      </NButton>
                      <NButton
                        size="tiny"
                        quaternary
                        :disabled="index === sortedBagSeeds.length - 1"
                        @click="moveBagSeed(seed.seedId, 1)"
                      >
                        ▼
                      </NButton>
                    </div>
                  </div>
                </div>
              </div>
            </template>

            <NDivider title-placement="left">{{ $t('page.farm.settings.intervals') }}</NDivider>
            <div class="grid max-w-640px gap-12px sm:grid-cols-2">
              <NFormItem :label="$t('page.farm.settings.farmMin')">
                <NInputNumber v-model:value="intervals.farmMin" class="w-full" :min="1" />
              </NFormItem>
              <NFormItem :label="$t('page.farm.settings.farmMax')">
                <NInputNumber v-model:value="intervals.farmMax" class="w-full" :min="1" />
              </NFormItem>
              <NFormItem :label="$t('page.farm.settings.helpMin')">
                <NInputNumber v-model:value="intervals.helpMin" class="w-full" :min="1" />
              </NFormItem>
              <NFormItem :label="$t('page.farm.settings.helpMax')">
                <NInputNumber v-model:value="intervals.helpMax" class="w-full" :min="1" />
              </NFormItem>
              <NFormItem :label="$t('page.farm.settings.stealMin')">
                <NInputNumber v-model:value="intervals.stealMin" class="w-full" :min="1" />
              </NFormItem>
              <NFormItem :label="$t('page.farm.settings.stealMax')">
                <NInputNumber v-model:value="intervals.stealMax" class="w-full" :min="1" />
              </NFormItem>
            </div>

            <NDivider title-placement="left">{{ $t('page.farm.settings.quietHours') }}</NDivider>
            <div class="grid gap-12px sm:grid-cols-2 md:grid-cols-3">
              <NFormItem :label="$t('page.farm.settings.quietHoursEnable')">
                <NSwitch v-model:value="quietHours.enabled" />
              </NFormItem>
              <NFormItem :label="$t('page.farm.settings.quietStart')">
                <NTimePicker
                  v-model:formatted-value="quietHours.start"
                  class="w-full"
                  format="HH:mm"
                  value-format="HH:mm"
                  :clearable="false"
                  :disabled="!quietHours.enabled"
                />
              </NFormItem>
              <NFormItem :label="$t('page.farm.settings.quietEnd')">
                <NTimePicker
                  v-model:formatted-value="quietHours.end"
                  class="w-full"
                  format="HH:mm"
                  value-format="HH:mm"
                  :clearable="false"
                  :disabled="!quietHours.enabled"
                />
              </NFormItem>
            </div>

            <NDivider title-placement="left">{{ $t('page.farm.settings.plantDelaySection') }}</NDivider>
            <div class="grid gap-12px sm:grid-cols-2 md:grid-cols-3">
              <NFormItem :label="$t('page.farm.settings.plantOrderRandom')">
                <NSwitch v-model:value="plantOrderRandom" />
              </NFormItem>
              <NFormItem :label="$t('page.farm.settings.plantDelaySeconds')">
                <NInputNumber v-model:value="plantDelaySeconds" class="w-full" :min="0" :max="60" />
              </NFormItem>
              <NFormItem :label="$t('page.farm.settings.stealDelaySeconds')">
                <NInputNumber v-model:value="stealDelaySeconds" class="w-full" :min="0" :max="60" />
              </NFormItem>
            </div>
          </NForm>

          <div class="mt-16px flex justify-end border-t border-[var(--n-border-color)] pt-16px">
            <NButton
              type="primary"
              size="small"
              :loading="strategySaving || loading"
              @click="handleSaveStrategy"
            >
              {{ $t('page.farm.settings.saveStrategy') }}
            </NButton>
          </div>
        </NCard>
      </NTabPane>

      <NTabPane name="automation" :tab="$t('page.farm.settings.automation')">
        <NEmpty
          v-if="!farmAccountStore.currentAccountId"
          class="py-48px"
          :description="$t('page.farm.common.selectAccount')"
        />
        <NCard v-else :bordered="false" size="small" class="card-wrapper">
          <div class="auto-switch-grid">
            <div class="auto-switch-item">
              <NSwitch v-model:value="automation.farm" />
              <span>{{ $t('page.farm.settings.farm') }}</span>
            </div>
            <div class="auto-switch-item">
              <NSwitch v-model:value="automation.task" />
              <span>{{ $t('page.farm.settings.task') }}</span>
            </div>
            <div class="auto-switch-item">
              <NSwitch v-model:value="automation.sell" />
              <span>{{ $t('page.farm.settings.sell') }}</span>
            </div>
            <div class="auto-switch-item">
              <NSwitch v-model:value="automation.friend" />
              <span>{{ $t('page.farm.settings.friend') }}</span>
            </div>
            <div class="auto-switch-item" :class="{ 'is-disabled': !automation.farm }">
              <NSwitch v-model:value="automation.farm_push" :disabled="!automation.farm" />
              <span>{{ $t('page.farm.settings.farmPush') }}</span>
            </div>
            <div class="auto-switch-item">
              <NSwitch v-model:value="automation.land_upgrade" />
              <span>{{ $t('page.farm.settings.landUpgrade') }}</span>
            </div>
            <div class="auto-switch-item">
              <NSwitch v-model:value="automation.fertilizer_gift" />
              <span>{{ $t('page.farm.settings.fertilizerGift') }}</span>
            </div>
            <div class="auto-switch-item">
              <NSwitch v-model:value="automation.fertilizer_buy_organic" />
              <span>{{ $t('page.farm.settings.fertilizerBuyOrganic') }}</span>
            </div>
            <div class="auto-switch-item">
              <NSwitch v-model:value="automation.fertilizer_buy_normal" />
              <span>{{ $t('page.farm.settings.fertilizerBuyNormal') }}</span>
            </div>
            <div class="auto-switch-item">
              <NSwitch v-model:value="automation.skip_own_weed_bug" />
              <span>{{ $t('page.farm.settings.skipOwnWeedBug') }}</span>
            </div>
          </div>

          <template v-if="showFertilizerBuyPanel">
            <NDivider title-placement="left">{{ $t('page.farm.settings.fertilizerBuy') }}</NDivider>
            <NForm label-placement="left" :label-width="140">
              <div v-if="automation.fertilizer_buy_organic" class="mb-12px grid gap-12px sm:grid-cols-2 md:grid-cols-3">
                <div class="sm:col-span-2 md:col-span-3 text-sm font-medium">
                  {{ $t('page.farm.settings.fertilizerBuyOrganicTitle') }}
                </div>
                <NFormItem :label="$t('page.farm.settings.fertilizerBuyOrganicCount')">
                  <NInputNumber v-model:value="fertilizerBuy.organicCount" class="w-full" :min="1" :max="10000" />
                </NFormItem>
                <NFormItem :label="$t('page.farm.settings.fertilizerBuyOrganicThreshold')">
                  <NInputNumber
                    v-model:value="fertilizerBuy.organicThresholdHours"
                    class="w-full"
                    :min="1"
                    :max="990"
                  />
                </NFormItem>
              </div>
              <div v-if="automation.fertilizer_buy_normal" class="mb-12px grid gap-12px sm:grid-cols-2 md:grid-cols-3">
                <div class="sm:col-span-2 md:col-span-3 text-sm font-medium">
                  {{ $t('page.farm.settings.fertilizerBuyNormalTitle') }}
                </div>
                <NFormItem :label="$t('page.farm.settings.fertilizerBuyNormalCount')">
                  <NInputNumber v-model:value="fertilizerBuy.normalCount" class="w-full" :min="1" :max="10000" />
                </NFormItem>
                <NFormItem :label="$t('page.farm.settings.fertilizerBuyNormalThreshold')">
                  <NInputNumber v-model:value="fertilizerBuy.normalThresholdHours" class="w-full" :min="1" :max="990" />
                </NFormItem>
              </div>
              <div class="grid gap-12px sm:grid-cols-2 md:grid-cols-3">
                <NFormItem :label="$t('page.farm.settings.fertilizerBuyCheckInterval')">
                  <NInputNumber
                    v-model:value="fertilizerBuy.checkIntervalMinutes"
                    class="w-full"
                    :min="1"
                    :max="1440"
                  />
                </NFormItem>
              </div>
              <NText depth="3" class="text-12px">{{ $t('page.farm.settings.fertilizerBuyHint') }}</NText>
            </NForm>
          </template>

          <template v-if="automation.friend">
            <NDivider title-placement="left">{{ $t('page.farm.settings.friend') }}</NDivider>
            <div class="auto-switch-grid">
              <div class="auto-switch-item">
                <NSwitch v-model:value="automation.friend_steal" />
                <span>{{ $t('page.farm.settings.friendSteal') }}</span>
              </div>
              <div class="auto-switch-item">
                <NSwitch v-model:value="automation.friend_help" />
                <span>{{ $t('page.farm.settings.friendHelp') }}</span>
              </div>
              <div class="auto-switch-item">
                <NSwitch v-model:value="automation.friend_bad" />
                <span>{{ $t('page.farm.settings.friendBad') }}</span>
              </div>
              <div class="auto-switch-item">
                <NSwitch v-model:value="automation.friend_help_exp_limit" />
                <span>{{ $t('page.farm.settings.friendHelpExpLimit') }}</span>
              </div>
            </div>
          </template>

          <NDivider title-placement="left">{{ $t('page.farm.settings.fertilizer') }}</NDivider>
          <div class="fertilizer-panel">
            <div class="fertilizer-field">
              <div class="fertilizer-label">{{ $t('page.farm.settings.fertilizerLandTypes') }}</div>
              <NSelect
                v-model:value="automation.fertilizer_land_types"
                class="max-w-480px"
                multiple
                :options="fertilizerLandTypeOptions"
              />
              <NText depth="3" class="mt-8px text-12px">{{ $t('page.farm.settings.fertilizerLandTypesHint') }}</NText>
            </div>
            <div class="fertilizer-field">
              <div class="fertilizer-label">{{ $t('page.farm.settings.fertilizer') }}</div>
              <NSelect v-model:value="automation.fertilizer" class="max-w-320px" :options="fertilizerOptions" />
            </div>
            <div class="auto-switch-item fertilizer-field">
              <NSwitch v-model:value="automation.fertilizer_multi_season" />
              <span>{{ $t('page.farm.settings.fertilizerMultiSeason') }}</span>
            </div>
            <div v-if="showSmartSeconds" class="fertilizer-field flex flex-wrap items-end gap-12px">
              <div>
                <div class="fertilizer-label">{{ $t('page.farm.settings.fertilizerSmartSeconds') }}</div>
                <NInputNumber
                  v-model:value="automation.fertilizer_smart_seconds"
                  class="w-160px"
                  :min="30"
                  :max="3600"
                  :step="30"
                />
              </div>
              <NText depth="3" class="pb-8px text-12px">
                {{ $t('page.farm.settings.fertilizerSmartSecondsHint') }}
              </NText>
            </div>
          </div>

          <div class="mt-16px flex justify-end border-t border-[var(--n-border-color)] pt-16px">
            <NButton
              type="primary"
              size="small"
              :loading="automationSaving || loading"
              @click="handleSaveAutomation"
            >
              {{ $t('page.farm.settings.saveAutomation') }}
            </NButton>
          </div>
        </NCard>
      </NTabPane>

      <NTabPane name="offline" :tab="$t('page.farm.settings.offlineReminder')">
        <NCard :bordered="false" size="small" class="card-wrapper">
          <NText depth="3" class="mb-16px block text-12px">{{ $t('page.farm.settings.offlineHint') }}</NText>
          <NForm label-placement="left" :label-width="140">
            <div class="grid max-w-640px gap-12px">
              <NFormItem :label="$t('page.farm.settings.provider')">
                <div class="flex w-full gap-8px">
                  <NSelect v-model:value="offline.provider" class="flex-1" :options="providerOptions" />
                  <NButton
                    :disabled="!currentProviderDocUrl"
                    @click="openProviderDocs"
                  >
                    {{ $t('page.farm.settings.botDocs') }}
                  </NButton>
                </div>
              </NFormItem>
              <template v-if="offline.provider === 'qq_bot'">
                <NFormItem :label="$t('page.farm.settings.qqBotAppId')">
                  <NInput v-model:value="offline.qqBot.appId" />
                </NFormItem>
                <NFormItem :label="$t('page.farm.settings.qqBotClientSecret')">
                  <NInput
                    v-model:value="offline.qqBot.clientSecret"
                    type="password"
                    show-password-on="click"
                  />
                </NFormItem>
                <NText depth="3" class="text-12px">{{ $t('page.farm.settings.qqBotHint') }}</NText>
                <NFormItem :label="$t('page.farm.settings.qqBotBindStatus')">
                  <div class="flex w-full flex-col gap-12px">
                    <NTag :type="qqBotBindStatus.bound || offline.qqBotBinding.userOpenid ? 'success' : 'default'">
                      {{ qqBotBindStateLabel }}
                    </NTag>
                    <NText v-if="offline.qqBotBinding.nickname" depth="3" class="text-12px">
                      {{ offline.qqBotBinding.nickname }}
                    </NText>
                    <div v-if="qqBotBindQrDataUrl" class="flex flex-col items-start gap-8px">
                      <img :src="qqBotBindQrDataUrl" alt="qq-bot-bind-qr" class="h-180px w-180px rounded-8px border border-[var(--n-border-color)]" />
                      <NText depth="3" class="text-12px">{{ $t('page.farm.settings.qqBotBindScanHint') }}</NText>
                    </div>
                    <NText v-else-if="qqBotBindPolling" depth="3" class="text-12px">
                      {{ $t('page.farm.settings.qqBotBindManualHint') }}
                    </NText>
                    <div class="flex flex-wrap gap-8px">
                      <NButton
                        type="primary"
                        :loading="qqBotBindLoading"
                        :disabled="!qqBotCredentialsReady || qqBotBindPolling"
                        @click="handleStartQqBotBind"
                      >
                        {{ qqBotBindPolling ? $t('page.farm.settings.qqBotBindWaiting') : $t('page.farm.settings.qqBotBindStart') }}
                      </NButton>
                      <NButton
                        :disabled="!qqBotBindStatus.botInviteUrl"
                        @click="openBotInvite"
                      >
                        {{ $t('page.farm.settings.qqBotOpenBot') }}
                      </NButton>
                      <NButton
                        :disabled="!qqBotBindStatus.bound && !offline.qqBotBinding.userOpenid"
                        @click="handleUnbindQqBot"
                      >
                        {{ $t('page.farm.settings.qqBotUnbind') }}
                      </NButton>
                    </div>
                  </div>
                </NFormItem>
              </template>
            </div>
          </NForm>
          <div class="mt-16px flex justify-end gap-8px border-t border-[var(--n-border-color)] pt-16px">
            <NButton
              size="small"
              :loading="offlineTesting"
              :disabled="offlineSaving || offline.provider !== 'qq_bot' || (!qqBotBindStatus.bound && !offline.qqBotBinding.userOpenid)"
              @click="handleTestOffline"
            >
              {{ $t('page.farm.settings.testOffline') }}
            </NButton>
            <NButton
              type="primary"
              size="small"
              :loading="offlineSaving"
              :disabled="offlineTesting"
              @click="handleSaveOffline"
            >
              {{ $t('page.farm.settings.saveOffline') }}
            </NButton>
          </div>
        </NCard>
      </NTabPane>

      <NTabPane name="system" :tab="$t('page.farm.settings.system')">
        <NCard :bordered="false" size="small" class="card-wrapper">
          <template #header>
            <div class="text-16px font-medium">{{ $t('page.farm.settings.runtimeEnv') }}</div>
          </template>

          <NSpin :show="systemConfigLoading">
            <div class="flex-col gap-16px">
              <div v-if="devicePresets.length">
                <div class="mb-8px text-13px">{{ $t('page.farm.settings.devicePresets') }}</div>
                <NSpace wrap>
                  <NButton
                    v-for="preset in devicePresets"
                    :key="preset.id"
                    size="small"
                    :type="selectedPresetId === preset.id ? 'primary' : 'default'"
                    :title="preset.description"
                    @click="applyDevicePreset(preset.id)"
                  >
                    {{ preset.name }}
                  </NButton>
                </NSpace>
              </div>

              <NForm label-placement="top" label-width="auto">
                <NFormItem :label="$t('page.farm.settings.serverUrl')">
                  <NInput v-model:value="localSystemConfig.serverUrl" placeholder="wss://..." />
                </NFormItem>

                <div class="grid gap-16px md:grid-cols-2">
                  <NFormItem :label="$t('page.farm.settings.platform')">
                    <NSpace wrap>
                      <NButton
                        v-for="option in platformOptions"
                        :key="option.value"
                        size="small"
                        :type="localSystemConfig.platform === option.value ? 'primary' : 'default'"
                        @click="localSystemConfig.platform = option.value"
                      >
                        {{ option.label }}
                      </NButton>
                    </NSpace>
                  </NFormItem>

                  <NFormItem :label="$t('page.farm.settings.deviceOs')">
                    <NSpace wrap>
                      <NButton
                        v-for="option in osOptions"
                        :key="option.value"
                        size="small"
                        :type="localSystemConfig.deviceInfo.os === option.value ? 'primary' : 'default'"
                        @click="
                          localSystemConfig.deviceInfo.os = option.value;
                          localSystemConfig.os = option.value;
                        "
                      >
                        {{ option.label }}
                      </NButton>
                    </NSpace>
                  </NFormItem>
                </div>

                <div class="grid gap-16px md:grid-cols-2">
                  <NFormItem :label="$t('page.farm.settings.clientVersion')">
                    <NInput v-model:value="localSystemConfig.deviceInfo.clientVersion" />
                  </NFormItem>
                  <NFormItem :label="$t('page.farm.settings.sysSoftware')">
                    <NInput v-model:value="localSystemConfig.deviceInfo.sysSoftware" />
                  </NFormItem>
                  <NFormItem :label="$t('page.farm.settings.deviceId')">
                    <NInput v-model:value="localSystemConfig.deviceInfo.deviceId" />
                  </NFormItem>
                  <NFormItem :label="$t('page.farm.settings.memory')">
                    <NInput v-model:value="localSystemConfig.deviceInfo.memory" />
                  </NFormItem>
                  <NFormItem :label="$t('page.farm.settings.network')">
                    <NInput v-model:value="localSystemConfig.deviceInfo.network" />
                  </NFormItem>
                </div>

                <NFormItem :label="$t('page.farm.settings.userAgent')">
                  <NInput v-model:value="localSystemConfig.deviceInfo.userAgent" type="textarea" :rows="3" />
                </NFormItem>
              </NForm>

              <NSpace>
                <NButton type="primary" :loading="systemConfigSaving" @click="handleSaveSystemConfig">
                  {{ $t('page.farm.settings.saveRuntimeEnv') }}
                </NButton>
                <NButton :loading="systemConfigSaving" @click="handleResetSystemConfig">
                  {{ $t('page.farm.settings.resetRuntimeEnv') }}
                </NButton>
              </NSpace>
            </div>
          </NSpin>
        </NCard>
      </NTabPane>
    </NTabs>
  </div>
</template>

<style scoped>
.bag-seed-item {
  cursor: grab;
  user-select: none;
  transition:
    opacity 0.15s ease,
    box-shadow 0.15s ease;
}

.bag-seed-item:active {
  cursor: grabbing;
}

.bag-seed-item--dragging {
  opacity: 0.55;
  box-shadow: 0 0 0 1px var(--n-primary-color);
}

.auto-switch-grid {
  display: grid;
  grid-template-columns: repeat(1, minmax(0, 1fr));
  gap: 12px 24px;
}

@media (min-width: 640px) {
  .auto-switch-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (min-width: 1024px) {
  .auto-switch-grid {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}

.auto-switch-item {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  min-height: 28px;
  margin: 0;
  user-select: none;
}

.auto-switch-item span {
  font-size: 13px;
  line-height: 1.4;
  color: var(--n-text-color);
}

.auto-switch-item.is-disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.fertilizer-panel {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.fertilizer-field {
  max-width: 480px;
}

.fertilizer-label {
  margin-bottom: 8px;
  font-size: 13px;
  line-height: 1.4;
}
</style>
