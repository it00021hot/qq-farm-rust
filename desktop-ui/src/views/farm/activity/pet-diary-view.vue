<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import {
  NButton,
  NCard,
  NEmpty,
  NInput,
  NInputNumber,
  NModal,
  NProgress,
  NSelect,
  NSpin,
  NSwitch,
  NTabPane,
  NTabs,
  NTag,
  useMessage
} from 'naive-ui';
import {
  fetchGetFarmActivityPetDiary,
  fetchGetFarmActivityPetDiaryFriend,
  fetchGetFarmActivityPetDiaryRecords,
  fetchGetFarmAutomationDetail,
  fetchGetFarmFriendList,
  fetchOperateFarmActivityPetDiary
} from '@/service/api';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { resolveCatalogImage } from '@/views/farm/game-config/shared';
import dayjs from 'dayjs';

defineOptions({ name: 'FarmActivityPetDiaryView' });

type Item = {
  id?: string | number;
  name?: string;
  count?: number | string;
  image?: string;
  known?: boolean;
};

type TreasurePreview = {
  challengeId?: string;
  canStart?: boolean;
  maxProfit?: Item;
  maxLoss?: Item;
  plunderableCount?: string;
};

type Treasure = {
  id?: string;
  status?: number;
  item?: Item;
  protectedCount?: string;
  originalCount?: string;
  maxCount?: string;
  startTime?: number;
  endTime?: number;
  plunderCount?: number;
  maxPlunderCount?: number;
  previews?: TreasurePreview[];
};

type Charm = {
  id?: number;
  name?: string;
  description?: string;
  shortDescription?: string;
  useLimit?: number;
  image?: string;
  remaining?: number[];
};

type Goods = {
  id?: string;
  name?: string;
  image?: string;
  rewards?: Item[];
  costs?: Item[];
  limit?: string;
  purchased?: string;
  remaining?: string | null;
  exchangeable?: boolean;
  safeCosts?: boolean;
  order?: number;
  category?: string;
};

type Story = {
  order?: number;
  unlocked?: boolean;
  claimed?: boolean;
  animated?: boolean;
  photo?: string;
};

type SolarTerm = {
  id?: number;
  name?: string;
  startTime?: number;
  endTime?: number;
  canClaim?: boolean;
  claimed?: boolean;
  rewards?: Item[];
};

type PetDiarySnapshot = {
  activityId?: string;
  title?: string;
  active?: boolean;
  startTime?: number;
  endTime?: number;
  serverTime?: number;
  balances?: Item[];
  warnings?: string[];
  nurture?: {
    initialized?: boolean;
    adult?: boolean;
    growth?: number;
    adultGrowth?: number;
    dogGranted?: boolean;
    feedCount?: number;
    feedLimit?: number;
    feedCosts?: Item[];
    canFeed?: boolean;
  };
  hunt?: {
    count?: number;
    limit?: number;
    total?: string;
    luckyStarTotal?: string;
    costs?: Item[];
    canDraw?: boolean;
    canPlunder?: boolean;
  };
  seeds?: {
    canClaim?: boolean;
    days?: Array<{ day?: number; claimed?: boolean; claimable?: boolean; rewards?: Item[] }>;
  };
  stories?: Story[];
  charms?: {
    pool?: Charm[];
    equipped?: Charm[];
    all?: Charm[];
    picked?: boolean;
    canChoose?: boolean;
    freeRefreshRemaining?: number;
    freeRefreshLimit?: number;
    paidRefreshCount?: number;
    paidRefreshRemaining?: number;
    paidRefreshLimit?: number;
    refreshCost?: Item;
    refreshBalance?: string | null;
    canRefresh?: boolean;
    refreshNote?: string;
  };
  treasures?: Treasure[];
  compensationCount?: string;
  battleCount?: number;
  battleLimit?: number;
  skipBattle?: boolean;
  shop?: Goods[];
  solarTerms?: { terms?: SolarTerm[] } | null;
  plants?: Item[];
};

type BattleResult = {
  won?: boolean;
  defenderName?: string;
  plundered?: Item;
  streakTriggered?: boolean;
  streakReward?: Item;
  rewards?: Item[];
  attackerDice?: number;
  defenderDice?: number;
  needRefresh?: boolean;
  isFake?: boolean;
};

type InteractLog = {
  time?: number;
  type?: number;
  costs?: Item[];
  rewards?: Item[];
};

