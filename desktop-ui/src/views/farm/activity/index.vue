<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import {
  NButton,
  NCard,
  NCollapse,
  NCollapseItem,
  NEmpty,
  NInputNumber,
  NModal,
  NProgress,
  NSpace,
  NSpin,
  NTag,
  useMessage
} from 'naive-ui';
import {
  fetchClaimFarmActivityGreenPlum,
  fetchClaimFarmActivityPass,
  fetchClaimFarmActivityQixiBridge,
  fetchClaimFarmActivitySolarTerm,
  fetchContinueFarmActivityGreenPlumBrew,
  fetchExchangeFarmActivityShop,
  fetchGetFarmActivitySnapshot,
  fetchGetFarmFriendList,
  fetchGiftFarmActivityQixiSachet,
  fetchLightFarmActivityConstellation,
  fetchSettleFarmActivityGreenPlumBrew,
  fetchStartFarmActivityGreenPlumBrew
} from '@/service/api';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { resolveCatalogImage } from '@/views/farm/game-config/shared';
import ActivityRulesDialog from './activity-rules-dialog.vue';
import { normalizeActivityRules } from './rules';
import { $t } from '@/locales';
import QixiView from './qixi-view.vue';

defineOptions({
  name: 'FarmActivity'
});

type ActivityTab = 'travel' | 'constellation' | 'shop' | 'solar';
type GameplayKey = 'stellar' | 'qixi' | 'greenPlum';
type ActivityStatus = 'active' | 'upcoming' | 'ended';
type ActivityDirectoryItem = {
  id?: string;
  name?: string;
  startTime?: number | string;
  endTime?: number | string;
  gameplayKey?: string | null;
  detailTarget?: string | null;
  gameplayTargets?: string[];
};
type RewardItem = {
  id?: string | number;
  name?: string;
  count?: number | string;
  image?: string;
  rarity?: number;
  locked?: boolean;
};
type PassNode = {
  id?: string | number;
  level?: number;
  claimable?: boolean;
  claimed?: boolean;
  current?: boolean;
  locked?: boolean;
  keyLevel?: boolean;
  rewards?: RewardItem[];
};
type ShopGoods = {
  id?: string | number;
  name?: string;
  category?: string;
  categoryId?: string;
  categoryName?: string;
  exchangeable?: boolean;
  soldOut?: boolean;
  owned?: boolean;
  balanceKnown?: boolean;
  maxExchangeCountKnown?: boolean;
  maxExchangeCount?: string | number;
  item?: RewardItem;
  cost?: { count?: string | number; image?: string; name?: string };
};
type ShopCategory = { id: string; name: string };
type SolarTerm = {
  id?: string;
  name?: string;
  title?: string;
  englishName?: string;
  description?: string;
  rewardTitle?: string;
  rewardDescription?: string;
  claimable?: boolean;
  claimed?: boolean;
  canClaim?: boolean;
  locked?: boolean;
  current?: boolean;
  startTime?: number;
  endTime?: number;
  rewards?: RewardItem[];
};
type ConstellationGroup = {
  id?: string;
  name?: string;
  order?: number;
  visualState?: string;
  claimStatus?: string;
  rewards?: RewardItem[];
};
type GreenPlumIngredient = {
  uid?: string;
  key?: string;
  id?: string | number;
  name?: string;
  count?: number | string;
  image?: string;
  mutantTypes?: string[];
};
type GreenPlum = {
  activityId?: string;
  dailyActivityId?: string;
  brewActivityId?: string;
  known?: boolean;
  active?: boolean;
  name?: string;
  typeCode?: number;
  startTime?: number;
  endTime?: number;
  balance?: string;
  balanceKnown?: boolean;
  ingredients?: GreenPlumIngredient[];
  currentRound?: number;
  maxRounds?: number;
  started?: boolean;
  finished?: boolean;
  baseGold?: string;
  basePrice?: string;
  guaranteedPrice?: string;
  quotePrices?: string[];
  quoteTotals?: string[];
  quote?: { round?: number; unitPrice?: string; totalGold?: string; doubled?: boolean } | null;
  dailySeed?: {
    claimed?: boolean;
    grantId?: string;
    reward?: RewardItem;
  };
  actions?: {
    claimSeed?: { enabled?: boolean; available?: boolean };
    start?: { enabled?: boolean; available?: boolean };
    continue?: { enabled?: boolean; available?: boolean };
    settle?: { enabled?: boolean; available?: boolean };
  };
};
type QixiReward = RewardItem & { id?: string | number };
type QixiStage = {
  id?: string;
  stage?: number;
  statusCode?: string;
  completed?: boolean;
  claimed?: boolean;
  claimable?: boolean;
  current?: boolean;
  cost?: QixiReward;
  rewards?: QixiReward[];
};
type Qixi = {
  groupId?: string;
  activityId?: string;
  name?: string;
  title?: string;
  startTime?: string | number;
  endTime?: string | number;
  active?: boolean;
  feather?: QixiReward;
  sachet?: QixiReward;
  receivedSachet?: QixiReward;
  balances?: {
    feather?: string | null;
    sachet?: string | null;
    receivedSachet?: string | null;
    known?: boolean;
  };
  bridge?: {
    currentStage?: number;
    stages?: QixiStage[];
    claimable?: boolean;
  };
  gift?: {
    sentCount?: string;
  };
  actions?: {
    bridge?: { enabled?: boolean; available?: boolean };
    gift?: { enabled?: boolean; available?: boolean };
  };
};

const farmAccountStore = useFarmAccountStore();
const message = useMessage();

const activeTab = ref<ActivityTab>('travel');
const selectedGameplay = ref<GameplayKey | null>(null);
const loading = ref(false);
const pendingKey = ref<string | null>(null);
const season = ref<Record<string, unknown>>({});
const constellation = ref<Record<string, unknown>>({});
const shop = ref<Record<string, unknown>>({});
const solarTerms = ref<Record<string, unknown>>({});
const greenPlum = ref<GreenPlum>({});
const qixi = ref<Qixi>({});
const directory = ref<ActivityDirectoryItem[]>([]);
const qixiFriends = ref<Api.Farm.Friend[]>([]);
const qixiFriendsLoading = ref(false);
const capabilities = ref<Record<string, boolean>>({});
const actions = ref<Record<string, Api.Farm.ActivityAction>>({});
const clockNow = ref(Date.now());
const selectedSolarId = ref('');
const selectedConstellationId = ref('');
const shopCategory = ref('__all__');
const exchangeOpen = ref(false);
const exchangeGoods = ref<ShopGoods | null>(null);
const exchangeCount = ref(1);
const travelRulesOpen = ref(false);
const constellationRulesOpen = ref(false);

let clockTimer: ReturnType<typeof setInterval> | null = null;

const allTabs: Array<{ key: ActivityTab; labelKey: App.I18n.I18nKey }> = [
  { key: 'travel', labelKey: 'page.farm.activity.tabTravel' },
  { key: 'constellation', labelKey: 'page.farm.activity.tabConstellation' },
  { key: 'shop', labelKey: 'page.farm.activity.tabShop' },
  { key: 'solar', labelKey: 'page.farm.activity.tabSolar' }
];
const dateFormatter = new Intl.DateTimeFormat('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit' });

const pass = computed(() => (season.value.pass as Record<string, unknown> | undefined) || {});
const passNodes = computed(() => ((pass.value.nodes as PassNode[]) || []).slice());
const passProgress = computed(() => Number(pass.value.progress ?? 0));
const passProgressMax = computed(() => Number(pass.value.progressMax ?? 0));
const passLevel = computed(() => pass.value.level ?? '--');
const passPercent = computed(() => {
  if (!passProgressMax.value) return 0;
  return Math.min(100, Math.max(0, (passProgress.value / passProgressMax.value) * 100));
});
const hasClaimablePass = computed(() => passNodes.value.some(node => node.claimable));
const passRules = computed(() => normalizeActivityRules(pass.value.rules));
const constellationRules = computed(() => normalizeActivityRules(constellation.value.rules));
const greenPlumRules = computed(() => normalizeActivityRules(greenPlum.value.rules));

