<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import InteractionItemsPanel from '../personal/InteractionItemsPanel.vue';
import LandCountdown from '../shared/LandCountdown.vue';
import {
  NAvatar,
  NButton,
  NCard,
  NEmpty,
  NInput,
  NPagination,
  NPopconfirm,
  NSpace,
  NSpin,
  NTabPane,
  NTabs,
  NTag,
  useMessage
} from 'naive-ui';
import {
  fetchDeleteFarmFriend,
  fetchFarmFriendOp,
  fetchGetFarmAutomationDetail,
  fetchGetFarmFriendInteractRecords,
  fetchGetFarmFriendLands,
  fetchGetFarmFriendList,
  fetchModifyFarmAutomation
} from '@/service/api';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { useFarmWs } from '@/hooks/business/farm-ws';
import { resolveCatalogImage } from '@/views/farm/game-config/shared';
import { formatCareerCount, formatCareerStealRatio } from '@/views/farm/shared/career';
import {
  landCardClass,
  landGridStyle,
  landIdLabel,
  soilLabel,
  soilLevelClass,
  visibleLands
} from '@/views/farm/shared/land-display';
import { $t } from '@/locales';

defineOptions({
  name: 'FarmFriends'
});

type FriendOp = 'steal' | 'help' | 'bad';
type TabKey = 'friends' | 'blacklist' | 'visitors';

const FRIEND_PAGE_SIZE = 25;

const farmAccountStore = useFarmAccountStore();
const message = useMessage();

const activeTab = ref<TabKey>('friends');
const loading = ref(false);
const interactLoading = ref(false);
const interactError = ref('');
const opLoadingKey = ref<string | null>(null);
const stealAllLoading = ref(false);
const blacklistLoading = ref(false);
const deletingGid = ref<number | null>(null);
const friends = ref<Api.Farm.Friend[]>([]);
const friendCareers = ref<Record<number, Api.Farm.Career | null>>({});
const friendBlacklist = ref<number[]>([]);
const interactRecords = ref<Api.Farm.FriendInteractRecord[]>([]);
const interactFilter = ref<'all' | 'steal' | 'help' | 'bad'>('all');
const searchKeyword = ref('');
const friendPage = ref(1);
const expandedGid = ref<number | null>(null);
const friendLands = ref<Record<number, Api.Farm.LandRow[]>>({});
const friendLandsLoading = ref<Record<number, boolean>>({});
const avatarErrorKeys = ref<Set<number>>(new Set());
const interactAvatarErrors = ref<Set<string>>(new Set());

const interactFilters: { key: 'all' | 'steal' | 'help' | 'bad'; labelKey: App.I18n.I18nKey }[] = [
  { key: 'all', labelKey: 'page.farm.friends.filterAll' },
  { key: 'steal', labelKey: 'page.farm.friends.filterSteal' },
  { key: 'help', labelKey: 'page.farm.friends.filterHelp' },
  { key: 'bad', labelKey: 'page.farm.friends.filterBad' }
];

function opKey(gid: number, op: string) {
  return `${gid}:${op}`;
}

function friendStatusText(friend: Api.Farm.Friend) {
  const plant = friend.plant || {};
  const parts: string[] = [];
  if (plant.stealNum) parts.push(`偷${plant.stealNum}`);
  if (plant.dryNum) parts.push(`水${plant.dryNum}`);
  if (plant.weedNum) parts.push(`草${plant.weedNum}`);
  if (plant.insectNum) parts.push(`虫${plant.insectNum}`);
  return parts.length ? parts.join(' ') : $t('page.farm.friends.noAction');
}

function hasActionStatus(friend: Api.Farm.Friend) {
  return friendStatusText(friend) !== $t('page.farm.friends.noAction');
}

function canStealFriend(friend: Api.Farm.Friend) {
  return Number(friend.plant?.stealNum || 0) > 0;
}

function canHelpFriend(friend: Api.Farm.Friend) {
  const plant = friend.plant || {};
  return Number(plant.dryNum || 0) > 0 || Number(plant.weedNum || 0) > 0 || Number(plant.insectNum || 0) > 0;
}

function isBlacklisted(gid: number) {
  return friendBlacklist.value.includes(Number(gid));
}

function formatFriendGold(value: unknown) {
  const gold = Number.parseInt(String(value ?? ''), 10);
  if (!Number.isFinite(gold) || gold < 0) return '0';
  return gold.toLocaleString('zh-CN');
}

function friendActionRank(friend: Api.Farm.Friend) {
  const steal = Number(friend.plant?.stealNum || 0);
  const help =
    Number(friend.plant?.dryNum || 0) + Number(friend.plant?.weedNum || 0) + Number(friend.plant?.insectNum || 0);
  return { steal, help };
}

const sortedFriends = computed(() =>
  [...friends.value].sort((a, b) => {
    const ar = friendActionRank(a);
    const br = friendActionRank(b);
    if (br.steal !== ar.steal) return br.steal - ar.steal;
    if (br.help !== ar.help) return br.help - ar.help;
    const levelDiff = Number(b.level || 0) - Number(a.level || 0);
    if (levelDiff !== 0) return levelDiff;
    return Number(a.gid || 0) - Number(b.gid || 0);
  })
);

