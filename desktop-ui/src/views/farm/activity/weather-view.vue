<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import {
  NAvatar,
  NButton,
  NCard,
  NCollapse,
  NCollapseItem,
  NEmpty,
  NInput,
  NProgress,
  NSpace,
  NSpin,
  NTag,
  useMessage
} from 'naive-ui';
import {
  fetchAdvanceWeatherResearch,
  fetchCollectWeather,
  fetchExchangeWeatherCollector,
  fetchGetWeatherFriends,
  fetchGetWeatherSnapshot,
  fetchScanWeatherFriends,
  fetchSummonWeather,
  fetchWeatherMischiefCloud,
  fetchWeatherMischiefFrog
} from '@/service/api';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { useManagedInterval } from '@/hooks/common/use-managed-interval';
import { resolveCatalogImage } from '@/views/farm/game-config/shared';
import { $t } from '@/locales';

defineOptions({ name: 'FarmActivityWeatherView' });

type WeatherItem = {
  id?: string;
  count?: string | number;
  name?: string;
  image?: string;
  rarity?: number;
};

type WeatherStatus = {
  hostGid?: string;
  type?: number;
  status?: number;
  beginTime?: number;
  endTime?: number;
  active?: boolean;
  isThunderstorm?: boolean;
  collectedThisCycle?: boolean;
  remainingSec?: number;
};

type WeatherFriendRow = {
  gid: string;
  name?: string;
  avatarUrl?: string;
  level?: number;
  inspected?: boolean;
  inspectedAt?: number;
  scanError?: string;
  availability?: 'unknown' | 'available' | 'collected' | 'expired' | 'unavailable' | string;
  availabilityReason?: string;
  canCollect?: boolean;
  eligibleCloudLandIds?: string[];
  weather?: WeatherStatus;
};

type WeatherSnapshot = {
  groupId?: string;
  active?: boolean;
  serverTime?: number;
  activity?: { id?: string; name?: string; startTime?: number; endTime?: number };
  rules?: { title?: string; paragraphs?: string[] };
  ownWeather?: WeatherStatus;
  shop?: {
    goodsId?: number;
    item?: WeatherItem;
    cost?: WeatherItem;
    balance?: string | number;
    owned?: boolean;
    statusCode?: number;
    dailyLimit?: number;
    available?: boolean;
    reason?: string;
  } | null;
  collector?: {
    collectorItemId?: number;
    collectorItemCount?: number;
    rewards?: Array<{ id?: string; reward?: WeatherItem; statusCode?: number; probability?: number }>;
  } | null;
  tasks?: Array<{
    id?: string;
    triggerItemId?: string;
    title?: string;
    reward?: WeatherItem;
    dailyLimit?: number;
    current?: number;
    progressKnown?: boolean;
  }>;
  research?: {
    currentStage?: number;
    badgeBalance?: string | number;
    nodes?: Array<{
      id?: string;
      prerequisiteNodeIds?: string[];
      statusCode?: number;
      cost?: WeatherItem;
      reward?: WeatherItem;
      availableByStatus?: boolean;
      completed?: boolean;
      locked?: boolean;
      affordable?: boolean;
    }>;
    nextNode?: Record<string, unknown> | null;
    operateSupported?: boolean;
  } | null;
  inventory?: WeatherItem[];
  actions?: {
    exchangeCollector?: { enabled?: boolean };
    collectWeather?: { enabled?: boolean; dailyLimit?: number };
    scanFriendWeather?: { enabled?: boolean; batchSize?: number; reason?: string };
    frogMischief?: { enabled?: boolean; dailyLimit?: number };
    cloudMischief?: { enabled?: boolean; dailyLimit?: number };
    summonThunderstorm?: { enabled?: boolean; reason?: string };
    advanceResearch?: { enabled?: boolean; nodeId?: string; reason?: string };
  };
} | null;

const FRIEND_SHOW_LIMIT = 60;

const farmAccountStore = useFarmAccountStore();
const message = useMessage();

const loading = ref(false);
const snapshotError = ref('');
const snapshot = ref<WeatherSnapshot>(null);
const snapshotAtSec = ref(0);
const nowSec = ref(Math.floor(Date.now() / 1000));
const pendingKey = ref('');