const constellationGroups = computed(() => {
  const groups = ((constellation.value.groups as ConstellationGroup[]) || []).slice();
  return groups.sort((a, b) => Number(a.order ?? 999) - Number(b.order ?? 999));
});
const selectedConstellation = computed(
  () => constellationGroups.value.find(g => g.id === selectedConstellationId.value) || null
);
const canLightConstellation = computed(() => {
  const state = selectedConstellation.value?.visualState || '';
  return ['lightable', 'claimableUnknown'].includes(state) && actionEnabled('lightConstellation');
});

const shopGoods = computed(() => (shop.value.goods as ShopGoods[]) || []);
const shopCategories = computed(() => {
  const raw = shop.value.categories;
  if (!Array.isArray(raw)) return [] as ShopCategory[];
  const list: ShopCategory[] = [];
  for (const item of raw) {
    if (typeof item === 'string') {
      const name = item.trim();
      if (name) list.push({ id: name, name });
      continue;
    }
    if (item && typeof item === 'object') {
      const row = item as { id?: string; name?: string };
      const id = String(row.id ?? row.name ?? '').trim();
      const name = String(row.name ?? row.id ?? '').trim();
      if (id || name) list.push({ id: id || name, name: name || id });
    }
  }
  return list;
});
const shopBalance = computed(() => {
  if (shop.value.balanceKnown === false) return '--';
  const currencies = (shop.value.currencies as Array<{ balance?: string | number | null }> | undefined) || [];
  const currency = shop.value.currency as { balance?: string | number | null; count?: string | number } | undefined;
  const value = shop.value.balance ?? currencies[0]?.balance ?? currency?.balance ?? currency?.count;
  if (value === undefined || value === null || value === '') return '--';
  return String(value);
});
function goodsCategory(goods: ShopGoods) {
  return String(goods.categoryId || goods.categoryName || goods.category || '').trim();
}
const visibleShopGoods = computed(() => {
  if (shopCategory.value === '__all__') return shopGoods.value;
  return shopGoods.value.filter(item => goodsCategory(item) === shopCategory.value);
});

const solarTermList = computed(() => (solarTerms.value.terms as SolarTerm[]) || []);
const selectedSolar = computed(() => solarTermList.value.find(term => term.id === selectedSolarId.value) || null);

const greenPlumActive = computed(() => greenPlum.value.known === true && greenPlum.value.active === true);
const greenPlumEnd = computed(() => Number(greenPlum.value.endTime || 0) || undefined);
const qixiKnown = computed(() => Boolean(qixi.value.name || qixi.value.groupId));
const qixiEnd = computed(() => Number(qixi.value.endTime || 0) || undefined);
const greenPlumBalance = computed(() => {
  if (greenPlum.value.balanceKnown === false) return '--';
  return greenPlum.value.balance ?? '0';
});
const visibleTabs = computed(() => allTabs.map(tab => ({ ...tab, label: '' })));
const greenPlumStarted = computed(() => greenPlum.value.started === true);
const greenPlumFinished = computed(() => greenPlum.value.finished === true);
const greenPlumRound = computed(() => Number(greenPlum.value.currentRound || 0));
const greenPlumMaxRounds = computed(() => Math.max(1, Number(greenPlum.value.maxRounds || 3)));
const greenPlumQuotes = computed(() => {
  const prices = greenPlum.value.quotePrices || [];
  const totals = greenPlum.value.quoteTotals || [];
  return totals.map((total, index) => ({
    index: index + 1,
    unitPrice: prices[index] || '0',
    total
  }));
});
const greenPlumSeedClaimed = computed(() => greenPlum.value.dailySeed?.claimed === true);
const greenPlumIngredients = computed(() => greenPlum.value.ingredients || []);
const greenPlumSelectedUids = ref<Set<string>>(new Set());
const greenPlumIngredientCounts = ref<Record<string, number>>({});
const greenPlumAllSelected = computed(
  () => greenPlumIngredients.value.length > 0 && greenPlumSelectedUids.value.size === greenPlumIngredients.value.length
);
const greenPlumSelectedIngredients = computed(() =>
  greenPlumIngredients.value
    .filter(item => greenPlumSelectedUids.value.has(String(item.uid)))
    .map(item => ({ uid: String(item.uid), count: greenPlumIngredientCounts.value[String(item.uid)] || 1 }))
);
const greenPlumSelectedTotal = computed(() =>
  greenPlumSelectedIngredients.value.reduce((sum, item) => sum + (item.count || 0), 0)
);
const greenPlumBusy = computed(() =>
  ['greenPlum', 'greenPlumStart', 'greenPlumContinue', 'greenPlumSettle'].includes(pendingKey.value || '')
);
const greenPlumCanStart = computed(() => {
  const action = greenPlum.value.actions?.start;
  if (action && typeof action.enabled === 'boolean') return action.enabled;
  return !greenPlumStarted.value;
});
const greenPlumCanContinue = computed(() => {
  const action = greenPlum.value.actions?.continue;
  if (action && typeof action.enabled === 'boolean') return action.enabled;
  return greenPlumStarted.value && !greenPlumFinished.value && greenPlumRound.value < greenPlumMaxRounds.value;
});
const greenPlumCanSettle = computed(() => {
  const action = greenPlum.value.actions?.settle;
  if (action && typeof action.enabled === 'boolean') return action.enabled;
  return greenPlumQuotes.value.length > 0 || greenPlumFinished.value;
});

const stellarAvailable = computed(
  () => Boolean(season.value.title || shop.value.title || constellation.value.title || solarTermList.value.length)
);

function activityTimestamp(value: unknown) {
  const n = Number(value || 0);
  if (!n) return 0;
  return n > 1e12 ? n : n * 1000;
}

function activityStatus(activity: ActivityDirectoryItem): ActivityStatus {
  const now = clockNow.value;
  const end = activityTimestamp(activity.endTime);
  const start = activityTimestamp(activity.startTime);
  if (end && now >= end) return 'ended';
  if (start && now < start) return 'upcoming';
  return 'active';
}

function resolveGameplay(activity: ActivityDirectoryItem): GameplayKey | null {
  const key =
    activity.gameplayKey ||
    (activity.detailTarget === 'qixi'
      ? 'qixi'
      : activity.detailTarget === 'greenPlum'
        ? 'greenPlum'
        : activity.detailTarget
          ? 'stellar'
          : null);
  if (key === 'qixi' || key === 'stellar' || key === 'greenPlum') return key;
  return null;
}

const displayActivities = computed(() => {
  const entries = directory.value.length
    ? [...directory.value]
    : stellarAvailable.value
      ? [
          {
            id: String((season.value.pass as { activityId?: string } | undefined)?.activityId || season.value.id || 'stellar'),
            name: String(season.value.title || $t('page.farm.activity.tabTravel')),
            startTime: Number(season.value.startTime || 0),
            endTime: Number(season.value.endTime || 0),
            gameplayKey: 'stellar',
            gameplayTargets: ['travel', 'constellation', 'shop', 'solar'],
            detailTarget: 'travel'
          } satisfies ActivityDirectoryItem
        ]
      : [];
  if (greenPlumActive.value && !entries.some(item => item.gameplayKey === 'greenPlum' || item.detailTarget === 'greenPlum')) {
    entries.push({
      id: String(greenPlum.value.activityId || greenPlum.value.brewActivityId || 'greenPlum'),
      name: String(greenPlum.value.name || $t('page.farm.activity.tabGreenPlum')),
      startTime: Number(greenPlum.value.startTime || 0),
      endTime: Number(greenPlum.value.endTime || 0),
      gameplayKey: 'greenPlum',
      detailTarget: 'greenPlum'
    });
  }
  if (qixiKnown.value && !entries.some(item => resolveGameplay(item) === 'qixi')) {
    entries.push({
      id: String(qixi.value.groupId || qixi.value.activityId || 'qixi'),
      name: String(qixi.value.name || qixi.value.title || $t('page.farm.activity.tabQixi')),
      startTime: Number(qixi.value.startTime || 0),
      endTime: Number(qixi.value.endTime || 0),
      gameplayKey: 'qixi',
      detailTarget: 'qixi'
    });
  }
  const rank: Record<ActivityStatus, number> = { active: 0, upcoming: 1, ended: 2 };
  return entries.sort((left, right) => {
    const leftStatus = activityStatus(left);
    const rightStatus = activityStatus(right);
    if (leftStatus !== rightStatus) return rank[leftStatus] - rank[rightStatus];
    return activityTimestamp(left.startTime) - activityTimestamp(right.startTime);
  });
});