const filteredFriends = computed(() => {
  const keyword = searchKeyword.value.trim().toLowerCase();
  const list = sortedFriends.value;
  if (!keyword) return list;
  return list.filter(friend => {
    const name = String(friend.nickname || '').toLowerCase();
    const gid = String(friend.gid || '');
    return name.includes(keyword) || gid.includes(keyword);
  });
});

const normalFriends = computed(() => filteredFriends.value.filter(friend => !isBlacklisted(friend.gid)));

const friendTotalPages = computed(() => Math.ceil(normalFriends.value.length / FRIEND_PAGE_SIZE) || 1);

const pagedNormalFriends = computed(() => {
  const start = (friendPage.value - 1) * FRIEND_PAGE_SIZE;
  return normalFriends.value.slice(start, start + FRIEND_PAGE_SIZE);
});

const friendPageRange = computed(() => {
  const total = normalFriends.value.length;
  if (!total) return { start: 0, end: 0, total };
  const start = (friendPage.value - 1) * FRIEND_PAGE_SIZE + 1;
  const end = Math.min(total, friendPage.value * FRIEND_PAGE_SIZE);
  return { start, end, total };
});

watch(searchKeyword, () => {
  friendPage.value = 1;
});

watch(friendTotalPages, total => {
  if (friendPage.value > total) friendPage.value = total;
});

const stealableFriends = computed(() => normalFriends.value.filter(friend => canStealFriend(friend)));

/** 宠物状态今日已确认数（other 含「没有上场狗」这一结论） */
const petKnownCount = computed(
  () => friends.value.filter(friend => friend.petState === 'protect' || friend.petState === 'other').length
);

const blacklistFriends = computed(() => {
  const byGid = new Map(friends.value.map(f => [Number(f.gid), f]));
  return friendBlacklist.value.map(gid => {
    const friend = byGid.get(Number(gid));
    return (
      friend ||
      ({
        accountId: farmAccountStore.currentAccountId || 0,
        gid,
        nickname: `GID:${gid}`
      } as Api.Farm.Friend)
    );
  });
});

const filteredBlacklistFriends = computed(() => {
  const keyword = searchKeyword.value.trim().toLowerCase();
  if (!keyword) return blacklistFriends.value;
  return blacklistFriends.value.filter(friend => {
    const name = String(friend.nickname || '').toLowerCase();
    const gid = String(friend.gid || '');
    return name.includes(keyword) || gid.includes(keyword);
  });
});

const visibleInteractRecords = computed(() => {
  const list = interactRecords.value || [];
  if (interactFilter.value === 'all') return list;
  const typeMap = { steal: 1, help: 2, bad: 3 } as const;
  const want = typeMap[interactFilter.value];
  return list.filter(item => Number(item.actionType) === want);
});

async function loadBlacklist() {
  if (!farmAccountStore.currentAccountId) {
    friendBlacklist.value = [];
    return;
  }
  const { error, data } = await fetchGetFarmAutomationDetail(farmAccountStore.currentAccountId);
  if (!error && data) {
    friendBlacklist.value = (data.friendBlacklist || []).map(Number).filter(Boolean);
  }
}

async function loadFriends(opts?: { force?: boolean }) {
  if (!farmAccountStore.currentAccountId) {
    friends.value = [];
    return;
  }
  loading.value = true;
  try {
    const [{ error, data }] = await Promise.all([
      fetchGetFarmFriendList({
        current: 1,
        size: 500,
        accountId: farmAccountStore.currentAccountId,
        force: Boolean(opts?.force)
      }),
      loadBlacklist()
    ]);
    if (!error && data) {
      friends.value = data.records || [];
    }
  } finally {
    loading.value = false;
  }
}

/** Manual refresh: bust server cache and reload steal bubbles. */
async function refreshFriendList() {
  await loadFriends({ force: true });
  if (expandedGid.value) {
    await loadFriendLands(expandedGid.value);
  }
}

async function loadInteractRecords() {
  if (!farmAccountStore.currentAccountId) {
    interactRecords.value = [];
    interactError.value = '';
    return;
  }
  interactLoading.value = true;
  interactError.value = '';
  try {
    const { error, data } = await fetchGetFarmFriendInteractRecords(farmAccountStore.currentAccountId);
    if (error) {
      interactRecords.value = [];
      interactError.value = error.message || $t('page.farm.friends.visitorsFailed');
      return;
    }
    interactRecords.value = data || [];
  } finally {
    interactLoading.value = false;
  }
}

function buildPlantSummaryFromLands(lands: Api.Farm.LandRow[]) {
  let stealNum = 0;
  let dryNum = 0;
  let weedNum = 0;
  let insectNum = 0;
  for (const land of lands) {
    if (!land?.unlocked) continue;
    if (land.status === 'stealable') stealNum += 1;
    if (land.needWater) dryNum += 1;
    if (land.needWeed) weedNum += 1;
    if (land.needBug) insectNum += 1;
  }
  return { stealNum, dryNum, weedNum, insectNum };
}