const friendsLoading = ref(false);
const weatherFriends = ref<WeatherFriendRow[]>([]);
const scannedRows = ref<Record<string, { row: WeatherFriendRow; at: number }>>({});
const scanningGid = ref('');
const selectedGid = ref('');
const searchKeyword = ref('');
const deferredNotice = ref('');
const failedAvatars = ref(new Set<string>());

const clockTimer = useManagedInterval();

const ownWeather = computed(() => snapshot.value?.ownWeather || null);
const shop = computed(() => snapshot.value?.shop || null);
const tasks = computed(() => snapshot.value?.tasks || []);
const research = computed(() => snapshot.value?.research || null);
const researchNodes = computed(() => research.value?.nodes || []);
const inventory = computed(() => snapshot.value?.inventory || []);
const actions = computed(() => snapshot.value?.actions || {});
const rules = computed(() => snapshot.value?.rules || null);

const weatherName = computed(() => {
  const weather = ownWeather.value;
  if (!weather?.active) return '无特殊天气';
  if (weather.isThunderstorm) return '雷雨';
  return `特殊天气（类型 ${weather.type ?? '--'}）`;
});

const ownRemaining = computed(() => {
  const weather = ownWeather.value;
  if (!weather?.active) return 0;
  const base = Number(weather.remainingSec || 0);
  return Math.max(0, base - Math.max(0, nowSec.value - snapshotAtSec.value));
});

const ownRemainingText = computed(() => formatDuration(ownRemaining.value));

const filteredFriends = computed(() => {
  const keyword = searchKeyword.value.trim().toLowerCase();
  const list = weatherFriends.value.filter(friend => Number(friend.gid) > 0);
  const matched = keyword
    ? list.filter(
        friend =>
          String(friend.name || '').toLowerCase().includes(keyword) || String(friend.gid).includes(keyword)
      )
    : list;
  return matched.slice(0, FRIEND_SHOW_LIMIT);
});

const selectedBase = computed(
  () => weatherFriends.value.find(friend => String(friend.gid) === selectedGid.value) || null
);
const selectedScan = computed(() =>
  selectedGid.value ? scannedRows.value[String(selectedGid.value)] || null : null
);
const selectedFriend = computed<WeatherFriendRow | null>(() => {
  if (!selectedBase.value) return null;
  const scanned = selectedScan.value?.row;
  return scanned ? { ...selectedBase.value, ...scanned } : selectedBase.value;
});

const scanDisabled = computed(() => actions.value.scanFriendWeather?.enabled === false);