const pageTitle = computed(() => {
  if (selectedGameplay.value === 'qixi') {
    return String(qixi.value.name || qixi.value.title || $t('page.farm.activity.tabQixi'));
  }
  if (selectedGameplay.value === 'greenPlum') {
    return String(greenPlum.value.name || $t('page.farm.activity.tabGreenPlum'));
  }
  if (activeTab.value === 'shop') {
    return String(shop.value.title || shop.value.name || $t('page.farm.activity.tabShop'));
  }
  if (activeTab.value === 'constellation') {
    return String(
      constellation.value.title || constellation.value.displayName || $t('page.farm.activity.tabConstellation')
    );
  }
  if (activeTab.value === 'solar') {
    return String(
      selectedSolar.value?.title ||
        selectedSolar.value?.name ||
        solarTerms.value.title ||
        $t('page.farm.activity.tabSolar')
    );
  }
  return String(season.value.title || $t('page.farm.activity.tabTravel'));
});

const remainingText = computed(() => {
  let endTime: number | undefined;
  if (selectedGameplay.value === 'qixi') endTime = qixiEnd.value;
  else if (selectedGameplay.value === 'greenPlum') endTime = greenPlumEnd.value;
  else if (activeTab.value === 'shop') endTime = Number(shop.value.endTime || 0) || undefined;
  else if (activeTab.value === 'constellation') {
    endTime = Number(constellation.value.endTime || season.value.endTime || 0) || undefined;
  } else if (activeTab.value === 'solar') {
    endTime = Number(selectedSolar.value?.endTime || season.value.endTime || 0) || undefined;
  } else endTime = Number(season.value.endTime || 0) || undefined;
  if (!endTime) return '';
  const endMs = endTime > 1e12 ? endTime : endTime * 1000;
  const diff = Math.max(0, endMs - clockNow.value);
  if (diff === 0) return $t('page.farm.activity.ended');
  const days = Math.floor(diff / 86400000);
  const hours = Math.floor((diff % 86400000) / 3600000);
  const minutes = Math.floor((diff % 3600000) / 60000);
  return days > 0
    ? $t('page.farm.activity.remainingDays', { days, hours })
    : $t('page.farm.activity.remainingHours', { hours, minutes });
});

const tabBadges = computed(() => ({
  travel: hasClaimablePass.value && actionEnabled('claimPass'),
  constellation: constellationGroups.value.some(g =>
    ['lightable', 'claimableUnknown'].includes(String(g.visualState || ''))
  ),
  shop: shopGoods.value.some(g => g.exchangeable),
  solar: solarTermList.value.some(t => t.claimable || t.canClaim)
}));

function actionEnabled(key: string): boolean {
  const action = actions.value[key];
  if (action && typeof action.enabled === 'boolean') return action.enabled;
  return capabilities.value[key] === true;
}

function rewardImage(item?: RewardItem) {
  return resolveCatalogImage(item?.image);
}

function formatRewardLabel(item: RewardItem) {
  const name = String(item.name || '').trim();
  const label = name && !/^\d+$/.test(name) ? name : '';
  const count = item.count == null ? '' : `×${item.count}`;
  return `${label}${count}`.trim();
}

function claimSuccessMessage(data: Record<string, unknown> | null | undefined) {
  if (data?.message && typeof data.message === 'string') {
    return String(data.message);
  }
  const rewards = (data?.rewards as RewardItem[] | undefined) || [];
  const labels = rewards.map(item => formatRewardLabel(item)).filter(Boolean);
  if (labels.length) {
    return $t('page.farm.activity.claimRewardsSuccess', { items: labels.join('、') });
  }
  return $t('page.farm.activity.claimSuccess');
}

function notifyClaimResult(data: Record<string, unknown> | null | undefined) {
  const text = claimSuccessMessage(data);
  if (data?.noClaimable || data?.outcome === 'nothingToClaim') {
    message.info(text);
    return;
  }
  message.success(text);
}

function shopDisabledReason(item: ShopGoods) {
  if (pendingKey.value === 'shop') return $t('page.farm.activity.exchanging');
  if (item.owned || item.soldOut) return $t('page.farm.activity.alreadyExchanged');
  if (!item.exchangeable) return $t('page.farm.activity.notExchangeable');
  if (!actionEnabled('exchange')) return $t('page.farm.activity.exchangeUnavailable');
  return '';
}

function solarButtonLabel(term: SolarTerm | null) {
  if (!term) return $t('page.farm.activity.claimUnavailable');
  if (pendingKey.value === 'solar') return $t('page.farm.activity.claiming');
  if (term.claimed) return $t('page.farm.activity.claimed');
  if (term.locked) return $t('page.farm.activity.notStarted');
  if (term.claimable || term.canClaim) return $t('page.farm.activity.claim');
  return $t('page.farm.activity.claimUnavailable');
}

function constellationStateLabel(group: ConstellationGroup | null) {
  if (!group) return '';
  if (group.visualState === 'lit') {
    return group.claimStatus === 'confirmed-no-claimable'
      ? $t('page.farm.activity.claimedToday')
      : $t('page.farm.activity.lit');
  }
  if (group.visualState === 'locked') return $t('page.farm.activity.locked');
  if (group.visualState === 'lightable' || group.visualState === 'claimableUnknown') {
    return $t('page.farm.activity.lightable');
  }
  return String(group.visualState || '');
}

/** Prefer today's lightable node, then currentDay, then latest progress — not the first star. */
function pickConstellationFocus(groups: ConstellationGroup[]) {
  const sorted = groups.slice().sort((a, b) => Number(a.order ?? 999) - Number(b.order ?? 999));
  const actionable = sorted.find(g => ['lightable', 'claimableUnknown'].includes(String(g.visualState || '')));
  if (actionable?.id) return actionable.id;

  const currentDay = Number(constellation.value.currentDay || 0);
  if (currentDay > 0) {
    const byDay = sorted.find(g => Number(g.order) === currentDay);
    if (byDay?.id) return byDay.id;
  }

  const inProgress = sorted.find(g => g.visualState === 'unknown');
  if (inProgress?.id) return inProgress.id;

  const latestLit = [...sorted].reverse().find(g => g.visualState === 'lit');
  if (latestLit?.id) return latestLit.id;

  return sorted[0]?.id || '';
}

function ensureActiveTabVisible() {
  if (visibleTabs.value.some(tab => tab.key === activeTab.value)) return;
  activeTab.value = visibleTabs.value[0]?.key || 'travel';
}

function activityHasGameplay(activity: ActivityDirectoryItem) {
  return resolveGameplay(activity) !== null;
}

function activityStatusLabel(activity: ActivityDirectoryItem) {
  return {
    active: $t('page.farm.activity.statusActive'),
    upcoming: $t('page.farm.activity.statusUpcoming'),
    ended: $t('page.farm.activity.statusEnded')
  }[activityStatus(activity)];
}

function formatActivityPeriod(activity: ActivityDirectoryItem) {
  const startMs = activityTimestamp(activity.startTime);
  const endMs = activityTimestamp(activity.endTime);
  const start = startMs ? dateFormatter.format(startMs) : '';
  const end = endMs ? dateFormatter.format(endMs) : '';
  if (start && end) return `${start} - ${end}`;
  if (start) return `${start} ${$t('page.farm.activity.statusUpcoming')}`;
  if (end) return `${end} ${$t('page.farm.activity.statusEnded')}`;
  return $t('page.farm.activity.periodPending');
}

function openActivity(activity: ActivityDirectoryItem) {
  const gameplay = resolveGameplay(activity);
  if (!gameplay) return;
  if (gameplay === 'stellar') {
    const target = activity.detailTarget;
    if (target === 'constellation' || target === 'shop' || target === 'solar' || target === 'travel') {
      activeTab.value = target;
    }
  }
  selectedGameplay.value = gameplay;
  if (gameplay === 'qixi') void loadQixiFriends();
}

function goBackToList() {
  selectedGameplay.value = null;
}