type PlunderLog = {
  time?: number;
  attackerGid?: string;
  name?: string;
  won?: boolean;
  treasureId?: string;
  challenge?: Item;
  level?: number;
  lost?: Item[];
  injected?: Item[];
  fake?: boolean;
};

const farmAccountStore = useFarmAccountStore();
const message = useMessage();

const snapshot = ref<PetDiarySnapshot | null>(null);
const loading = ref(false);
const snapshotError = ref('');
const pending = ref('');
const battleResult = ref<BattleResult | null>(null);
const friendGid = ref('');
const friendLoading = ref(false);
const friendTreasures = ref<Treasure[]>([]);
const friendCharms = ref<number[]>([]);
const friendError = ref('');
// 好友下拉：只列「可夺宝」好友（护送中 + 有 canStart 挑战书），避免全量刷屏
const friendOptions = ref<{ label: string; value: string }[]>([]);
const friendOptionsLoading = ref(false);
const friendOptionsHint = ref('');

function isPlunderableTreasure(treasure: Treasure): boolean {
  return treasure.status === 2 && (treasure.previews ?? []).some(preview => Boolean(preview.canStart));
}

async function loadFriendOptions(force = false) {
  const accountId = farmAccountStore.currentAccountId;
  if (!accountId || friendOptionsLoading.value) return;
  if (!force && friendOptions.value.length) return;
  friendOptionsLoading.value = true;
  friendOptionsHint.value = '正在筛选可夺宝好友…';
  try {
    const [{ error, data }, autoRes] = await Promise.all([
      fetchGetFarmFriendList({ current: 1, size: 500, accountId }),
      fetchGetFarmAutomationDetail(accountId)
    ]);
    if (error) {
      friendOptions.value = [];
      friendOptionsHint.value = error.message || '好友列表加载失败';
      return;
    }
    const blacklist = new Set((autoRes.data?.friendBlacklist || []).map(Number).filter(Boolean));
    const candidates = (data?.records ?? []).filter(
      friend => Number(friend.gid) > 0 && !blacklist.has(Number(friend.gid))
    );
    const plunderable: { label: string; value: string }[] = [];
    const concurrency = 5;
    let cursor = 0;
    const probeAccountId = accountId;
    async function probeWorker() {
      while (cursor < candidates.length) {
        const index = cursor;
        cursor += 1;
        const friend = candidates[index];
        const gid = String(friend.gid);
        try {
          const { error: probeError, data: probe } = await fetchGetFarmActivityPetDiaryFriend(probeAccountId, gid);
          if (probeError) continue;
          if ((probe?.treasures ?? []).some(isPlunderableTreasure)) {
            plunderable.push({
              label: `${friend.nickname || '未命名'}（${gid}）`,
              value: gid
            });
          }
        } catch {
          // 探测失败的好友跳过，tag 模式仍可手输 GID
        }
      }
    }
    await Promise.all(Array.from({ length: concurrency }, () => probeWorker()));
    plunderable.sort((a, b) => a.label.localeCompare(b.label, 'zh'));
    friendOptions.value = plunderable;
    friendOptionsHint.value = plunderable.length
      ? `可夺宝 ${plunderable.length} 人（共探测 ${candidates.length}）`
      : `暂无可夺宝好友（已探测 ${candidates.length}）`;
  } finally {
    friendOptionsLoading.value = false;
  }
}
const battleGid = ref('');
const battleTreasureId = ref('');
const battleChallengeId = ref('80101');
const logKind = ref<'interact' | 'plunder'>('interact');
const interactLogs = ref<InteractLog[]>([]);
const plunderLogs = ref<PlunderLog[]>([]);
const logsLoading = ref(false);
const exchangeGoodsId = ref('');
const exchangeCount = ref(1);
const paidRefreshModal = ref(false);

const CHALLENGES = [
  { id: '80101', label: '初级（50 幸运星）' },
  { id: '80102', label: '中级（150 幸运星）' },
  { id: '80103', label: '高级（300 幸运星）' }
];

const TREASURE_STATUS: Record<number, string> = {
  1: '待护送',
  2: '护送中',
  3: '待领取',
  4: '已领取'
};

function catalogImage(item: Item | undefined): string {
  // 后端道具图是 /game-config/... 相对路径，须转 farmcfg:// 协议后才能在 webview 加载
  return resolveCatalogImage(item?.image);
}

function itemText(item: Item | undefined): string {
  if (!item) return '';
  const count = item.count ?? '';
  return item.name ? `${item.name} ×${count}` : `道具 ${item.id} ×${count}`;
}