function formatDuration(sec: number) {
  const value = Math.max(0, Math.floor(sec));
  if (value <= 0) return '';
  const h = Math.floor(value / 3600);
  const m = Math.floor((value % 3600) / 60);
  const s = value % 60;
  return `${h > 0 ? `${h}:` : ''}${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
}

/** 好友天气剩余时间（扫描返回时为基准，随共享时钟递减）。 */
function friendRemainingText(friend: WeatherFriendRow | null) {
  if (!friend?.weather?.active) return '';
  const scanned = scannedRows.value[String(friend.gid)];
  const base = Number(friend.weather.remainingSec || 0);
  const elapsed = scanned ? Math.max(0, nowSec.value - scanned.at) : 0;
  return formatDuration(Math.max(0, base - elapsed)) || '进行中';
}

function stripErrorCode(text?: string) {
  return String(text || '').replace(/^[A-Z_]+：/, '') || '操作失败';
}

function itemImage(item?: WeatherItem) {
  return resolveCatalogImage(item?.image);
}

function itemLabel(item?: WeatherItem) {
  const name = String(item?.name || '').trim();
  if (name && !/^\d+$/.test(name)) return name;
  return `物品 ${item?.id ?? '--'}`;
}

function itemCount(item?: WeatherItem) {
  return item?.count == null ? '' : `×${item.count}`;
}

function rewardSummary(items?: WeatherItem[]) {
  return (items || [])
    .map(item => `${itemLabel(item)}${itemCount(item)}`)
    .filter(Boolean)
    .join('、');
}

function availabilityTag(friend: WeatherFriendRow | null): {
  type: 'success' | 'warning' | 'error' | 'info' | 'default';
  label: string;
} {
  if (!friend) return { type: 'default', label: '未检查' };
  if (friend.scanError) return { type: 'error', label: '读取失败' };
  switch (friend.availability) {
    case 'available':
      return { type: 'success', label: '可采' };
    case 'collected':
      return { type: 'warning', label: '已采' };
    case 'expired':
      return { type: 'default', label: '已失效' };
    case 'unavailable':
      return { type: 'info', label: '晴天' };
    default:
      return { type: 'default', label: '未检查' };
  }
}

function applySnapshot(next: WeatherSnapshot) {
  snapshot.value = next;
  snapshotAtSec.value = Math.floor(Date.now() / 1000);
}

function mergeScannedRow(row: WeatherFriendRow) {
  scannedRows.value = {
    ...scannedRows.value,
    [String(row.gid)]: { row, at: Math.floor(Date.now() / 1000) }
  };
}

/** 写操作返回：合并 friend 行 + 用 snapshot 就地刷新。 */
function applyActionResult(data: Record<string, unknown> | null | undefined, friendKey = 'friend') {
  if (!data) return;
  const friend = data[friendKey] as WeatherFriendRow | undefined;
  if (friend?.gid != null) mergeScannedRow(friend);
  const snap = (data.snapshot || null) as WeatherSnapshot;
  if (snap) applySnapshot(snap);
}

async function loadSnapshot() {
  if (!farmAccountStore.currentAccountId) {
    snapshot.value = null;
    snapshotError.value = '';
    return;
  }
  loading.value = true;
  try {
    const { error, data } = await fetchGetWeatherSnapshot(farmAccountStore.currentAccountId);
    if (error) {
      snapshotError.value = stripErrorCode(error.message);
      snapshot.value = null;
      return;
    }
    snapshotError.value = '';
    applySnapshot((data as WeatherSnapshot) || null);
  } finally {
    loading.value = false;
  }
}

async function loadFriends() {
  if (!farmAccountStore.currentAccountId) {
    weatherFriends.value = [];
    return;
  }
  friendsLoading.value = true;
  try {
    const { error, data } = await fetchGetWeatherFriends(farmAccountStore.currentAccountId);
    if (!error && Array.isArray(data)) {
      weatherFriends.value = (data as WeatherFriendRow[]).filter(friend => Number(friend.gid) > 0);
    }
  } finally {
    friendsLoading.value = false;
  }
}

async function refreshAll() {
  await Promise.all([loadSnapshot(), loadFriends()]);
}

async function scanFriend(friend: WeatherFriendRow) {
  if (!farmAccountStore.currentAccountId || scanDisabled.value) return;
  scanningGid.value = String(friend.gid);
  try {
    const { error, data } = await fetchScanWeatherFriends(farmAccountStore.currentAccountId, [
      String(friend.gid)
    ]);
    if (error) {
      message.error(stripErrorCode(error.message));
      return;
    }
    const rows = (data?.friends || []) as WeatherFriendRow[];
    for (const row of rows) mergeScannedRow(row);
    const deferred = (data?.deferredGids || []) as string[];
    deferredNotice.value = deferred.length ? `有 ${deferred.length} 位好友稍后重试（好友巡查忙）` : '';
    if (deferred.length) {
      message.warning(deferredNotice.value);
    }
  } finally {
    scanningGid.value = '';
  }
}

function chooseFriend(friend: WeatherFriendRow) {
  selectedGid.value = String(friend.gid);
  const scanned = scannedRows.value[String(friend.gid)];
  if (!scanned) void scanFriend(friend);
}

async function summonThunderstorm() {
  if (!farmAccountStore.currentAccountId) return;
  if (actions.value.summonThunderstorm?.enabled === false) return;
  pendingKey.value = 'summon';
  try {
    const { error, data } = await fetchSummonWeather(farmAccountStore.currentAccountId);
    if (error) {
      message.error(stripErrorCode(error.message));
      return;
    }
    const rewards = rewardSummary((data?.use?.rewards || []) as WeatherItem[]);
    message.success(rewards ? `召唤成功：${rewards}` : '雷雨召唤成功');
    applyActionResult(data as Record<string, unknown>);
  } finally {
    pendingKey.value = '';
  }
}

async function exchangeCollector() {
  if (!farmAccountStore.currentAccountId || !shop.value?.available) return;
  pendingKey.value = 'exchange';
  try {
    const { error, data } = await fetchExchangeWeatherCollector(farmAccountStore.currentAccountId);
    if (error) {
      message.error(stripErrorCode(error.message));
      return;
    }
    const rewards = rewardSummary((data?.rewards || []) as WeatherItem[]);
    message.success(
      rewards
        ? $t('page.farm.activity.claimRewardsSuccess', { items: rewards })
        : $t('page.farm.activity.claimSuccess')
    );
    applyActionResult(data as Record<string, unknown>);
  } finally {
    pendingKey.value = '';
  }
}

async function advanceResearch(nodeId?: string) {
  if (!farmAccountStore.currentAccountId || !nodeId) return;
  pendingKey.value = `research:${nodeId}`;
  try {
    const { error, data } = await fetchAdvanceWeatherResearch(farmAccountStore.currentAccountId, nodeId);
    if (error) {
      message.error(stripErrorCode(error.message));
      return;
    }
    const rewards = rewardSummary((data?.rewards || []) as WeatherItem[]);
    message.success(
      rewards
        ? $t('page.farm.activity.claimRewardsSuccess', { items: rewards })
        : $t('page.farm.activity.claimSuccess')
    );
    applyActionResult(data as Record<string, unknown>);
  } finally {
    pendingKey.value = '';
  }
}

async function collectWeather(friend: WeatherFriendRow | null) {
  if (!farmAccountStore.currentAccountId || !friend?.gid || !friend.canCollect) return;
  pendingKey.value = 'collect';
  try {
    const { error, data } = await fetchCollectWeather(farmAccountStore.currentAccountId, String(friend.gid));
    if (error) {
      message.error(stripErrorCode(error.message));
      return;
    }
    const rewards = rewardSummary((data?.rewards || []) as WeatherItem[]);
    message.success(
      rewards
        ? $t('page.farm.activity.claimRewardsSuccess', { items: rewards })
        : $t('page.farm.activity.claimSuccess')
    );
    applyActionResult(data as Record<string, unknown>);
  } finally {
    pendingKey.value = '';
  }
}

async function mischiefFrog(friend: WeatherFriendRow | null) {
  if (!farmAccountStore.currentAccountId || !friend?.gid) return;
  if (actions.value.frogMischief?.enabled === false) return;
  pendingKey.value = 'frog';
  try {
    const { error, data } = await fetchWeatherMischiefFrog(farmAccountStore.currentAccountId, String(friend.gid));
    if (error) {
      message.error(stripErrorCode(error.message));
      return;
    }
    const rewards = rewardSummary((data?.use?.rewards || []) as WeatherItem[]);
    message.success(rewards ? `青蛙使坏成功：${rewards}` : '青蛙使坏成功');
    applyActionResult(data as Record<string, unknown>);
  } finally {
    pendingKey.value = '';
  }
}

async function mischiefCloud(friend: WeatherFriendRow | null) {
  if (!farmAccountStore.currentAccountId || !friend?.gid) return;
  if (actions.value.cloudMischief?.enabled === false) return;
  if (!(friend.eligibleCloudLandIds || []).length) return;
  pendingKey.value = 'cloud';
  try {
    const { error, data } = await fetchWeatherMischiefCloud(
      farmAccountStore.currentAccountId,
      String(friend.gid)
    );
    if (error) {
      message.error(stripErrorCode(error.message));
      return;
    }
    const rewards = rewardSummary((data?.use?.rewards || []) as WeatherItem[]);
    const landId = String(data?.landId || '');
    message.success(
      rewards
        ? `乌云使坏成功${landId ? `（地块 ${landId}）` : ''}：${rewards}`
        : '乌云使坏成功'
    );
    applyActionResult(data as Record<string, unknown>);
  } finally {
    pendingKey.value = '';
  }
}

function friendAvatar(friend: WeatherFriendRow | null) {
  return String(friend?.avatarUrl || '');
}

function showFriendAvatar(friend: WeatherFriendRow | null) {
  return Boolean(friend?.avatarUrl) && !failedAvatars.value.has(String(friend?.gid));
}

function markAvatarFailed(friend: WeatherFriendRow | null) {
  if (friend?.gid == null) return;
  failedAvatars.value = new Set(failedAvatars.value).add(String(friend.gid));
}

function taskProgress(task: { current?: number; dailyLimit?: number }) {
  const current = Number(task.current || 0);
  const limit = Number(task.dailyLimit || 0);
  if (limit <= 0) return 0;
  return Math.min(100, Math.max(0, (current / limit) * 100));
}

watch(
  () => farmAccountStore.currentAccountId,
  () => {
    snapshot.value = null;
    snapshotError.value = '';
    weatherFriends.value = [];
    scannedRows.value = {};
    selectedGid.value = '';
    deferredNotice.value = '';
    void refreshAll();
  }
);

onMounted(() => {
  void refreshAll();
  clockTimer.start(() => {
    nowSec.value = Math.floor(Date.now() / 1000);
  }, 1000);
});
</script>

<template>
  <NCard :bordered="false" size="small" class="card-wrapper">
    <template #header>
      <div class="flex flex-wrap items-center gap-8px">
        <span>{{ snapshot?.activity?.name || '雨落成诗' }}</span>
        <NTag size="small" :type="snapshot?.active ? 'success' : 'default'" :bordered="false">
          {{ snapshot?.active ? '进行中' : '未开放/已结束' }}
        </NTag>
      </div>
    </template>
    <template #header-extra>
      <NButton size="small" :loading="loading" @click="refreshAll">
        {{ $t('common.refresh') }}
      </NButton>
    </template>

    <NSpin :show="loading">
      <NEmpty v-if="snapshotError" class="py-32px" :description="snapshotError" />
      <template v-else>
        <!-- 自己的天气 -->
        <div
          class="mb-16px flex flex-wrap items-center justify-between gap-12px rounded-8px bg-sky-50 px-16px py-14px dark:bg-sky-900/20"
        >
          <div class="flex-y-center gap-12px">
            <span class="text-30px">{{ ownWeather?.active ? (ownWeather.isThunderstorm ? '⛈️' : '🌦️') : '☀️' }}</span>
            <div>
              <div class="text-16px font-medium">{{ weatherName }}</div>
              <div class="mt-2px text-12px text-gray-500">
                <template v-if="ownWeather?.active">
                  剩余 {{ ownRemainingText || '进行中' }}
                  <template v-if="ownWeather.collectedThisCycle"> · 本轮已采</template>
                </template>
                <template v-else>使用雷雨召唤瓶为农场召唤一场雷雨</template>
              </div>
            </div>
          </div>
          <NButton
            type="primary"
            size="small"
            :loading="pendingKey === 'summon'"
            :disabled="actions.summonThunderstorm?.enabled === false"
            :title="actions.summonThunderstorm?.reason || ''"
            @click="summonThunderstorm"
          >
            召唤雷雨
          </NButton>
        </div>

        <!-- 兑换 + 任务 -->
        <div class="mb-16px grid gap-12px lg:grid-cols-2">
          <div class="rounded-8px border border-gray-200 p-14px dark:border-gray-700">
            <div class="mb-10px flex items-center justify-between gap-8px">
              <span class="text-14px font-medium">天气瓶补给</span>
              <NTag v-if="shop?.owned" size="tiny" type="success" :bordered="false">今日已兑换</NTag>
              <NTag v-else-if="shop?.available" size="tiny" type="warning" :bordered="false">可兑换</NTag>
            </div>
            <template v-if="shop">
              <div class="flex-y-center gap-10px">
                <img
                  v-if="itemImage(shop.item)"
                  :src="itemImage(shop.item)"
                  class="h-40px w-40px object-contain"
                  loading="lazy"
                />
                <span v-else class="text-24px opacity-40">🧴</span>
                <div class="min-w-0 flex-1">
                  <div class="truncate text-13px font-medium">
                    {{ itemLabel(shop.item) }}{{ itemCount(shop.item) }}
                  </div>
                  <div class="mt-2px flex-y-center gap-6px text-12px text-amber-600">
                    <img
                      v-if="itemImage(shop.cost)"
                      :src="itemImage(shop.cost)"
                      class="h-16px w-16px object-contain"
                      loading="lazy"
                    />
                    <span>消耗 {{ itemLabel(shop.cost) }} {{ shop.cost?.count ?? '--' }}</span>
                  </div>
                  <div class="mt-2px text-12px text-gray-500">持有 {{ shop.balance ?? '--' }}</div>
                </div>
              </div>
              <NButton
                class="mt-10px"
                block
                size="small"
                type="primary"
                :loading="pendingKey === 'exchange'"
                :disabled="!shop.available || actions.exchangeCollector?.enabled === false"
                @click="exchangeCollector"
              >
                {{ shop.reason && !shop.available ? shop.reason : '兑换采集瓶' }}
              </NButton>
            </template>
            <NEmpty v-else class="py-16px" description="暂未读取到补给目录" />
          </div>

          <div class="rounded-8px border border-gray-200 p-14px dark:border-gray-700">
            <div class="mb-10px flex items-center justify-between gap-8px">
              <span class="text-14px font-medium">活动任务</span>
              <span class="text-12px text-gray-500">雷电徽章 {{ research?.badgeBalance ?? '0' }}</span>
            </div>
            <NEmpty v-if="!tasks.length" class="py-16px" :description="$t('common.noData')" />
            <div v-else class="flex-col gap-10px">
              <div
                v-for="task in tasks"
                :key="String(task.id)"
                class="flex items-center justify-between gap-10px rounded-8px bg-gray-50 px-10px py-8px dark:bg-gray-800"
              >
                <div class="min-w-0 flex-1">
                  <div class="truncate text-13px font-medium" :title="task.title">{{ task.title || `任务 ${task.id}` }}</div>
                  <div class="mt-4px flex-y-center gap-8px">
                    <NProgress
                      type="line"
                      class="max-w-160px"
                      :percentage="taskProgress(task)"
                      :show-indicator="false"
                    />
                    <span class="text-12px text-gray-500">{{ task.current ?? 0 }} / {{ task.dailyLimit ?? '--' }}</span>
                  </div>
                </div>
                <div class="flex shrink-0 items-center gap-6px text-12px">
                  <img
                    v-if="itemImage(task.reward)"
                    :src="itemImage(task.reward)"
                    class="h-24px w-24px object-contain"
                    loading="lazy"
                  />
                  <span>{{ itemLabel(task.reward) }}{{ itemCount(task.reward) }}</span>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- 气象研究 -->
        <div
          v-if="researchNodes.length"
          class="mb-16px rounded-8px border border-gray-200 p-14px dark:border-gray-700"
        >
          <div class="mb-10px flex items-center justify-between gap-8px">
            <span class="text-14px font-medium">气象研究</span>
            <span class="text-12px text-gray-500">
              雷电徽章 {{ research?.badgeBalance ?? '0' }} · 第 {{ research?.currentStage ?? '--' }} 阶段
            </span>
          </div>
          <div class="flex flex-wrap items-stretch gap-8px">
            <template v-for="(node, index) in researchNodes" :key="String(node.id)">
              <div
                class="relative min-w-120px flex-1 rounded-8px border px-10px py-8px dark:border-gray-700"
                :class="{
                  'border-primary bg-primary/5': node.availableByStatus && !node.completed,
                  'opacity-60': node.locked
                }"
              >
                <div class="flex items-center justify-between gap-4px">
                  <span class="text-12px text-gray-500">#{{ node.id }}</span>
                  <NTag v-if="node.completed" size="tiny" type="success" :bordered="false">✓ 已完成</NTag>
                  <NTag v-else-if="node.availableByStatus" size="tiny" type="warning" :bordered="false">可推进</NTag>
                  <NTag v-else size="tiny" :bordered="false">未解锁</NTag>
                </div>
                <div class="mt-6px flex-y-center gap-6px">
                  <img
                    v-if="itemImage(node.reward)"
                    :src="itemImage(node.reward)"
                    class="h-28px w-28px object-contain"
                    loading="lazy"
                  />
                  <span class="text-13px">{{ itemLabel(node.reward) }}{{ itemCount(node.reward) }}</span>
                </div>
                <div class="mt-4px text-12px text-amber-600">
                  {{ itemLabel(node.cost) }} {{ node.cost?.count ?? '--' }}
                </div>
                <NButton
                  v-if="node.availableByStatus && !node.completed"
                  class="mt-8px"
                  size="tiny"
                  type="primary"
                  :loading="pendingKey === `research:${node.id}`"
                  :disabled="!node.affordable"
                  @click="advanceResearch(node.id)"
                >
                  推进
                </NButton>
              </div>
              <span v-if="index < researchNodes.length - 1" class="self-center text-gray-300">→</span>
            </template>
          </div>
          <div v-if="actions.advanceResearch?.reason" class="mt-8px text-12px text-gray-500">
            {{ actions.advanceResearch.reason }}
          </div>
        </div>

        <!-- 背包物品 -->
        <div
          v-if="inventory.length"
          class="mb-16px flex flex-wrap items-center gap-10px rounded-8px border border-gray-200 px-14px py-10px dark:border-gray-700"
        >
          <span class="text-13px text-gray-500">活动物品：</span>
          <div
            v-for="item in inventory"
            :key="String(item.id)"
            class="flex-y-center gap-4px rounded-6px bg-gray-50 px-8px py-4px text-12px dark:bg-gray-800"
            :title="itemLabel(item)"
          >
            <img
              v-if="itemImage(item)"
              :src="itemImage(item)"
              class="h-22px w-22px object-contain"
              loading="lazy"
            />
            <span>{{ itemLabel(item) }}</span>
            <span class="font-medium">×{{ item.count ?? '0' }}</span>
          </div>
        </div>

        <!-- 好友区 -->
        <div class="rounded-8px border border-gray-200 p-14px dark:border-gray-700">
          <div class="mb-10px flex flex-wrap items-center justify-between gap-8px">
            <span class="text-14px font-medium">好友天气采集</span>
            <NSpace :size="6" align="center">
              <span class="text-12px text-gray-500">
                每日上限 {{ actions.collectWeather?.dailyLimit ?? '--' }} 次
              </span>
              <NTag v-if="scanDisabled" size="tiny" type="warning" :bordered="false">
                {{ actions.scanFriendWeather?.reason || '暂不可扫描' }}
              </NTag>
            </NSpace>
          </div>

          <div class="grid gap-12px lg:grid-cols-[minmax(0,1fr)_320px]">
            <div>
              <NInput
                v-model:value="searchKeyword"
                size="small"
                clearable
                class="mb-8px max-w-320px"
                placeholder="搜索好友名称或 GID"
              />
              <NEmpty
                v-if="!filteredFriends.length"
                class="py-24px"
                :description="friendsLoading ? '正在加载好友列表…' : '暂无好友'"
              />
              <div v-else class="grid gap-6px sm:grid-cols-2">
                <button
                  v-for="friend in filteredFriends"
                  :key="String(friend.gid)"
                  type="button"
                  class="flex cursor-pointer items-center gap-10px rounded-8px border border-gray-200 px-10px py-8px text-left transition dark:border-gray-700"
                  :class="{ 'border-primary': selectedGid === String(friend.gid) }"
                  @click="chooseFriend(friend)"
                >
                  <NAvatar
                    v-if="showFriendAvatar(friend)"
                    :src="friendAvatar(friend)"
                    round
                    :size="32"
                    @error="() => markAvatarFailed(friend)"
                  />
                  <NAvatar v-else round :size="32">{{ (friend.name || '?').slice(0, 1) }}</NAvatar>
                  <div class="min-w-0 flex-1">
                    <div class="truncate text-13px font-medium">{{ friend.name || `好友 ${friend.gid}` }}</div>
                    <div class="text-11px text-gray-500">Lv.{{ friend.level || '--' }} · GID {{ friend.gid }}</div>
                  </div>
                  <NSpin v-if="scanningGid === String(friend.gid)" :size="14" />
                  <NTag
                    v-else
                    size="tiny"
                    :type="availabilityTag(scannedRows[String(friend.gid)]?.row || friend).type"
                    :bordered="false"
                  >
                    {{ availabilityTag(scannedRows[String(friend.gid)]?.row || friend).label }}
                  </NTag>
                </button>
              </div>
            </div>

            <!-- 好友详情卡 -->
            <div class="rounded-8px bg-gray-50 p-12px dark:bg-gray-800/60">
              <template v-if="selectedFriend">
                <div class="flex-y-center gap-10px">
                  <NAvatar
                    v-if="showFriendAvatar(selectedFriend)"
                    :src="friendAvatar(selectedFriend)"
                    round
                    :size="40"
                    @error="() => markAvatarFailed(selectedFriend)"
                  />
                  <NAvatar v-else round :size="40">{{ (selectedFriend.name || '?').slice(0, 1) }}</NAvatar>
                  <div class="min-w-0">
                    <div class="truncate text-14px font-medium" :title="selectedFriend.name">
                      {{ selectedFriend.name || `好友 ${selectedFriend.gid}` }}
                    </div>
                    <div class="text-12px text-gray-500">Lv.{{ selectedFriend.level || '--' }} · GID {{ selectedFriend.gid }}</div>
                  </div>
                </div>

                <div class="mt-10px flex-y-center gap-8px">
                  <NTag size="small" :type="availabilityTag(selectedFriend).type" :bordered="false">
                    {{ availabilityTag(selectedFriend).label }}
                  </NTag>
                  <span class="text-12px text-gray-500">
                    {{ friendRemainingText(selectedFriend) ? `雷雨剩余 ${friendRemainingText(selectedFriend)}` : '' }}
                  </span>
                </div>
                <div
                  v-if="selectedFriend.availabilityReason"
                  class="mt-6px text-12px text-gray-500"
                >
                  {{ selectedFriend.availabilityReason }}
                </div>
                <div v-if="selectedFriend.scanError" class="mt-6px text-12px text-red-500">
                  {{ selectedFriend.scanError }}
                </div>

                <div class="mt-12px flex flex-col gap-8px">
                  <NButton
                    type="primary"
                    size="small"
                    block
                    :loading="pendingKey === 'collect'"
                    :disabled="!selectedFriend.canCollect || actions.collectWeather?.enabled === false"
                    @click="collectWeather(selectedFriend)"
                  >
                    采雨
                  </NButton>
                  <div class="grid grid-cols-2 gap-8px">
                    <NButton
                      size="small"
                      type="warning"
                      ghost
                      :loading="pendingKey === 'frog'"
                      :disabled="actions.frogMischief?.enabled === false"
                      @click="mischiefFrog(selectedFriend)"
                    >
                      青蛙使坏
                    </NButton>
                    <NButton
                      size="small"
                      type="warning"
                      ghost
                      :loading="pendingKey === 'cloud'"
                      :disabled="
                        actions.cloudMischief?.enabled === false ||
                        !(selectedFriend.eligibleCloudLandIds || []).length
                      "
                      @click="mischiefCloud(selectedFriend)"
                    >
                      乌云使坏
                    </NButton>
                  </div>
                  <NButton
                    size="tiny"
                    quaternary
                    :loading="scanningGid === String(selectedFriend.gid)"
                    :disabled="scanDisabled"
                    @click="scanFriend(selectedFriend)"
                  >
                    重新读取现场天气
                  </NButton>
                </div>
              </template>
              <div v-else class="flex-col-center gap-8px py-24px text-center text-gray-400">
                <div class="text-28px">🌧️</div>
                <div class="text-13px">点击好友读取现场天气</div>
              </div>
            </div>
          </div>

          <div v-if="deferredNotice" class="mt-10px text-12px text-amber-600">{{ deferredNotice }}</div>
        </div>

        <!-- 活动说明 -->
        <NCollapse v-if="rules?.paragraphs?.length" class="mt-16px">
          <NCollapseItem :title="rules?.title || '活动说明'" name="rules">
            <p
              v-for="(paragraph, index) in rules?.paragraphs"
              :key="index"
              class="mb-8px whitespace-pre-line text-13px leading-relaxed"
            >
              {{ paragraph }}
            </p>
          </NCollapseItem>
        </NCollapse>
      </template>
    </NSpin>
  </NCard>
</template>

<style scoped></style>