function applySnapshot(data: Api.Farm.ActivitySnapshot) {
  const snap = (data.snapshot || data) as Api.Farm.ActivitySnapshot;
  season.value = (snap.season as Record<string, unknown>) || {};
  constellation.value = (snap.constellation as Record<string, unknown>) || {};
  shop.value = (snap.shop as Record<string, unknown>) || {};
  solarTerms.value = (snap.solarTerms as Record<string, unknown>) || {};
  greenPlum.value = (snap.greenPlum as GreenPlum) || (snap.qingMei as GreenPlum) || {};
  qixi.value = (snap.qixi as Qixi) || {};
  directory.value = Array.isArray(snap.activities) ? (snap.activities as ActivityDirectoryItem[]) : [];
  capabilities.value = snap.capabilities || data.capabilities || {};
  actions.value = snap.actions || data.actions || {};
  ensureActiveTabVisible();

  const groups = (constellation.value.groups as ConstellationGroup[]) || [];
  selectedConstellationId.value = pickConstellationFocus(groups);
  const terms = (solarTerms.value.terms as SolarTerm[]) || [];
  const currentTermId = String(solarTerms.value.currentTermId || '');
  if (!terms.some(t => t.id === selectedSolarId.value)) {
    selectedSolarId.value = currentTermId || terms.find(t => t.current)?.id || terms[0]?.id || '';
  }
}

async function loadActivities() {
  if (!farmAccountStore.currentAccountId) {
    season.value = {};
    constellation.value = {};
    shop.value = {};
    solarTerms.value = {};
    greenPlum.value = {};
    qixi.value = {};
    directory.value = [];
    return;
  }
  loading.value = true;
  try {
    const { error, data } = await fetchGetFarmActivitySnapshot(farmAccountStore.currentAccountId);
    if (!error && data) applySnapshot(data);
  } finally {
    loading.value = false;
  }
}

async function claimPass() {
  if (!farmAccountStore.currentAccountId || !actionEnabled('claimPass')) return;
  const fallbackRewards = passNodes.value.filter(n => n.claimable).flatMap(n => n.rewards || []);
  pendingKey.value = 'pass';
  try {
    const { error, data } = await fetchClaimFarmActivityPass({
      accountId: farmAccountStore.currentAccountId
    });
    if (error) {
      message.error(error.message || $t('page.farm.activity.claimFailed'));
      return;
    }
    const payload = { ...(data as Record<string, unknown>) };
    if ((!Array.isArray(payload.rewards) || !(payload.rewards as unknown[]).length) && fallbackRewards.length) {
      payload.rewards = fallbackRewards;
    }
    notifyClaimResult(payload);
    if (data) applySnapshot(data as Api.Farm.ActivitySnapshot);
    else await loadActivities();
  } finally {
    pendingKey.value = null;
  }
}

async function lightConstellation() {
  if (!farmAccountStore.currentAccountId || !canLightConstellation.value) return;
  pendingKey.value = 'constellation';
  try {
    const { error, data } = await fetchLightFarmActivityConstellation({
      accountId: farmAccountStore.currentAccountId
    });
    if (error) {
      message.error(error.message || $t('page.farm.activity.claimFailed'));
      return;
    }
    notifyClaimResult(data as Record<string, unknown>);
    if (data) applySnapshot(data as Api.Farm.ActivitySnapshot);
    else await loadActivities();
  } finally {
    pendingKey.value = null;
  }
}

function openExchange(goods: ShopGoods) {
  if (shopDisabledReason(goods)) return;
  exchangeGoods.value = goods;
  exchangeCount.value = 1;
  exchangeOpen.value = true;
}

async function confirmExchange() {
  if (!farmAccountStore.currentAccountId || !exchangeGoods.value?.id) return;
  const exchanging = exchangeGoods.value;
  pendingKey.value = 'shop';
  try {
    const { error, data } = await fetchExchangeFarmActivityShop({
      accountId: farmAccountStore.currentAccountId,
      itemId: String(exchanging.id),
      count: exchangeCount.value || 1
    });
    if (error) {
      message.error(error.message || $t('page.farm.activity.claimFailed'));
      return;
    }
    const payload = { ...(data as Record<string, unknown>) };
    if (!Array.isArray(payload.rewards) || !(payload.rewards as unknown[]).length) {
      const fallbackItem = exchanging.item
        ? {
            ...exchanging.item,
            count: Number(exchanging.item.count || 1) * Number(exchangeCount.value || 1)
          }
        : { id: exchanging.id, name: exchanging.name, count: exchangeCount.value || 1 };
      payload.rewards = [fallbackItem];
    }
    notifyClaimResult(payload);
    exchangeOpen.value = false;
    exchangeGoods.value = null;
    if (data) applySnapshot(data as Api.Farm.ActivitySnapshot);
    else await loadActivities();
  } finally {
    pendingKey.value = null;
  }
}

async function claimSolar() {
  const term = selectedSolar.value;
  if (!farmAccountStore.currentAccountId || !term?.id) return;
  if (!(term.claimable || term.canClaim) || term.claimed) return;
  pendingKey.value = 'solar';
  try {
    const { error, data } = await fetchClaimFarmActivitySolarTerm({
      accountId: farmAccountStore.currentAccountId,
      termId: term.id
    });
    if (error) {
      message.error(error.message || $t('page.farm.activity.claimFailed'));
      return;
    }
    const payload = { ...(data as Record<string, unknown>) };
    if ((!Array.isArray(payload.rewards) || !(payload.rewards as unknown[]).length) && term.rewards?.length) {
      payload.rewards = term.rewards;
    }
    notifyClaimResult(payload);
    if (data) applySnapshot(data as Api.Farm.ActivitySnapshot);
    else await loadActivities();
  } finally {
    pendingKey.value = null;
  }
}

async function claimGreenPlum() {
  if (!farmAccountStore.currentAccountId || !greenPlumActive.value) return;
  if (greenPlumSeedClaimed.value) return;
  pendingKey.value = 'greenPlum';
  try {
    const { error, data } = await fetchClaimFarmActivityGreenPlum({
      accountId: farmAccountStore.currentAccountId
    });
    if (error) {
      message.error(error.message || $t('page.farm.activity.claimFailed'));
      return;
    }
    notifyClaimResult(data as Record<string, unknown>);
    if (data) applySnapshot(data as Api.Farm.ActivitySnapshot);
    else await loadActivities();
  } finally {
    pendingKey.value = null;
  }
}

async function startGreenPlumBrew() {
  if (!farmAccountStore.currentAccountId || !greenPlumActive.value) return;
  const ingredients = greenPlumSelectedIngredients.value;
  if (ingredients.length === 0) return;
  pendingKey.value = 'greenPlumStart';
  try {
    const { error, data } = await fetchStartFarmActivityGreenPlumBrew({
      accountId: farmAccountStore.currentAccountId,
      ingredients
    });
    if (error) {
      message.error(error.message || $t('page.farm.activity.claimFailed'));
      return;
    }
    notifyClaimResult(data as Record<string, unknown>);
    if (data) applySnapshot(data as Api.Farm.ActivitySnapshot);
    else await loadActivities();
  } finally {
    pendingKey.value = null;
  }
}

async function continueGreenPlumBrew() {
  if (!farmAccountStore.currentAccountId || !greenPlumActive.value) return;
  pendingKey.value = 'greenPlumContinue';
  try {
    const { error, data } = await fetchContinueFarmActivityGreenPlumBrew({
      accountId: farmAccountStore.currentAccountId
    });
    if (error) {
      message.error(error.message || $t('page.farm.activity.claimFailed'));
      return;
    }
    const payload = data as Record<string, unknown> | null | undefined;
    if (payload?.message && typeof payload.message === 'string') {
      message.success(String(payload.message));
    }
    if (data) applySnapshot(data as Api.Farm.ActivitySnapshot);
    else await loadActivities();
  } finally {
    pendingKey.value = null;
  }
}

async function settleGreenPlumBrew() {
  if (!farmAccountStore.currentAccountId || !greenPlumActive.value) return;
  pendingKey.value = 'greenPlumSettle';
  try {
    const { error, data } = await fetchSettleFarmActivityGreenPlumBrew({
      accountId: farmAccountStore.currentAccountId
    });
    if (error) {
      message.error(error.message || $t('page.farm.activity.claimFailed'));
      return;
    }
    notifyClaimResult(data as Record<string, unknown>);
    if (data) applySnapshot(data as Api.Farm.ActivitySnapshot);
    else await loadActivities();
  } finally {
    pendingKey.value = null;
  }
}