function syncFriendPlantFromLands(gid: number, lands: Api.Farm.LandRow[]) {
  const idx = friends.value.findIndex(f => f.gid === gid);
  if (idx < 0) return;
  friends.value[idx] = {
    ...friends.value[idx]!,
    plant: buildPlantSummaryFromLands(lands)
  };
}

function applyOpOptimisticPlant(gid: number, op: FriendOp) {
  const idx = friends.value.findIndex(f => f.gid === gid);
  if (idx < 0) return;
  const prev = friends.value[idx]!.plant || {};
  const next = { ...prev };
  if (op === 'help') {
    next.dryNum = 0;
    next.weedNum = 0;
    next.insectNum = 0;
  } else if (op === 'steal') {
    next.stealNum = 0;
  }
  friends.value[idx] = { ...friends.value[idx]!, plant: next };
}

async function loadFriendLands(gid: number) {
  if (!farmAccountStore.currentAccountId) return;
  friendLandsLoading.value = { ...friendLandsLoading.value, [gid]: true };
  try {
    const { error, data } = await fetchGetFarmFriendLands({
      accountId: farmAccountStore.currentAccountId,
      gid
    });
    if (error) {
      message.error(error.message || $t('page.farm.friends.opFailed'));
      friendLands.value = { ...friendLands.value, [gid]: [] };
      friendCareers.value = { ...friendCareers.value, [gid]: null };
      return;
    }
    const lands = data?.lands || [];
    const now = Math.floor(Date.now() / 1000);
    friendLands.value = {
      ...friendLands.value,
      [gid]: (lands || []).map((land: Api.Farm.LandRow) => ({ ...land, matureAt: now + Number(land.matureInSec || 0) }))
    };
    friendCareers.value = { ...friendCareers.value, [gid]: data?.career || null };
    syncFriendPlantFromLands(gid, lands);
  } finally {
    friendLandsLoading.value = { ...friendLandsLoading.value, [gid]: false };
  }
}

async function toggleFriend(gid: number) {
  if (expandedGid.value === gid) {
    expandedGid.value = null;
    return;
  }
  expandedGid.value = gid;
  await loadFriendLands(gid);
}

async function runFriendOp(
  friend: Api.Farm.Friend,
  op: FriendOp,
  event?: MouseEvent,
  options?: { quiet?: boolean; onData?: (data: Record<string, unknown>) => void }
): Promise<boolean> {
  event?.stopPropagation();
  if (!farmAccountStore.currentAccountId) return false;
  const quiet = !!options?.quiet;
  const key = opKey(friend.gid, op);
  opLoadingKey.value = key;
  try {
    const { error, data } = await fetchFarmFriendOp({
      accountId: farmAccountStore.currentAccountId,
      gid: friend.gid,
      op
    });
    if (error) {
      if (!quiet) message.error(error.message || $t('page.farm.friends.opFailed'));
      return false;
    }
    const count = Number(data?.count || 0);
    if (count > 0) {
      options?.onData?.(data as Record<string, unknown>);
      if (!quiet) {
        const summary = String(data?.summary || data?.helpSummary || '').trim();
        message.success(summary || $t('page.farm.friends.opSuccess'));
      }
      applyOpOptimisticPlant(friend.gid, op);
      if (expandedGid.value === friend.gid) {
        await loadFriendLands(friend.gid);
      }
      return true;
    }
    if (!quiet) {
      if (op === 'steal') {
        message.info($t('page.farm.friends.opNoStealable'));
      } else {
        message.info($t('page.farm.friends.opNothing'));
      }
    }
    applyOpOptimisticPlant(friend.gid, op);
    if (expandedGid.value === friend.gid) {
      await loadFriendLands(friend.gid);
    }
    return false;
  } finally {
    if (opLoadingKey.value === key) opLoadingKey.value = null;
  }
}

async function stealAllFriends() {
  if (!farmAccountStore.currentAccountId || stealAllLoading.value) return;
  const targets = [...stealableFriends.value];
  if (!targets.length) {
    message.info($t('page.farm.friends.stealAllEmpty'));
    return;
  }
  stealAllLoading.value = true;
  let ok = 0;
  let skip = 0;
  const totals = new Map<string, number>();
  try {
    for (const friend of targets) {
      const stolen = await runFriendOp(friend, 'steal', undefined, {
        quiet: true,
        onData: data => {
          const items = Array.isArray(data.items) ? (data.items as Array<{ name?: string; count?: number }>) : [];
          for (const item of items) {
            const name = String(item?.name || '').trim();
            if (!name) continue;
            totals.set(name, (totals.get(name) || 0) + Number(item.count || 0));
          }
        }
      });
      if (stolen) ok += 1;
      else skip += 1;
    }
    const done = $t('page.farm.friends.stealAllDone', { ok, skip });
    const detail = [...totals.entries()].map(([name, n]) => `${name}×${n}`).join('、');
    message.success(detail ? `${done}：${detail}` : done);
  } finally {
    stealAllLoading.value = false;
  }
}