function fmtTime(ms?: number): string {
  if (!ms) return '';
  return dayjs(ms).format('MM-DD HH:mm');
}

const growthPercent = computed(() => {
  const nurture = snapshot.value?.nurture;
  if (!nurture || !nurture.adultGrowth) return 100;
  return Math.min(100, Math.round(((nurture.growth || 0) / nurture.adultGrowth) * 100));
});

const claimableTreasure = computed(() => snapshot.value?.treasures?.some(t => t.status === 3) ?? false);

const solarClaimable = computed(() => snapshot.value?.solarTerms?.terms?.filter(t => t.canClaim) ?? []);

async function loadSnapshot(showLoading = true) {
  const accountId = farmAccountStore.currentAccountId;
  if (!accountId) return;
  if (showLoading) loading.value = true;
  try {
    const { error, data } = await fetchGetFarmActivityPetDiary(accountId);
    if (error) {
      snapshotError.value = error.message || '萌宠日记读取失败';
      snapshot.value = null;
    } else {
      snapshotError.value = '';
      snapshot.value = data;
      if (data?.skipBattle !== undefined) {
        // skipBattle 状态由快照携带
      }
    }
  } finally {
    loading.value = false;
  }
}

async function operate(action: string, params: Record<string, unknown> = {}, successText = '') {
  if (pending.value) return;
  const accountId = farmAccountStore.currentAccountId;
  if (!accountId) return;
  pending.value = action;
  try {
    const { error, data } = await fetchOperateFarmActivityPetDiary(accountId, action, params);
    if (error) {
      message.error(error.message || '操作失败');
      return;
    }
    const result = data?.result;
    if (action === 'battle') {
      battleResult.value = result ?? null;
      if (data?.snapshot) snapshot.value = data.snapshot;
    } else if (data?.snapshot) {
      snapshot.value = data.snapshot;
    }
    if (data?.refreshError) message.warning(data.refreshError);
    const rewards: Item[] = data?.rewards ?? [];
    const rewardText = rewards.map(itemText).filter(Boolean).join('、');
    message.success(
      rewardText
        ? `${successText || data?.message || '操作成功'}：${rewardText}`
        : successText || data?.message || '操作成功'
    );
  } finally {
    pending.value = '';
  }
}

async function loadFriend() {
  const gid = friendGid.value.trim();
  if (!gid) {
    message.warning('请输入好友 GID');
    return;
  }
  const accountId = farmAccountStore.currentAccountId;
  if (!accountId) return;
  friendLoading.value = true;
  friendError.value = '';
  try {
    const { error, data } = await fetchGetFarmActivityPetDiaryFriend(accountId, gid);
    if (error) {
      friendError.value = error.message || '好友查询失败';
      friendTreasures.value = [];
      friendCharms.value = [];
    } else {
      // 查询结果也只展示可夺宝宝藏，避免不可开战项占位
      const all = data?.treasures ?? [];
      friendTreasures.value = all.filter(isPlunderableTreasure);
      friendCharms.value = data?.charms ?? [];
      if (!friendTreasures.value.length) {
        friendError.value = all.length ? '该好友暂无可开战宝藏' : '该好友没有护送中的宝藏';
      }
    }
  } finally {
    friendLoading.value = false;
  }
}

function pickBattleTarget(treasure: Treasure, challengeId: string) {
  battleGid.value = friendGid.value.trim();
  battleTreasureId.value = String(treasure.id || '');
  battleChallengeId.value = challengeId;
}

async function startBattle() {
  if (!battleGid.value || !battleTreasureId.value) {
    message.warning('请先选择好友宝藏与挑战书');
    return;
  }
  await operate(
    'battle',
    {
      gid: battleGid.value,
      treasureId: battleTreasureId.value,
      challengeId: battleChallengeId.value
    },
    '夺宝完成'
  );
  await loadFriend();
}

async function refreshCharm(paid: boolean) {
  if (paid) {
    paidRefreshModal.value = false;
    await operate(
      'refreshCharm',
      {
        payment: 'tickets',
        expectedPaidRefreshCount: snapshot.value?.charms?.paidRefreshCount ?? 0
      },
      '锦囊已刷新'
    );
  } else {
    await operate('refreshCharm', { payment: 'free' }, '锦囊已刷新');
  }
}

async function exchange() {
  if (!exchangeGoodsId.value.trim()) {
    message.warning('请输入商品编号');
    return;
  }
  await operate('exchange', { goodsId: exchangeGoodsId.value.trim(), count: exchangeCount.value || 1 }, '兑换成功');
}