function toggleGreenPlumIngredient(uid: string) {
  const next = new Set(greenPlumSelectedUids.value);
  if (next.has(uid)) next.delete(uid);
  else next.add(uid);
  greenPlumSelectedUids.value = next;
}

function toggleGreenPlumAll() {
  greenPlumSelectedUids.value = greenPlumAllSelected.value
    ? new Set()
    : new Set(greenPlumIngredients.value.map(item => String(item.uid)));
}

function setGreenPlumIngredientCount(uid: string, value: unknown) {
  const item = greenPlumIngredients.value.find(entry => String(entry.uid) === uid);
  const maximum = Math.max(1, Number(item?.count || 1));
  greenPlumIngredientCounts.value = {
    ...greenPlumIngredientCounts.value,
    [uid]: Math.max(1, Math.min(Math.trunc(Number(value) || 1), maximum))
  };
}

async function loadQixiFriends(force = false) {
  if (!farmAccountStore.currentAccountId) {
    qixiFriends.value = [];
    return;
  }
  qixiFriendsLoading.value = true;
  try {
    const { error, data } = await fetchGetFarmFriendList({
      current: 1,
      size: 500,
      accountId: farmAccountStore.currentAccountId,
      force
    });
    if (!error && data) {
      qixiFriends.value = data.records || [];
    }
  } finally {
    qixiFriendsLoading.value = false;
  }
}

async function claimQixiBridge() {
  if (!farmAccountStore.currentAccountId) return;
  pendingKey.value = 'qixiBridge';
  try {
    const { error, data } = await fetchClaimFarmActivityQixiBridge({
      accountId: farmAccountStore.currentAccountId
    });
    if (error) {
      message.error(error.message || $t('page.farm.activity.claimFailed'));
      return;
    }
    notifyClaimResult(data as Record<string, unknown>);
    if (data) applySnapshot(data as Api.Farm.ActivitySnapshot);
    else await loadActivities();
  } finally {
    pendingKey.value = null;
  }
}

async function giftQixiSachet(payload: { friendGid: string; count: number }) {
  if (!farmAccountStore.currentAccountId) return;
  const count = Math.trunc(Number(payload.count));
  if (!payload.friendGid || count < 1) return;
  const friend = qixiFriends.value.find(item => String(item.gid) === String(payload.friendGid));
  const friendName =
    String(friend?.nickname || friend?.name || '').trim() || $t('page.farm.activity.qixiFriendFallback');
  pendingKey.value = 'qixiGift';
  try {
    const { error, data } = await fetchGiftFarmActivityQixiSachet({
      accountId: farmAccountStore.currentAccountId,
      friendGid: payload.friendGid,
      count,
      sachetCount: count
    });
    if (error) {
      message.error(error.message || $t('page.farm.activity.claimFailed'));
      return;
    }
    notifyClaimResult({
      ...(data as Record<string, unknown>),
      message: $t('page.farm.activity.qixiGiftSuccess', { name: friendName, count })
    });
    if (data) applySnapshot(data as Api.Farm.ActivitySnapshot);
    else await loadActivities();
  } finally {
    pendingKey.value = null;
  }
}

function greenPlumIngredientCount(uid: string) {
  return greenPlumIngredientCounts.value[uid] || 1;
}

watch(
  () => farmAccountStore.currentAccountId,
  () => {
    selectedGameplay.value = null;
    void loadActivities();
  }
);

watch(
  () => selectedGameplay.value,
  gameplay => {
    if (gameplay === 'qixi') void loadQixiFriends();
  }
);

watch(
  greenPlumIngredients,
  items => {
    const availableUids = new Set(items.map(item => String(item.uid)));
    greenPlumSelectedUids.value = new Set([...greenPlumSelectedUids.value].filter(uid => availableUids.has(uid)));
    const nextCounts: Record<string, number> = {};
    for (const item of items) {
      const uid = String(item.uid);
      const max = Math.max(1, Number(item.count || 1));
      nextCounts[uid] = Math.max(1, Math.min(greenPlumIngredientCounts.value[uid] || max, max));
    }
    greenPlumIngredientCounts.value = nextCounts;
  },
  { immediate: true }
);

onMounted(async () => {
  if (!farmAccountStore.accounts.length) {
    await farmAccountStore.loadAccounts();
  }
  await loadActivities();
  clockTimer = setInterval(() => {
    clockNow.value = Date.now();
  }, 1000);
});

onUnmounted(() => {
  if (clockTimer) clearInterval(clockTimer);
});
</script>