async function toggleBlacklist(friend: Api.Farm.Friend, event?: MouseEvent) {
  event?.stopPropagation();
  if (!farmAccountStore.currentAccountId) return;
  blacklistLoading.value = true;
  try {
    const gid = Number(friend.gid);
    const next = isBlacklisted(gid) ? friendBlacklist.value.filter(id => id !== gid) : [...friendBlacklist.value, gid];
    const { error } = await fetchModifyFarmAutomation({
      accountId: farmAccountStore.currentAccountId,
      friendBlacklist: next
    });
    if (error) {
      message.error(error.message || $t('page.farm.friends.blacklistFailed'));
      return;
    }
    friendBlacklist.value = next;
    message.success($t('page.farm.friends.blacklistSuccess'));
  } finally {
    blacklistLoading.value = false;
  }
}

/** 游戏内删除好友（后端会同时加入黑名单，不再自动互动）。 */
async function deleteFriend(friend: Api.Farm.Friend) {
  if (!farmAccountStore.currentAccountId) return;
  deletingGid.value = Number(friend.gid);
  try {
    const { error } = await fetchDeleteFarmFriend(farmAccountStore.currentAccountId, friend.gid);
    if (error) {
      message.error(error.message || $t('page.farm.friends.deleteFailed'));
      return;
    }
    friends.value = friends.value.filter(item => Number(item.gid) !== Number(friend.gid));
    if (expandedGid.value === Number(friend.gid)) expandedGid.value = null;
    message.success($t('page.farm.friends.deleteSuccess', { name: friend.nickname || friend.gid }));
    await loadBlacklist();
  } finally {
    deletingGid.value = null;
  }
}

/**
 * 好友宠物徽标（后端 petState / pet 字段缺失时不展示）。
 * `unknown`（今天还没同步到）不展示徽标：每日宠物同步在后台按节奏补齐，
 * 逐行显示「待确认」只是噪音；整体进度见工具栏的同步提示。
 */
function petBadge(friend: Api.Farm.Friend): { kind: 'protect' | 'name'; label: string } | null {
  const petName = String(friend.pet?.name || '').trim();
  if (friend.petState === 'protect') return { kind: 'protect', label: '护主犬' };
  if (friend.petState === 'other') {
    if (petName) return { kind: 'name', label: petName };
    if (friend.pet) return { kind: 'name', label: '宠物' };
  }
  return null;
}

function landImageSrc(land: Api.Farm.LandRow) {
  return resolveCatalogImage(land.seedImage);
}

function displayFriendLands(gid: number) {
  return visibleLands(friendLands.value[gid] || []);
}

function landStatusLabel(land: Api.Farm.LandRow) {
  if (land.status === 'stealable') return $t('page.farm.personal.statusStealable');
  if (land.status === 'harvestable' || land.status === 'harvested') return $t('page.farm.personal.statusHarvestable');
  if (land.status === 'dead') return $t('page.farm.personal.statusDead');
  if (land.status === 'empty') return $t('page.farm.personal.statusEmpty');
  if (land.status === 'locked') return $t('page.farm.personal.statusLocked');
  return $t('page.farm.personal.statusGrowing');
}

function canShowAvatar(friend: Api.Farm.Friend) {
  return Boolean(friend.avatar) && !avatarErrorKeys.value.has(friend.gid);
}

function handleAvatarError(friend: Api.Farm.Friend) {
  avatarErrorKeys.value = new Set(avatarErrorKeys.value).add(friend.gid);
}

function canShowInteractAvatar(record: Api.Farm.FriendInteractRecord) {
  const key = String(record.key || record.visitorGid || '');
  return Boolean(record.avatarUrl) && key !== '' && !interactAvatarErrors.value.has(key);
}

function handleInteractAvatarError(record: Api.Farm.FriendInteractRecord) {
  const key = String(record.key || record.visitorGid || '');
  if (!key) return;
  interactAvatarErrors.value = new Set(interactAvatarErrors.value).add(key);
}

function interactBadgeType(actionType?: number): 'info' | 'success' | 'error' | 'default' {
  if (Number(actionType) === 1) return 'info';
  if (Number(actionType) === 2) return 'success';
  if (Number(actionType) === 3) return 'error';
  return 'default';
}

function formatInteractTime(timestamp?: number) {
  const ts = Number(timestamp) || 0;
  if (!ts) return '--';
  const date = new Date(ts);
  const now = new Date();
  const diff = now.getTime() - date.getTime();
  const minute = 60 * 1000;
  const hour = 60 * minute;
  if (diff >= 0 && diff < minute) return $t('page.farm.friends.justNow');
  if (diff >= minute && diff < hour) {
    return $t('page.farm.friends.minutesAgo', { n: Math.floor(diff / minute) });
  }
  const sameDay =
    now.getFullYear() === date.getFullYear() && now.getMonth() === date.getMonth() && now.getDate() === date.getDate();
  if (sameDay) {
    return `${$t('page.farm.friends.today')} ${date.toLocaleTimeString('zh-CN', {
      hour: '2-digit',
      minute: '2-digit'
    })}`;
  }
  return date.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit'
  });
}