async function loadLogs() {
  const accountId = farmAccountStore.currentAccountId;
  if (!accountId) return;
  logsLoading.value = true;
  try {
    const { error, data } = await fetchGetFarmActivityPetDiaryRecords(accountId, logKind.value);
    if (error) {
      message.error(error.message || '日志读取失败');
      return;
    }
    if (logKind.value === 'interact') {
      interactLogs.value = data ?? [];
    } else {
      plunderLogs.value = data ?? [];
    }
  } finally {
    logsLoading.value = false;
  }
}

function switchLogKind(kind: 'interact' | 'plunder') {
  logKind.value = kind;
  loadLogs();
}

onMounted(() => {
  loadSnapshot();
  loadFriendOptions(true);
});

watch(
  () => farmAccountStore.currentAccountId,
  () => {
    friendOptions.value = [];
    friendOptionsHint.value = '';
    friendGid.value = '';
    friendTreasures.value = [];
    friendCharms.value = [];
    friendError.value = '';
    loadSnapshot();
    loadFriendOptions(true);
  }
);
</script>

<template>
  <div class="flex flex-col gap-12px">
    <NSpin :show="loading">
      <NCard :bordered="false" size="small" class="card-wrapper">
        <template #header>
          <div class="flex items-center gap-8px">
            <span class="text-16px font-medium">萌宠成长日记</span>
            <NTag :type="snapshot?.active ? 'success' : 'default'" size="small">
              {{ snapshot?.active ? '进行中' : '未开放' }}
            </NTag>
          </div>
        </template>
        <template #header-extra>
          <NButton size="small" :loading="loading" @click="loadSnapshot()">刷新</NButton>
        </template>

        <NEmpty v-if="snapshotError" :description="snapshotError" />
        <template v-else-if="snapshot">
          <div v-if="snapshot.warnings?.length" class="mb-8px">
            <NTag v-for="warning in snapshot.warnings" :key="warning" type="warning" size="small" class="mr-4px">
              {{ warning }}
            </NTag>
          </div>

          <div class="mb-12px flex flex-wrap items-center gap-10px text-13px text-gray-600">
            <span>{{ fmtTime(snapshot.startTime) }} ~ {{ fmtTime(snapshot.endTime) }}</span>
            <span v-for="balance in snapshot.balances" :key="balance.id" class="flex items-center gap-2px">
              <img v-if="catalogImage(balance)" :src="catalogImage(balance)" class="h-18px w-18px" alt="" />
              {{ balance.name || balance.id }} ×{{ balance.count ?? '未知' }}
            </span>
          </div>

          <!-- 养成 -->
          <NCollapse :default-expanded-keys="['nurture', 'hunt', 'charm', 'battle', 'seeds', 'shop', 'story', 'solar']">
            <NCollapseItem title="萌宠养成" name="nurture">
              <div class="flex flex-col gap-10px">
                <template v-if="!snapshot.nurture?.initialized">
                  <NButton
                    type="primary"
                    size="small"
                    :loading="pending === 'initialize'"
                    @click="operate('initialize', {}, '领养成功')"
                  >
                    领养比熊幼崽
                  </NButton>
                </template>
                <template v-else>
                  <div class="flex items-center gap-12px">
                    <NProgress
                      type="line"
                      :percentage="growthPercent"
                      :height="14"
                      class="max-w-360px"
                      indicator-text-color="#18a058"
                    />
                    <span class="text-13px">
                      成长 {{ snapshot.nurture?.growth ?? 0 }}/{{ snapshot.nurture?.adultGrowth ?? 0 }} （{{
                        snapshot.nurture?.adult ? '已成年' : '幼年期'
                      }}）
                    </span>
                  </div>
                  <div class="flex flex-wrap items-center gap-8px">
                    <NButton
                      size="small"
                      type="primary"
                      :disabled="!snapshot.nurture?.canFeed"
                      :loading="pending === 'feed'"
                      @click="operate('feed', {}, '投喂成功')"
                    >
                      投喂元气糕（{{ snapshot.nurture?.feedCount ?? 0 }}/{{ snapshot.nurture?.feedLimit ?? 16 }}）
                    </NButton>
                    <span class="text-12px text-gray-500">
                      消耗：{{ (snapshot.nurture?.feedCosts ?? []).map(itemText).join('、') }}
                    </span>
                    <NButton
                      v-if="snapshot.nurture?.adult && !snapshot.nurture?.dogGranted"
                      size="small"
                      type="warning"
                      :loading="pending === 'claimDog'"
                      @click="operate('claimDog', {}, '已领取永久比熊')"
                    >
                      领取永久比熊
                    </NButton>
                    <NTag v-if="snapshot.nurture?.dogGranted" type="success" size="small">永久比熊已入队</NTag>
                  </div>
                </template>
              </div>
            </NCollapseItem>

            <!-- 寻宝 / 护送 -->
            <NCollapseItem title="寻宝与护送" name="hunt">
              <div class="flex flex-col gap-10px">
                <div class="flex flex-wrap items-center gap-8px">
                  <NButton
                    size="small"
                    type="primary"
                    :disabled="!snapshot.hunt?.canDraw"
                    :loading="pending === 'draw'"
                    @click="operate('draw', {}, '寻宝成功')"
                  >
                    寻宝（{{ snapshot.hunt?.count ?? 0 }}/{{ snapshot.hunt?.limit ?? 10 }}）
                  </NButton>
                  <span class="text-12px text-gray-500">
                    消耗：{{ (snapshot.hunt?.costs ?? []).map(itemText).join('、') }}；累计宝藏
                    {{ snapshot.hunt?.total ?? '0' }}，幸运星 {{ snapshot.hunt?.luckyStarTotal ?? '0' }}
                  </span>
                  <NButton
                    size="small"
                    :disabled="!claimableTreasure"
                    :loading="pending === 'openTreasure'"
                    @click="operate('openTreasure', {}, '宝藏已开启')"
                  >
                    开启完成的宝藏
                  </NButton>
                  <NButton
                    size="small"
                    :disabled="Number(snapshot.compensationCount) <= 0"
                    :loading="pending === 'compensation'"
                    @click="operate('compensation', {}, '补偿已领取')"
                  >
                    领取夺宝补偿（{{ snapshot.compensationCount ?? 0 }}）
                  </NButton>
                </div>
                <div v-if="snapshot.treasures?.length" class="flex flex-col gap-6px">
                  <div
                    v-for="treasure in snapshot.treasures"
                    :key="treasure.id"
                    class="flex flex-wrap items-center gap-8px rounded-6px bg-gray-50 px-10px py-6px dark:bg-gray-800/40"
                  >
                    <NTag
                      size="small"
                      :type="treasure.status === 3 ? 'success' : treasure.status === 2 ? 'info' : 'default'"
                    >
                      {{ TREASURE_STATUS[treasure.status ?? 0] || `状态 ${treasure.status}` }}
                    </NTag>
                    <img
                      v-if="catalogImage(treasure.item)"
                      :src="catalogImage(treasure.item)"
                      class="h-20px w-20px"
                      alt=""
                    />
                    <span class="text-13px">{{ itemText(treasure.item) }}</span>
                    <span class="text-12px text-gray-500">
                      保护 {{ treasure.protectedCount }}/{{ treasure.originalCount }}，被夺
                      {{ treasure.plunderCount }}/{{ treasure.maxPlunderCount }}
                    </span>
                    <span v-if="treasure.endTime" class="text-12px text-gray-400">
                      {{ fmtTime(treasure.endTime) }} 到达
                    </span>
                  </div>
                </div>
                <NEmpty v-else description="暂无宝藏，寻宝后可开启护送" size="small" />
              </div>
            </NCollapseItem>

            <!-- 锦囊 -->
            <NCollapseItem title="夺宝锦囊" name="charm">
              <div class="flex flex-col gap-10px">
                <div class="flex flex-wrap items-center gap-8px">
                  <NButton
                    size="small"
                    :disabled="!snapshot.charms?.canRefresh"
                    :loading="pending === 'refreshCharm'"
                    @click="
                      (snapshot.charms?.freeRefreshRemaining ?? 0) > 0 ? refreshCharm(false) : (paidRefreshModal = true)
                    "
                  >
                    刷新锦囊池（免费 {{ snapshot.charms?.freeRefreshRemaining ?? 0 }} 次）
                  </NButton>
                  <span class="text-12px text-gray-500">{{ snapshot.charms?.refreshNote }}</span>
                </div>
                <div class="flex flex-wrap gap-8px">
                  <div
                    v-for="charm in snapshot.charms?.pool ?? []"
                    :key="charm.id"
                    class="flex w-220px flex-col gap-4px rounded-8px border border-gray-200 px-10px py-8px dark:border-gray-700"
                  >
                    <div class="flex items-center gap-6px">
                      <img v-if="charm.image" :src="charm.image" class="h-22px w-22px" alt="" />
                      <span class="text-13px font-medium">{{ charm.name }}</span>
                    </div>
                    <span class="text-12px text-gray-500">{{ charm.shortDescription || charm.description }}</span>
                    <NButton
                      v-if="snapshot.charms?.canChoose"
                      size="tiny"
                      type="primary"
                      :loading="pending === 'equipCharm'"
                      @click="operate('equipCharm', { charmId: charm.id }, '锦囊已装备')"
                    >
                      装备
                    </NButton>
                  </div>
                  <NEmpty v-if="!(snapshot.charms?.pool ?? []).length" description="暂无待选锦囊" size="small" />
                </div>
                <div v-if="(snapshot.charms?.equipped ?? []).length" class="flex flex-wrap items-center gap-8px">
                  <span class="text-12px text-gray-500">已装备：</span>
                  <NTag v-for="charm in snapshot.charms?.equipped ?? []" :key="charm.id" size="small" type="info">
                    {{ charm.name }}
                    <template v-if="charm.remaining?.length">（剩 {{ charm.remaining.join('/') }} 次）</template>
                  </NTag>
                </div>
              </div>
            </NCollapseItem>

            <!-- 夺宝 -->
            <NCollapseItem title="好友夺宝" name="battle">
              <div class="flex flex-col gap-10px">
                <div class="flex flex-wrap items-center gap-8px">
                  <NSelect
                    v-model:value="friendGid"
                    :options="friendOptions"
                    :loading="friendOptionsLoading"
                    placeholder="搜索可夺宝好友（可手输 GID）"
                    size="small"
                    class="w-240px"
                    filterable
                    clearable
                    tag
                  />
                  <NButton size="small" :loading="friendOptionsLoading" @click="loadFriendOptions(true)">
                    刷新可夺
                  </NButton>
                  <NButton size="small" :loading="friendLoading" @click="loadFriend()">查询好友宝藏</NButton>
                  <span class="text-12px text-gray-500">
                    今日夺宝 {{ snapshot.battleCount ?? 0 }}/{{ snapshot.battleLimit ?? 20 }}
                  </span>
                  <span class="flex items-center gap-6px text-12px text-gray-500">
                    跳过战斗动画
                    <NSwitch
                      :value="snapshot.skipBattle"
                      :loading="pending === 'skipBattle'"
                      @update:value="(value: boolean) => operate('skipBattle', { skip: value }, '已更新')"
                    />
                  </span>
                </div>
                <div v-if="friendOptionsHint" class="text-12px text-gray-400">{{ friendOptionsHint }}</div>
                <NEmpty v-if="friendError && !friendTreasures.length" :description="friendError" size="small" />
                <div v-else-if="friendTreasures.length" class="flex flex-col gap-6px">
                  <div
                    v-for="treasure in friendTreasures"
                    :key="treasure.id"
                    class="flex flex-wrap items-center gap-8px rounded-6px bg-gray-50 px-10px py-6px dark:bg-gray-800/40"
                  >
                    <img
                      v-if="catalogImage(treasure.item)"
                      :src="catalogImage(treasure.item)"
                      class="h-20px w-20px"
                      alt=""
                    />
                    <span class="text-13px">{{ itemText(treasure.item) }}</span>
                    <NButton
                      v-for="preview in treasure.previews ?? []"
                      :key="preview.challengeId"
                      size="tiny"
                      :type="preview.canStart ? 'primary' : 'default'"
                      :disabled="!preview.canStart"
                      @click="pickBattleTarget(treasure, String(preview.challengeId ?? ''))"
                    >
                      {{
                        CHALLENGES.find(c => c.id === String(preview.challengeId))?.label ||
                        `挑战书 ${preview.challengeId}`
                      }}（可夺 {{ preview.plunderableCount ?? 0 }}）
                    </NButton>
                  </div>
                  <div class="flex flex-wrap items-center gap-8px">
                    <span v-if="battleTreasureId" class="text-12px">
                      已选宝藏 {{ battleTreasureId }}，挑战书
                      {{ CHALLENGES.find(c => c.id === battleChallengeId)?.label || battleChallengeId }}
                    </span>
                    <NButton
                      size="small"
                      type="error"
                      :disabled="!snapshot.hunt?.canPlunder"
                      :loading="pending === 'battle'"
                      @click="startBattle()"
                    >
                      开始夺宝
                    </NButton>
                  </div>
                </div>
                <div v-if="battleResult" class="rounded-8px bg-orange-50 px-12px py-8px dark:bg-orange-900/20">
                  <div class="text-14px font-medium">
                    {{ battleResult.won ? '夺宝成功' : '本次夺宝未获胜' }}
                    <span v-if="battleResult.isFake" class="text-12px text-gray-500">（触发移花接木：假宝藏）</span>
                  </div>
                  <div class="mt-2px text-12px text-gray-600">
                    骰子 我方 {{ battleResult.attackerDice }} : 对方 {{ battleResult.defenderDice }}
                    <template v-if="battleResult.plundered">；掠夺 {{ itemText(battleResult.plundered) }}</template>
                    <template v-if="battleResult.rewards?.length">
                      ；奖励 {{ battleResult.rewards.map(itemText).join('、') }}
                    </template>
                  </div>
                </div>
              </div>
            </NCollapseItem>

            <!-- 种子赠礼 -->
            <NCollapseItem title="种子赠礼" name="seeds">
              <div class="flex flex-col gap-10px">
                <div class="flex flex-wrap gap-8px">
                  <div
                    v-for="day in snapshot.seeds?.days ?? []"
                    :key="day.day"
                    class="flex w-150px flex-col gap-4px rounded-8px border border-gray-200 px-10px py-8px text-center dark:border-gray-700"
                  >
                    <span class="text-13px">第 {{ day.day }} 天</span>
                    <img
                      v-for="reward in day.rewards ?? []"
                      :key="reward.id"
                      :src="catalogImage(reward)"
                      class="h-20px w-20px"
                      :title="itemText(reward)"
                      alt=""
                    />
                    <NTag size="tiny" :type="day.claimed ? 'success' : day.claimable ? 'warning' : 'default'">
                      {{ day.claimed ? '已领取' : day.claimable ? '可领取' : '未解锁' }}
                    </NTag>
                  </div>
                </div>
                <NButton
                  size="small"
                  type="primary"
                  :disabled="!snapshot.seeds?.canClaim"
                  :loading="pending === 'seeds'"
                  @click="operate('seeds', {}, '种子已领取')"
                >
                  一键领取可领种子
                </NButton>
              </div>
            </NCollapseItem>

            <!-- 拾物小铺 -->
            <NCollapseItem title="拾物小铺" name="shop">
              <div class="flex flex-col gap-10px">
                <div class="flex flex-col gap-6px">
                  <div
                    v-for="goodsItem in snapshot.shop ?? []"
                    :key="goodsItem.id"
                    class="flex flex-wrap items-center gap-10px rounded-6px bg-gray-50 px-10px py-6px dark:bg-gray-800/40"
                  >
                    <span class="w-24px text-center text-12px text-gray-400">#{{ goodsItem.id }}</span>
                    <img v-if="goodsItem.image" :src="goodsItem.image" class="h-22px w-22px" alt="" />
                    <span class="text-13px">{{ goodsItem.name }}</span>
                    <span class="text-12px text-gray-500">
                      消耗 {{ (goodsItem.costs ?? []).map(itemText).join('、') }}
                    </span>
                    <span class="text-12px text-gray-500">
                      限购 {{ goodsItem.purchased }}/{{ goodsItem.limit || '不限' }}
                      <template v-if="goodsItem.remaining">(剩 {{ goodsItem.remaining }})</template>
                    </span>
                    <NButton
                      size="tiny"
                      :disabled="!goodsItem.exchangeable"
                      :loading="pending === 'exchange'"
                      @click="operate('exchange', { goodsId: goodsItem.id, count: 1 }, '兑换成功')"
                    >
                      兑换
                    </NButton>
                  </div>
                  <NEmpty v-if="!(snapshot.shop ?? []).length" description="小铺目录为空" size="small" />
                </div>
                <div class="flex flex-wrap items-center gap-8px">
                  <NInput v-model:value="exchangeGoodsId" placeholder="商品编号" size="small" class="w-120px" />
                  <NInputNumber v-model:value="exchangeCount" :min="1" size="small" class="w-100px" />
                  <NButton size="small" :loading="pending === 'exchange'" @click="exchange()">按编号兑换</NButton>
                </div>
              </div>
            </NCollapseItem>

            <!-- 手记 -->
            <NCollapseItem title="成长手记" name="story">
              <div class="flex flex-wrap gap-10px">
                <div
                  v-for="story in snapshot.stories ?? []"
                  :key="story.order"
                  class="flex w-170px flex-col gap-4px rounded-8px border border-gray-200 px-10px py-8px text-center dark:border-gray-700"
                >
                  <span class="text-12px text-gray-400">第 {{ story.order }} 则</span>
                  <img v-if="story.photo" :src="story.photo" class="w-full rounded-4px object-contain" alt="" />
                  <span v-else class="text-12px text-gray-400">未解锁</span>
                  <NTag size="tiny" :type="story.claimed ? 'success' : story.unlocked ? 'warning' : 'default'">
                    {{ story.claimed ? '已领取' : story.unlocked ? '可领取' : '未解锁' }}
                  </NTag>
                  <NButton
                    v-if="story.unlocked && !story.claimed"
                    size="tiny"
                    type="primary"
                    :loading="pending === 'story'"
                    @click="operate('story', { order: story.order }, '手记奖励已领取')"
                  >
                    领取
                  </NButton>
                </div>
              </div>
            </NCollapseItem>

            <!-- 节令 -->
            <NCollapseItem
              v-if="solarClaimable.length || (snapshot.solarTerms?.terms ?? []).length"
              title="节令小礼"
              name="solar"
            >
              <div class="flex flex-wrap items-center gap-10px">
                <div
                  v-for="term in snapshot.solarTerms?.terms ?? []"
                  :key="term.id"
                  class="flex items-center gap-8px rounded-6px bg-gray-50 px-10px py-6px dark:bg-gray-800/40"
                >
                  <span class="text-13px">{{ term.name || `节令 ${term.id}` }}</span>
                  <span class="text-12px text-gray-500">{{ (term.rewards ?? []).map(itemText).join('、') }}</span>
                  <NButton
                    v-if="term.canClaim"
                    size="tiny"
                    type="primary"
                    :loading="pending === 'solar'"
                    @click="operate('solar', { termId: term.id }, '节令好礼已领取')"
                  >
                    领取
                  </NButton>
                  <NTag v-else-if="term.claimed" size="tiny" type="success">已领取</NTag>
                </div>
              </div>
            </NCollapseItem>

            <!-- 日志 -->
            <NCollapseItem title="互动与被夺日志" name="logs">
              <template #header-extra>
                <NButton size="tiny" :loading="logsLoading" @click.stop="loadLogs()">刷新日志</NButton>
              </template>
              <NTabs :value="logKind" type="segment" size="small" @update:value="switchLogKind">
                <NTabPane name="interact" tab="互动日志">
                  <div class="flex flex-col gap-4px">
                    <div v-for="(log, index) in interactLogs" :key="index" class="text-12px text-gray-600">
                      {{ fmtTime(log.time) }} · 类型 {{ log.type }}
                      <template v-if="log.costs?.length">· 消耗 {{ log.costs.map(itemText).join('、') }}</template>
                      <template v-if="log.rewards?.length">· 获得 {{ log.rewards.map(itemText).join('、') }}</template>
                    </div>
                    <NEmpty v-if="!interactLogs.length" description="暂无互动日志" size="small" />
                  </div>
                </NTabPane>
                <NTabPane name="plunder" tab="被夺日志">
                  <div class="flex flex-col gap-4px">
                    <div v-for="(log, index) in plunderLogs" :key="index" class="text-12px text-gray-600">
                      {{ fmtTime(log.time) }} · {{ log.name }}（Lv.{{ log.level }}）
                      <NTag size="tiny" :type="log.won ? 'error' : 'success'">
                        {{ log.won ? '对方获胜' : '对方失败' }}
                      </NTag>
                      <template v-if="log.lost?.length">· 损失 {{ log.lost.map(itemText).join('、') }}</template>
                      <template v-if="log.fake">· 移花接木假宝藏</template>
                    </div>
                    <NEmpty v-if="!plunderLogs.length" description="暂无被夺日志" size="small" />
                  </div>
                </NTabPane>
              </NTabs>
            </NCollapseItem>
          </NCollapse>
        </template>
      </NCard>
    </NSpin>

    <NModal
      v-model:show="paidRefreshModal"
      preset="confirm"
      title="付费刷新锦囊"
      :content="`免费刷新已用完，本次将消耗 ${snapshot?.charms?.refreshCost?.count ?? 30} 点券（余额 ${snapshot?.charms?.refreshBalance ?? '未知'}），是否继续？不会使用钻石。`"
      positive-text="消耗点券刷新"
      negative-text="取消"
      @positive-click="refreshCharm(true)"
    />
  </div>
</template>

<style scoped></style>