<template>
  <div class="min-h-500px flex-col-stretch gap-16px overflow-auto">
    <NEmpty
      v-if="!farmAccountStore.currentAccountId"
      class="py-48px"
      :description="$t('page.farm.common.selectAccount')"
    />

    <template v-else-if="!selectedGameplay">
      <div class="flex flex-wrap items-center justify-between gap-12px">
        <div>
          <div class="text-12px text-gray-500">{{ $t('page.farm.activity.title') }}</div>
          <div class="text-18px font-medium">
            {{ displayActivities.length ? $t('page.farm.activity.listTitle') : $t('page.farm.activity.noActivities') }}
          </div>
        </div>
        <NButton size="small" :loading="loading" @click="loadActivities">
          {{ $t('common.refresh') }}
        </NButton>
      </div>
      <NSpin :show="loading">
        <NEmpty v-if="!loading && !displayActivities.length" class="py-48px" :description="$t('page.farm.activity.noActivities')" />
        <div v-else class="grid gap-12px md:grid-cols-2 xl:grid-cols-3">
          <button
            v-for="activity in displayActivities"
            :key="String(activity.id || activity.name)"
            type="button"
            class="rounded-8px border border-gray-200 px-16px py-14px text-left dark:border-gray-700"
            :class="activityHasGameplay(activity) ? 'hover:border-primary' : 'opacity-60'"
            :disabled="!activityHasGameplay(activity)"
            @click="openActivity(activity)"
          >
            <div class="mb-8px flex items-center justify-between gap-8px">
              <NTag size="tiny" :type="activityStatus(activity) === 'active' ? 'success' : 'default'" :bordered="false">
                {{ activityStatusLabel(activity) }}
              </NTag>
            </div>
            <div class="text-16px font-medium">{{ activity.name }}</div>
            <div class="mt-6px text-12px text-gray-500">{{ formatActivityPeriod(activity) }}</div>
            <div class="mt-10px text-12px" :class="activityHasGameplay(activity) ? 'text-primary' : 'text-gray-400'">
              {{ activityHasGameplay(activity) ? $t('page.farm.activity.viewDetail') : $t('page.farm.activity.unsupported') }}
            </div>
          </button>
        </div>
      </NSpin>
    </template>

    <template v-else>
      <div>
        <NButton size="small" secondary @click="goBackToList">
          <template #icon>
            <icon-ic-round-arrow-back class="text-icon" />
          </template>
          {{ $t('page.farm.activity.backToList') }}
        </NButton>
        <div class="mt-12px flex flex-wrap items-center justify-between gap-12px">
          <div>
            <div class="text-18px font-medium">{{ pageTitle }}</div>
            <div v-if="remainingText" class="mt-4px text-12px text-gray-500">{{ remainingText }}</div>
          </div>
          <NSpace align="center">
            <span
              v-if="selectedGameplay === 'stellar' && (activeTab === 'shop' || activeTab === 'travel')"
              class="text-13px text-amber-600"
            >
              {{ $t('page.farm.activity.starSand') }} {{ shopBalance }}
            </span>
            <NButton size="small" :loading="loading" @click="loadActivities">
              {{ $t('common.refresh') }}
            </NButton>
          </NSpace>
        </div>
      </div>

      <div
        v-if="selectedGameplay === 'stellar'"
        class="flex flex-wrap gap-8px border-b border-gray-200 pb-12px dark:border-gray-700"
      >
        <NButton
          v-for="tab in visibleTabs"
          :key="tab.key"
          size="small"
          :type="activeTab === tab.key ? 'primary' : 'default'"
          secondary
          @click="activeTab = tab.key"
        >
          <NSpace :size="6" align="center">
            <span>{{ tab.label || $t(tab.labelKey) }}</span>
            <NTag v-if="tabBadges[tab.key]" size="tiny" type="error" :bordered="false" round>•</NTag>
          </NSpace>
        </NButton>
      </div>

      <NSpin :show="loading">
        <template v-if="selectedGameplay === 'stellar'">
        <!-- 千星游记 -->
        <NCard v-show="activeTab === 'travel'" :bordered="false" size="small" class="card-wrapper">
          <div
            class="mb-16px flex flex-wrap items-center gap-16px rounded-8px bg-blue-50 px-16px py-14px dark:bg-blue-900/20"
          >
            <div class="h-64px w-64px flex flex-col items-center justify-center rounded-full bg-blue-500 text-white">
              <div class="text-22px font-semibold">{{ passLevel }}</div>
              <div class="text-11px opacity-80">{{ $t('page.farm.activity.level') }}</div>
            </div>
            <div class="min-w-200px flex-1">
              <div class="mb-6px text-13px font-medium">{{ $t('page.farm.activity.travelScore') }}</div>
              <div class="mb-8px text-15px">{{ passProgress }} / {{ passProgressMax || '--' }}</div>
              <NProgress type="line" :percentage="passPercent" :show-indicator="false" />
            </div>
          </div>

          <div class="mb-12px flex flex-wrap items-center gap-8px text-12px text-gray-500">
            <span>{{ $t('page.farm.activity.travelTip') }}</span>
            <NButton v-if="passRules" size="tiny" quaternary @click="travelRulesOpen = true">
              {{ $t('page.farm.activity.viewRules') }}
            </NButton>
          </div>

          <NEmpty v-if="!passNodes.length" class="py-24px" :description="$t('common.noData')" />
          <div v-else class="flex-col gap-10px">
            <div
              v-for="node in passNodes"
              :key="String(node.id ?? node.level)"
              class="flex flex-wrap items-center justify-between gap-12px rounded-8px border border-gray-200 px-12px py-10px dark:border-gray-700"
              :class="{
                'border-primary': node.current || node.level === passLevel,
                'opacity-70': node.claimed
              }"
            >
              <div class="flex items-center gap-12px">
                <div
                  class="h-44px w-44px flex flex-col items-center justify-center rounded-8px bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-200"
                >
                  <div class="text-16px font-semibold">{{ node.level ?? '--' }}</div>
                  <div class="text-10px">{{ $t('page.farm.activity.level') }}</div>
                </div>
                <div class="flex flex-wrap gap-8px">
                  <div
                    v-for="(reward, idx) in node.rewards || []"
                    :key="String(reward.id ?? idx)"
                    class="flex items-center gap-6px rounded-6px bg-gray-50 px-8px py-4px text-12px dark:bg-gray-800"
                  >
                    <img
                      v-if="rewardImage(reward)"
                      :src="rewardImage(reward)"
                      class="h-24px w-24px object-contain"
                      loading="lazy"
                    />
                    <span>{{ formatRewardLabel(reward) }}</span>
                  </div>
                </div>
              </div>
              <NTag v-if="node.claimed" size="small" :bordered="false">{{ $t('page.farm.activity.claimed') }}</NTag>
              <NTag v-else-if="node.claimable" size="small" type="success" :bordered="false">
                {{ $t('page.farm.activity.claimable') }}
              </NTag>
            </div>
          </div>

          <div class="mt-16px flex justify-center">
            <NButton
              type="primary"
              :loading="pendingKey === 'pass'"
              :disabled="!actionEnabled('claimPass') || !hasClaimablePass"
              @click="claimPass"
            >
              {{ pendingKey === 'pass' ? $t('page.farm.activity.claiming') : $t('page.farm.activity.claimAll') }}
            </NButton>
          </div>
        </NCard>

        <!-- 观星礼录 -->
        <NCard v-show="activeTab === 'constellation'" :bordered="false" size="small" class="card-wrapper">
          <div v-if="constellationRules" class="mb-12px flex justify-end">
            <NButton size="small" quaternary @click="constellationRulesOpen = true">
              {{ $t('page.farm.activity.viewRules') }}
            </NButton>
          </div>
          <NEmpty v-if="!constellationGroups.length" class="py-24px" :description="$t('common.noData')" />
          <template v-else>
            <div class="mb-12px flex flex-wrap gap-8px">
              <NButton
                v-for="group in constellationGroups"
                :key="String(group.id)"
                size="small"
                :type="selectedConstellationId === group.id ? 'primary' : 'default'"
                secondary
                @click="selectedConstellationId = group.id || ''"
              >
                {{ group.name || group.id }}
              </NButton>
            </div>

            <div class="rounded-8px border border-gray-200 p-16px dark:border-gray-700">
              <div class="mb-8px text-16px font-medium">
                {{ selectedConstellation?.name || selectedConstellation?.id || '-' }}
              </div>
              <div class="mb-12px text-12px text-gray-500">
                {{ constellationStateLabel(selectedConstellation) }}
              </div>
              <div class="mb-16px flex flex-wrap gap-8px">
                <div
                  v-for="(reward, idx) in selectedConstellation?.rewards || []"
                  :key="String(reward.id ?? idx)"
                  class="flex items-center gap-6px rounded-6px bg-gray-50 px-8px py-4px text-12px dark:bg-gray-800"
                >
                  <img
                    v-if="rewardImage(reward)"
                    :src="rewardImage(reward)"
                    class="h-24px w-24px object-contain"
                    loading="lazy"
                  />
                  <span>{{ formatRewardLabel(reward) }}</span>
                </div>
                <span v-if="!(selectedConstellation?.rewards || []).length" class="text-12px text-gray-400">
                  {{ $t('page.farm.activity.noRewards') }}
                </span>
              </div>
              <NButton
                type="primary"
                :loading="pendingKey === 'constellation'"
                :disabled="!canLightConstellation"
                @click="lightConstellation"
              >
                {{
                  pendingKey === 'constellation'
                    ? $t('page.farm.activity.claiming')
                    : $t('page.farm.activity.lightConstellation')
                }}
              </NButton>
            </div>
          </template>
        </NCard>

        <!-- 星砂商店 -->
        <NCard v-show="activeTab === 'shop'" :bordered="false" size="small" class="card-wrapper">
          <div class="mb-12px rounded-8px bg-sky-50 px-14px py-12px dark:bg-sky-900/20">
            <div class="text-16px font-medium">
              {{ shop.title || shop.name || $t('page.farm.activity.tabShop') }}
            </div>
            <div class="mt-4px text-12px text-gray-500">
              {{ shop.description || $t('page.farm.activity.shopHint') }}
            </div>
          </div>

          <div class="mb-12px flex flex-wrap gap-8px">
            <NButton
              size="tiny"
              :type="shopCategory === '__all__' ? 'primary' : 'default'"
              secondary
              @click="shopCategory = '__all__'"
            >
              {{ $t('page.farm.activity.allCategories') }}
            </NButton>
            <NButton
              v-for="category in shopCategories"
              :key="String(category.id)"
              size="tiny"
              :type="shopCategory === category.id ? 'primary' : 'default'"
              secondary
              @click="shopCategory = category.id || ''"
            >
              {{ category.name || category.id }}
            </NButton>
          </div>

          <NEmpty v-if="!visibleShopGoods.length" class="py-24px" :description="$t('common.noData')" />
          <div v-else class="grid gap-12px sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            <div
              v-for="goods in visibleShopGoods"
              :key="String(goods.id)"
              class="relative rounded-8px border border-gray-200 p-12px dark:border-gray-700"
              :class="{ 'opacity-60': !!shopDisabledReason(goods) }"
            >
              <NTag
                v-if="goods.owned || goods.soldOut"
                size="tiny"
                type="success"
                :bordered="false"
                class="absolute right-8px top-8px z-1"
              >
                {{ $t('page.farm.activity.alreadyExchanged') }}
              </NTag>
              <div class="mb-8px flex-center h-72px rounded-6px bg-gray-50 dark:bg-gray-800">
                <img
                  v-if="rewardImage(goods.item)"
                  :src="rewardImage(goods.item)"
                  class="max-h-56px max-w-56px object-contain"
                  loading="lazy"
                />
                <span v-else class="text-24px opacity-40">🎁</span>
              </div>
              <div class="mb-4px truncate text-13px font-medium">
                {{ goods.name || goods.item?.name || goods.id }}
              </div>
              <div class="mb-8px flex items-center gap-6px text-12px text-amber-600">
                <img
                  v-if="resolveCatalogImage(goods.cost?.image)"
                  :src="resolveCatalogImage(goods.cost?.image)"
                  class="h-16px w-16px object-contain"
                  loading="lazy"
                />
                <span>{{ goods.cost?.count ?? '--' }}</span>
              </div>
              <NButton
                block
                size="small"
                :disabled="!!shopDisabledReason(goods)"
                @click="openExchange(goods)"
              >
                {{ shopDisabledReason(goods) || $t('page.farm.activity.exchange') }}
              </NButton>
            </div>
          </div>
        </NCard>

        <!-- 节令小札 -->
        <NCard v-show="activeTab === 'solar'" :bordered="false" size="small" class="card-wrapper">
          <NEmpty v-if="!solarTermList.length" class="py-24px" :description="$t('common.noData')" />
          <div v-else class="grid gap-16px lg:grid-cols-[140px_1fr]">
            <div class="flex flex-col gap-8px lg:max-h-520px lg:overflow-auto">
              <NButton
                v-for="term in solarTermList"
                :key="String(term.id)"
                size="small"
                :type="selectedSolarId === term.id ? 'primary' : 'default'"
                secondary
                @click="selectedSolarId = term.id || ''"
              >
                <span>{{ term.name || term.id }}</span>
                <NTag
                  v-if="term.claimable || term.canClaim"
                  size="tiny"
                  type="error"
                  :bordered="false"
                  class="ml-6px"
                  round
                >
                  •
                </NTag>
              </NButton>
            </div>

            <div class="rounded-8px border border-gray-200 p-16px dark:border-gray-700">
              <div v-if="selectedSolar?.englishName" class="mb-4px text-12px tracking-2px text-gray-400">
                {{ selectedSolar.englishName }}
              </div>
              <div class="mb-8px text-22px font-semibold">
                {{ selectedSolar?.title || selectedSolar?.name || '-' }}
              </div>
              <div class="mb-16px whitespace-pre-line text-13px text-gray-500">
                {{ selectedSolar?.description || solarTerms.description || '' }}
              </div>
              <div v-if="selectedSolar?.rewardTitle" class="mb-4px text-15px font-medium">
                {{ selectedSolar.rewardTitle }}
              </div>
              <div v-if="selectedSolar?.rewardDescription" class="mb-12px text-12px text-gray-500">
                {{ selectedSolar.rewardDescription }}
              </div>
              <div class="mb-16px flex flex-wrap gap-8px">
                <div
                  v-for="(reward, idx) in selectedSolar?.rewards || []"
                  :key="String(reward.id ?? idx)"
                  class="flex items-center gap-6px rounded-6px bg-gray-50 px-8px py-4px text-12px dark:bg-gray-800"
                >
                  <img
                    v-if="rewardImage(reward)"
                    :src="rewardImage(reward)"
                    class="h-24px w-24px object-contain"
                    loading="lazy"
                  />
                  <span>{{ formatRewardLabel(reward) }}</span>
                </div>
                <span v-if="!(selectedSolar?.rewards || []).length" class="text-12px text-gray-400">
                  {{ $t('page.farm.activity.noRewards') }}
                </span>
              </div>
              <NButton
                type="primary"
                :loading="pendingKey === 'solar'"
                :disabled="!(selectedSolar?.claimable || selectedSolar?.canClaim) || !!selectedSolar?.claimed"
                @click="claimSolar"
              >
                {{ solarButtonLabel(selectedSolar) }}
              </NButton>
            </div>
          </div>
        </NCard>
        </template>
        <QixiView
          v-else-if="selectedGameplay === 'qixi'"
          :activity="qixi"
          :friends="qixiFriends"
          :friends-loading="qixiFriendsLoading"
          :pending-bridge="pendingKey === 'qixiBridge'"
          :pending-gift="pendingKey === 'qixiGift'"
          @claim-bridge="claimQixiBridge"
          @gift="giftQixiSachet"
          @refresh-friends="loadQixiFriends(true)"
        />
        <!-- 青梅 -->
        <NCard v-else-if="selectedGameplay === 'greenPlum'" :bordered="false" size="small" class="card-wrapper">
          <div class="mb-12px rounded-8px bg-emerald-50 px-14px py-12px dark:bg-emerald-900/20">
            <div class="text-16px font-medium">
              {{ greenPlum.name || $t('page.farm.activity.greenPlumBrew') }}
            </div>
            <div class="mt-4px text-12px text-gray-500">{{ $t('page.farm.activity.greenPlumHint') }}</div>
          </div>

          <div class="mb-16px flex flex-wrap items-center gap-12px">
            <NTag
              size="small"
              :type="greenPlumActive ? 'success' : greenPlum.known ? 'warning' : 'default'"
              :bordered="false"
            >
              {{
                greenPlumActive
                  ? greenPlumFinished
                    ? $t('page.farm.activity.greenPlumReady')
                    : greenPlumSeedClaimed
                      ? $t('page.farm.activity.greenPlumSeedClaimed')
                      : $t('page.farm.activity.claimable')
                  : greenPlum.known
                    ? $t('page.farm.activity.claimUnavailable')
                    : $t('page.farm.activity.greenPlumUnknown')
              }}
            </NTag>
            <span v-if="greenPlum.typeCode" class="text-12px text-gray-500">
              {{ $t('page.farm.activity.activityId') }}: {{ greenPlum.activityId }} · type: {{ greenPlum.typeCode }}
            </span>
          </div>

          <div
            class="mb-16px flex flex-wrap items-center gap-10px rounded-8px border border-gray-200 px-12px py-10px dark:border-gray-700"
          >
            <span class="h-44px w-44px flex-center rounded-8px bg-emerald-100 text-24px dark:bg-emerald-900/40">
              🫒
            </span>
            <div class="flex-1">
              <div class="text-13px font-medium">
                {{ $t('page.farm.activity.greenPlumBalance') }}:
                <strong>{{ greenPlumBalance }}</strong>
              </div>
              <div class="mt-2px text-12px text-gray-500">{{ $t('page.farm.activity.greenPlumHint') }}</div>
            </div>
            <NButton
              type="primary"
              size="small"
              :loading="pendingKey === 'greenPlum'"
              :disabled="!greenPlumActive || greenPlumSeedClaimed"
              @click="claimGreenPlum"
            >
              {{
                pendingKey === 'greenPlum'
                  ? $t('page.farm.activity.claiming')
                  : greenPlumSeedClaimed
                    ? $t('page.farm.activity.greenPlumSeedClaimed')
                    : $t('page.farm.activity.greenPlumClaimSeed')
              }}
            </NButton>
          </div>

          <div class="rounded-8px border border-gray-200 p-16px dark:border-gray-700">
            <div class="mb-12px flex flex-wrap items-center justify-between gap-8px">
              <div>
                <div class="mb-4px text-13px text-gray-500">{{ $t('page.farm.activity.greenPlumBrewStatus') }}</div>
                <div class="text-15px font-medium">
                  {{
                    greenPlumStarted
                      ? $t('page.farm.activity.greenPlumRound', {
                          round: greenPlumRound,
                          max: greenPlumMaxRounds
                        })
                      : $t('page.farm.activity.greenPlumNotStarted')
                  }}
                </div>
              </div>
              <span class="text-12px text-gray-500">
                {{
                  $t('page.farm.activity.greenPlumBasePrice', {
                    price: greenPlum.guaranteedPrice || greenPlum.basePrice || '0'
                  })
                }}
              </span>
            </div>

            <template v-if="!greenPlumStarted">
              <div class="mb-8px flex items-center justify-between gap-8px">
                <span class="text-12px text-gray-500">{{ $t('page.farm.activity.greenPlumSelectIngredient') }}</span>
                <NButton
                  size="tiny"
                  quaternary
                  :disabled="greenPlumBusy || greenPlumIngredients.length === 0"
                  @click="toggleGreenPlumAll"
                >
                  {{
                    greenPlumAllSelected
                      ? $t('page.farm.activity.greenPlumDeselectAll')
                      : $t('page.farm.activity.greenPlumSelectAll')
                  }}
                </NButton>
              </div>

              <NEmpty
                v-if="!greenPlumIngredients.length"
                class="py-16px"
                :description="$t('page.farm.activity.greenPlumNoIngredient')"
              />
              <div v-else class="mb-12px flex flex-col gap-8px">
                <div
                  v-for="item in greenPlumIngredients"
                  :key="String(item.uid)"
                  class="rounded-8px border border-gray-200 px-12px py-10px dark:border-gray-700"
                  :class="{ 'border-emerald-500': greenPlumSelectedUids.has(String(item.uid)) }"
                >
                  <div
                    class="flex cursor-pointer items-center gap-10px"
                    @click="toggleGreenPlumIngredient(String(item.uid))"
                  >
                    <img
                      v-if="resolveCatalogImage(item.image)"
                      :src="resolveCatalogImage(item.image)"
                      class="h-36px w-36px object-contain"
                      loading="lazy"
                    />
                    <div class="flex-1">
                      <div class="text-13px font-medium">
                        {{ item.name || $t('page.farm.activity.greenPlumIngredientName') }}
                        <span v-if="item.mutantTypes?.length" class="text-11px text-gray-500">
                          · 变异 {{ item.mutantTypes.join('+') }}
                        </span>
                      </div>
                      <div class="mt-2px text-12px text-gray-500">
                        {{ $t('page.farm.activity.greenPlumUid') }} {{ item.uid }} ·
                        {{ $t('page.farm.activity.greenPlumOwned', { count: item.count }) }}
                      </div>
                    </div>
                    <div
                      class="flex h-22px w-22px items-center justify-center rounded-4px border border-gray-300"
                      :class="{
                        'border-emerald-500 bg-emerald-500 text-white': greenPlumSelectedUids.has(String(item.uid))
                      }"
                    >
                      <span v-if="greenPlumSelectedUids.has(String(item.uid))">✓</span>
                    </div>
                  </div>

                  <div v-if="greenPlumSelectedUids.has(String(item.uid))" class="mt-8px flex items-center gap-8px">
                    <span class="text-12px text-gray-500">{{ $t('page.farm.activity.greenPlumInputCount') }}</span>
                    <NInputNumber
                      :value="greenPlumIngredientCount(String(item.uid))"
                      class="w-110px"
                      :min="1"
                      :max="Number(item.count || 1)"
                      :disabled="greenPlumBusy"
                      @update:value="setGreenPlumIngredientCount(String(item.uid), $event)"
                    />
                    <NButton
                      size="tiny"
                      :disabled="greenPlumBusy || greenPlumIngredientCount(String(item.uid)) >= Number(item.count || 1)"
                      @click="setGreenPlumIngredientCount(String(item.uid), item.count)"
                    >
                      {{ $t('page.farm.activity.greenPlumAll') }}
                    </NButton>
                  </div>
                </div>
              </div>

              <div class="mb-12px flex flex-wrap items-center justify-between gap-8px text-12px text-gray-500">
                <span>
                  {{
                    $t('page.farm.activity.greenPlumSelectedSummary', {
                      groups: greenPlumSelectedIngredients.length,
                      total: greenPlumSelectedTotal
                    })
                  }}
                </span>
              </div>

              <div class="flex justify-center">
                <NButton
                  type="primary"
                  :loading="pendingKey === 'greenPlumStart'"
                  :disabled="!greenPlumActive || !greenPlumCanStart || greenPlumSelectedIngredients.length === 0"
                  @click="startGreenPlumBrew"
                >
                  {{
                    pendingKey === 'greenPlumStart'
                      ? $t('page.farm.activity.greenPlumBrewing')
                      : $t('page.farm.activity.greenPlumStartBrew')
                  }}
                </NButton>
              </div>
            </template>

            <template v-else>
              <div class="mb-12px grid gap-8px sm:grid-cols-2 lg:grid-cols-3">
                <div
                  v-for="quote in greenPlumQuotes"
                  :key="quote.index"
                  class="rounded-8px bg-emerald-50 px-10px py-8px text-center dark:bg-emerald-900/20"
                >
                  <div class="text-12px text-gray-500">
                    {{ $t('page.farm.activity.greenPlumQuoteRound', { round: quote.index }) }}
                  </div>
                  <div class="mt-2px text-15px font-semibold text-emerald-700 dark:text-emerald-300">
                    {{ Number(quote.total).toLocaleString() }}
                  </div>
                  <div class="mt-2px text-11px text-gray-500">
                    {{ $t('page.farm.activity.greenPlumQuoteUnitPrice', { price: quote.unitPrice }) }}
                  </div>
                </div>
                <div
                  v-for="index in Math.max(0, greenPlumMaxRounds - greenPlumQuotes.length)"
                  :key="`pending-${index}`"
                  class="rounded-8px bg-gray-50 px-10px py-8px text-center dark:bg-gray-800"
                >
                  <div class="text-12px text-gray-500">
                    {{
                      $t('page.farm.activity.greenPlumQuoteRound', {
                        round: greenPlumQuotes.length + index
                      })
                    }}
                  </div>
                  <div class="mt-2px text-15px font-semibold text-gray-400">
                    {{ $t('page.farm.activity.greenPlumQuotePending') }}
                  </div>
                </div>
              </div>

              <div class="flex flex-wrap justify-center gap-8px">
                <NButton
                  type="primary"
                  :loading="pendingKey === 'greenPlumContinue'"
                  :disabled="!greenPlumActive || !greenPlumCanContinue"
                  @click="continueGreenPlumBrew"
                >
                  {{
                    pendingKey === 'greenPlumContinue'
                      ? $t('page.farm.activity.greenPlumBrewing')
                      : $t('page.farm.activity.greenPlumContinueBrew')
                  }}
                </NButton>
                <NButton
                  type="warning"
                  :loading="pendingKey === 'greenPlumSettle'"
                  :disabled="!greenPlumActive || !greenPlumCanSettle"
                  @click="settleGreenPlumBrew"
                >
                  {{
                    pendingKey === 'greenPlumSettle'
                      ? $t('page.farm.activity.greenPlumSelling')
                      : $t('page.farm.activity.greenPlumSettleBrew')
                  }}
                </NButton>
              </div>
            </template>
          </div>

          <NCollapse v-if="greenPlumRules" class="mt-16px" :default-expanded-names="['rules']">
            <NCollapseItem
              :title="greenPlumRules.title || $t('page.farm.activity.qixiRules')"
              name="rules"
            >
              <p
                v-for="(paragraph, index) in greenPlumRules.paragraphs"
                :key="index"
                class="mb-8px whitespace-pre-line text-13px leading-relaxed"
              >
                {{ paragraph }}
              </p>
            </NCollapseItem>
          </NCollapse>
        </NCard>
      </NSpin>
    </template>

    <NModal
      v-model:show="exchangeOpen"
      preset="card"
      :title="$t('page.farm.activity.exchangeConfirm')"
      class="w-420px"
      :bordered="false"
    >
      <div class="mb-12px text-13px">
        {{ exchangeGoods?.name || exchangeGoods?.item?.name || exchangeGoods?.id }}
      </div>
      <div class="mb-16px flex-y-center gap-8px">
        <span class="text-13px text-gray-500">{{ $t('page.farm.activity.count') }}</span>
        <NInputNumber v-model:value="exchangeCount" class="w-120px" :min="1" :max="99" />
      </div>
      <div class="flex justify-end gap-8px">
        <NButton @click="exchangeOpen = false">{{ $t('common.cancel') }}</NButton>
        <NButton type="primary" :loading="pendingKey === 'shop'" @click="confirmExchange">
          {{ $t('page.farm.activity.exchange') }}
        </NButton>
      </div>
    </NModal>

    <ActivityRulesDialog
      :open="travelRulesOpen"
      :rules="passRules"
      @close="travelRulesOpen = false"
    />
    <ActivityRulesDialog
      :open="constellationRulesOpen"
      :rules="constellationRules"
      @close="constellationRulesOpen = false"
    />
  </div>
</template>

<style scoped></style>