// 倒计时由 LandCountdown 共享时钟渲染，不再整表重建
function startTick() {}

function stopTick() {}

watch(activeTab, tab => {
  if (tab === 'visitors') void loadInteractRecords();
});

watch(
  () => farmAccountStore.currentAccountId,
  async () => {
    expandedGid.value = null;
    friendLands.value = {};
    friendPage.value = 1;
    await loadFriends();
    if (activeTab.value === 'visitors') await loadInteractRecords();
  }
);

function patchFriendPlant(gid: number, plant: Partial<NonNullable<Api.Farm.Friend['plant']>>) {
  if (!gid) return;
  const idx = friends.value.findIndex(f => f.gid === gid);
  if (idx < 0) return;
  const prev = friends.value[idx]!.plant || {};
  friends.value[idx] = {
    ...friends.value[idx]!,
    plant: {
      stealNum: Number(plant.stealNum ?? prev.stealNum ?? 0),
      dryNum: Number(plant.dryNum ?? prev.dryNum ?? 0),
      weedNum: Number(plant.weedNum ?? prev.weedNum ?? 0),
      insectNum: Number(plant.insectNum ?? prev.insectNum ?? 0)
    }
  };
}

let listRefreshTimer: ReturnType<typeof setTimeout> | null = null;
function scheduleFriendListRefresh(gid?: number) {
  if (gid && gid > 0) {
    applyOpOptimisticPlant(gid, 'steal');
  }
  if (listRefreshTimer) clearTimeout(listRefreshTimer);
  listRefreshTimer = setTimeout(() => {
    listRefreshTimer = null;
    void loadFriends({ force: false });
    if (expandedGid.value) void loadFriendLands(expandedGid.value);
  }, 800);
}

useFarmWs({
  onMessage(type, payload, raw) {
    const body = (payload && typeof payload === 'object' ? payload : {}) as Record<string, unknown>;
    const accountId = Number(body.accountId || raw?.accountId || 0);
    const current = farmAccountStore.currentAccountId;
    if (current && accountId && accountId !== current) return;

    if (type === 'friend_plant') {
      const gid = Number(body.friendGid || body.gid || 0);
      const plant = (body.plant && typeof body.plant === 'object' ? body.plant : body) as Record<string, unknown>;
      patchFriendPlant(gid, {
        stealNum: Number(plant.stealNum ?? body.stealNum ?? 0),
        dryNum: Number(plant.dryNum ?? body.dryNum ?? 0),
        weedNum: Number(plant.weedNum ?? body.weedNum ?? 0),
        insectNum: Number(plant.insectNum ?? body.insectNum ?? 0)
      });
      return;
    }

    if (type === 'friend_interact') {
      const action = String(body.action || body.event || '');
      const result = String(body.result || '');
      const gid = Number(body.targetGid || body.friendGid || body.gid || 0);
      if ((action.includes('steal') || action === '偷菜') && result !== 'error') {
        scheduleFriendListRefresh(gid);
      }
      return;
    }

    if (
      type === 'log' ||
      type === 'log:new' ||
      type === 'account_log' ||
      type === 'account-log:new' ||
      type === 'worker_log'
    ) {
      const msg = String(body.message || '');
      const event = String(body.event || '');
      const gid = Number(body.friendGid || body.targetGid || 0);
      if (event === 'visit_friend' || /偷\d/.test(msg) || msg.includes('偷菜')) {
        scheduleFriendListRefresh(gid);
      }
    }
  }
});

onMounted(async () => {
  if (!farmAccountStore.accounts.length) {
    await farmAccountStore.loadAccounts();
  }
  await loadFriends();
  startTick();
});

onUnmounted(() => {
  stopTick();
  if (listRefreshTimer) clearTimeout(listRefreshTimer);
});
</script>

<template>
  <div class="min-h-500px flex-col-stretch gap-16px overflow-auto">
    <NEmpty
      v-if="!farmAccountStore.currentAccountId"
      class="py-48px"
      :description="$t('page.farm.common.selectAccount')"
    />

    <NTabs v-else v-model:value="activeTab" type="line" animated>
      <NTabPane name="friends" :tab="$t('page.farm.friends.tabFriends')">
        <NCard :bordered="false" size="small" class="card-wrapper">
          <div class="mb-12px flex flex-wrap items-center justify-between gap-12px">
            <NInput
              v-model:value="searchKeyword"
              clearable
              size="small"
              class="max-w-320px"
              :placeholder="$t('page.farm.friends.searchPlaceholder')"
            />
            <NSpace align="center">
              <span class="text-12px text-gray-500">
                {{
                  $t('page.farm.friends.friendPageCount', {
                    start: friendPageRange.start,
                    end: friendPageRange.end,
                    total: friends.length
                  })
                }}
              </span>
              <NPagination
                v-if="friendTotalPages > 1"
                v-model:page="friendPage"
                :page-count="friendTotalPages"
                size="small"
                :page-slot="5"
              />
              <NButton
                size="small"
                type="primary"
                ghost
                :loading="stealAllLoading"
                :disabled="!stealableFriends.length"
                @click="stealAllFriends"
              >
                {{ $t('page.farm.friends.stealAll') }}
              </NButton>
              <NButton size="small" :loading="loading" @click="refreshFriendList">
                {{ $t('page.farm.friends.refreshList') }}
              </NButton>
            </NSpace>
          </div>

          <div v-if="friends.length && petKnownCount < friends.length" class="mb-8px text-12px text-gray-400">
            {{
              $t('page.farm.friends.petSyncProgress', {
                known: petKnownCount,
                total: friends.length
              })
            }}
          </div>

          <NSpin :show="loading">
            <NEmpty v-if="!normalFriends.length" class="py-32px" :description="$t('common.noData')" />
            <div v-else class="flex-col gap-12px">
              <div
                v-for="friend in pagedNormalFriends"
                :key="friend.gid"
                class="overflow-hidden border border-gray-200 rounded-8px dark:border-gray-700"
              >
                <div
                  class="flex cursor-pointer flex-col gap-12px p-12px transition hover:bg-gray-50 dark:hover:bg-gray-800/50 sm:flex-row sm:items-center sm:justify-between"
                  @click="toggleFriend(friend.gid)"
                >
                  <div class="flex-y-center gap-12px">
                    <NAvatar
                      v-if="canShowAvatar(friend)"
                      :src="friend.avatar"
                      round
                      :size="40"
                      @error="() => handleAvatarError(friend)"
                    />
                    <NAvatar v-else round :size="40">{{ (friend.nickname || '?').slice(0, 1) }}</NAvatar>
                    <div>
                      <div class="flex-y-center gap-8px font-medium">
                        <span>{{ friend.nickname || friend.gid }}</span>
                        <span class="text-12px text-gray-400">({{ friend.gid }})</span>
                      </div>
                      <div class="mt-4px flex flex-wrap items-center gap-8px text-12px">
                        <NTag v-if="friend.level" size="tiny" :bordered="false">Lv{{ friend.level }}</NTag>
                        <NTag v-if="petBadge(friend)?.kind === 'protect'" size="tiny" type="success" :bordered="false">
                          🐕 {{ petBadge(friend)?.label }}
                        </NTag>
                        <span
                          v-else-if="petBadge(friend)?.kind === 'name'"
                          class="flex-y-center gap-4px rounded-4px bg-gray-100 px-6px py-2px text-gray-600 dark:bg-gray-800 dark:text-gray-300"
                        >
                          <img
                            v-if="resolveCatalogImage(friend.pet?.image)"
                            :src="resolveCatalogImage(friend.pet?.image)"
                            class="h-14px w-14px object-contain"
                            loading="lazy"
                          />
                          {{ petBadge(friend)?.label }}
                        </span>
                        <span
                          class="rounded-4px bg-amber-50 px-6px py-2px text-amber-700 dark:bg-amber-900/20 dark:text-amber-300"
                        >
                          {{ $t('page.farm.friends.gold') }} {{ formatFriendGold(friend.gold) }}
                        </span>
                      </div>
                      <div
                        class="mt-4px text-12px"
                        :class="hasActionStatus(friend) ? 'text-green-500 font-medium' : 'text-gray-400'"
                      >
                        {{ friendStatusText(friend) }}
                      </div>
                    </div>
                  </div>

                  <div class="flex flex-wrap gap-8px" @click.stop>
                    <NButton
                      v-if="canStealFriend(friend)"
                      size="small"
                      type="primary"
                      ghost
                      :loading="opLoadingKey === opKey(friend.gid, 'steal')"
                      @click="runFriendOp(friend, 'steal', $event)"
                    >
                      {{ $t('page.farm.friends.steal') }}
                    </NButton>
                    <NButton
                      v-if="canHelpFriend(friend)"
                      size="small"
                      type="info"
                      ghost
                      :loading="opLoadingKey === opKey(friend.gid, 'help')"
                      @click="runFriendOp(friend, 'help', $event)"
                    >
                      {{ $t('page.farm.friends.help') }}
                    </NButton>
                    <NPopconfirm @positive-click="runFriendOp(friend, 'bad')">
                      <template #trigger>
                        <NButton size="small" type="warning" ghost :loading="opLoadingKey === opKey(friend.gid, 'bad')">
                          {{ $t('page.farm.friends.bad') }}
                        </NButton>
                      </template>
                      {{ $t('page.farm.friends.opConfirm') }}
                    </NPopconfirm>
                    <NPopconfirm @positive-click="toggleBlacklist(friend)">
                      <template #trigger>
                        <NButton size="small" quaternary :loading="blacklistLoading">
                          {{ $t('page.farm.friends.addBlacklist') }}
                        </NButton>
                      </template>
                      {{ $t('page.farm.friends.blacklistConfirm', { name: friend.nickname || friend.gid }) }}
                    </NPopconfirm>
                    <NPopconfirm @positive-click="deleteFriend(friend)">
                      <template #trigger>
                        <NButton size="small" type="error" quaternary :loading="deletingGid === Number(friend.gid)">
                          {{ $t('page.farm.friends.deleteFriend') }}
                        </NButton>
                      </template>
                      {{ $t('page.farm.friends.deleteConfirm') }}
                    </NPopconfirm>
                  </div>
                </div>

                <div
                  v-if="expandedGid === friend.gid"
                  class="border-t border-gray-200 bg-gray-50 p-12px dark:border-gray-700 dark:bg-gray-900/40"
                >
                  <div
                    v-if="friendCareers[friend.gid]"
                    class="mb-10px flex flex-wrap items-center gap-12px rounded-8px bg-white px-12px py-8px text-13px dark:bg-gray-800/60"
                  >
                    <span class="text-gray-500">{{ $t('page.farm.personal.careerTitle') }}</span>
                    <span>
                      {{ $t('page.farm.personal.careerHarvest') }}
                      <strong class="font-semibold">{{ formatCareerCount(friendCareers[friend.gid]?.harvest) }}</strong>
                    </span>
                    <span>
                      {{ $t('page.farm.personal.careerSteal') }}
                      <strong class="font-semibold">{{ formatCareerCount(friendCareers[friend.gid]?.steal) }}</strong>
                    </span>
                    <span>
                      {{ $t('page.farm.personal.careerRatio') }}
                      <strong class="font-semibold">{{ formatCareerStealRatio(friendCareers[friend.gid]) }}</strong>
                    </span>
                  </div>
                  <NSpin :show="friendLandsLoading[friend.gid]">
                    <NEmpty
                      v-if="!friendLandsLoading[friend.gid] && !displayFriendLands(friend.gid).length"
                      class="py-16px"
                      :description="$t('page.farm.friends.noLands')"
                    />
                    <div v-else class="farm-land-grid is-compact">
                      <div
                        v-for="land in displayFriendLands(friend.gid)"
                        :key="land.id"
                        class="cv-auto"
                        :class="[landCardClass(land, { compact: true })]"
                        :style="landGridStyle(land)"
                      >
                        <div class="flex-y-center justify-between gap-4px">
                          <span class="text-12px opacity-50">{{ landIdLabel(land) }}</span>
                          <NTag size="tiny" :bordered="false">{{ landStatusLabel(land) }}</NTag>
                        </div>
                        <div class="farm-land-crop flex-center min-h-44px">
                          <img
                            v-if="landImageSrc(land)"
                            :src="landImageSrc(land)"
                            :alt="land.plantName"
                            class="max-h-44px max-w-full object-contain"
                            loading="lazy"
                            referrerpolicy="no-referrer"
                          />
                          <span v-else class="text-20px opacity-40">🌱</span>
                        </div>
                        <div class="truncate text-center text-12px font-medium" :title="land.plantName">
                          {{ land.plantName || '-' }}
                        </div>
                        <LandCountdown
                          :at="land.matureAt || 0"
                          :total="land.totalGrowTime || 0"
                          :level="land.level"
                          :phase="land.phaseName"
                        />
                        <div class="flex-center flex-wrap gap-4px">
                          <span
                            v-if="soilLabel(land.level)"
                            class="farm-soil-badge"
                            :class="soilLevelClass(land.level)"
                          >
                            {{ soilLabel(land.level) }}
                          </span>
                          <NTag v-if="land.totalSeason" size="tiny" type="info" :bordered="false">
                            {{
                              $t('page.farm.personal.seasonBadge', {
                                current: land.currentSeason || 1,
                                total: land.totalSeason
                              })
                            }}
                          </NTag>
                          <NTag v-if="land.needWater" size="tiny" type="info">水</NTag>
                          <NTag v-if="land.needWeed" size="tiny" type="success">草</NTag>
                          <NTag v-if="land.needBug" size="tiny" type="error">虫</NTag>
                          <NTag v-if="land.status === 'stealable'" size="tiny" type="warning">可偷</NTag>
                        </div>
                      </div>
                    </div>
                  </NSpin>
                </div>
              </div>
              <div v-if="friendTotalPages > 1" class="mt-4px flex flex-wrap items-center justify-center gap-12px">
                <NPagination v-model:page="friendPage" :page-count="friendTotalPages" size="small" :page-slot="5" />
              </div>
            </div>
          </NSpin>
        </NCard>
      </NTabPane>

      <NTabPane name="interaction" :tab="$t('page.farm.personal.tabInteraction')">
        <InteractionItemsPanel mode="friend" />
      </NTabPane>

      <NTabPane name="blacklist">
        <template #tab>
          <NSpace :size="6" align="center">
            <span>{{ $t('page.farm.friends.tabBlacklist') }}</span>
            <NTag v-if="friendBlacklist.length" size="tiny" type="error" :bordered="false" round>
              {{ friendBlacklist.length }}
            </NTag>
          </NSpace>
        </template>

        <NCard :bordered="false" size="small" class="card-wrapper">
          <div class="mb-12px text-12px text-gray-500">{{ $t('page.farm.friends.blacklistHint') }}</div>
          <NEmpty
            v-if="!filteredBlacklistFriends.length"
            class="py-32px"
            :description="$t('page.farm.friends.blacklistEmpty')"
          />
          <div v-else class="flex-col gap-12px">
            <div
              v-for="friend in filteredBlacklistFriends"
              :key="friend.gid"
              class="flex items-center justify-between rounded-8px border border-gray-200 px-12px py-10px dark:border-gray-700"
            >
              <div class="flex-y-center gap-12px">
                <NAvatar
                  v-if="canShowAvatar(friend)"
                  :src="friend.avatar"
                  round
                  :size="40"
                  @error="() => handleAvatarError(friend)"
                />
                <NAvatar v-else round :size="40">{{ (friend.nickname || '?').slice(0, 1) }}</NAvatar>
                <div>
                  <div class="font-medium">
                    {{ friend.nickname || friend.gid }}
                    <span class="ml-6px text-12px text-gray-400">({{ friend.gid }})</span>
                  </div>
                  <div v-if="friend.level || friend.gold" class="mt-4px flex flex-wrap gap-8px text-12px text-gray-500">
                    <span v-if="friend.level">Lv{{ friend.level }}</span>
                    <span v-if="friend.gold != null">
                      {{ $t('page.farm.friends.gold') }} {{ formatFriendGold(friend.gold) }}
                    </span>
                  </div>
                </div>
              </div>
              <NPopconfirm @positive-click="toggleBlacklist(friend)">
                <template #trigger>
                  <NButton size="small" type="error" quaternary :loading="blacklistLoading">
                    {{ $t('page.farm.friends.removeBlacklist') }}
                  </NButton>
                </template>
                {{ $t('page.farm.friends.unblacklistConfirm', { name: friend.nickname || friend.gid }) }}
              </NPopconfirm>
            </div>
          </div>
        </NCard>
      </NTabPane>

      <NTabPane name="visitors" :tab="$t('page.farm.friends.tabVisitors')">
        <NCard :bordered="false" size="small" class="card-wrapper">
          <div class="mb-12px flex flex-wrap items-center justify-between gap-12px">
            <div class="flex flex-wrap gap-8px">
              <NButton
                v-for="item in interactFilters"
                :key="item.key"
                size="tiny"
                :type="interactFilter === item.key ? 'primary' : 'default'"
                secondary
                @click="interactFilter = item.key"
              >
                {{ $t(item.labelKey) }}
              </NButton>
            </div>
            <NButton size="small" :loading="interactLoading" @click="loadInteractRecords">
              {{ $t('common.refresh') }}
            </NButton>
          </div>

          <NSpin :show="interactLoading">
            <div v-if="interactError" class="py-24px text-center text-13px text-red-500">{{ interactError }}</div>
            <NEmpty
              v-else-if="!visibleInteractRecords.length"
              class="py-32px"
              :description="$t('page.farm.friends.visitorsEmpty')"
            />
            <div v-else class="flex-col gap-12px">
              <div
                v-for="record in visibleInteractRecords"
                :key="record.key"
                class="flex items-start gap-12px rounded-8px border border-gray-200 px-12px py-10px dark:border-gray-700"
              >
                <NAvatar
                  v-if="canShowInteractAvatar(record)"
                  :src="String(record.avatarUrl)"
                  round
                  :size="44"
                  @error="() => handleInteractAvatarError(record)"
                />
                <NAvatar v-else round :size="44">{{ (record.nick || '?').slice(0, 1) }}</NAvatar>
                <div class="min-w-0 flex-1">
                  <div class="mb-4px flex flex-wrap items-center gap-8px">
                    <span class="truncate font-medium">{{ record.nick || `GID:${record.visitorGid}` }}</span>
                    <NTag size="tiny" :type="interactBadgeType(record.actionType)" :bordered="false">
                      {{ record.actionLabel || $t('page.farm.friends.interact') }}
                    </NTag>
                    <NTag v-if="record.level" size="tiny" :bordered="false">Lv{{ record.level }}</NTag>
                    <span v-if="record.visitorGid" class="text-12px text-gray-400">GID {{ record.visitorGid }}</span>
                  </div>
                  <div class="text-13px text-gray-600 dark:text-gray-300">
                    {{ record.actionDetail || record.actionLabel }}
                  </div>
                </div>
                <div class="shrink-0 text-12px text-gray-400">
                  {{ formatInteractTime(record.serverTimeMs || (record.serverTimeSec || 0) * 1000) }}
                </div>
              </div>
            </div>
          </NSpin>
        </NCard>
      </NTabPane>
    </NTabs>
  </div>
</template>

<style scoped></style>
