<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import dayjs from 'dayjs';
import { NAvatar, NButton, NCard, NEmpty, NGi, NGrid, NInput, NProgress, NSelect, NSpace, NSpin, NTag } from 'naive-ui';
import {
  fetchClearFarmLogs,
  fetchGetFarmBag,
  fetchGetFarmDiamond,
  fetchGetFarmLogs,
  fetchGetFarmStatusDetail
} from '@/service/api';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { useFarmWs } from '@/hooks/business/farm-ws';
import {
  COUPON_ITEM_ID,
  COLLECTION_NORMAL_ID,
  COLLECTION_RARE_ID,
  GOLD_BEAN_ITEM_ID,
  NORMAL_FERTILIZER_ID,
  ORGANIC_FERTILIZER_ID
} from '@/constants/items';
import { $t } from '@/locales';
import { getLogEventLabel, humanizeLogMessage, LOG_EVENT_FILTER_OPTIONS, logText, shouldShowEventChip } from './log-events';

defineOptions({
  name: 'FarmDashboard'
});

interface FarmLogRow {
  id: number;
  ts: number;
  time: string;
  tag: string;
  event: string;
  message: string;
  isWarn: boolean;
}

const OP_META: Record<string, { label: string; icon: string }> = {
  harvest: { label: '收获', icon: '🌾' },
  farming: { label: '一键务农', icon: '🧑‍🌾' },
  fertilize: { label: '施肥', icon: '🧪' },
  plant: { label: '种植', icon: '🌱' },
  steal: { label: '偷菜', icon: '🏃' },
  helpFarming: { label: '帮务农', icon: '🤝' },
  taskClaim: { label: '任务', icon: '✅' },
  sell: { label: '出售', icon: '💰' }
};

const MODULE_OPTIONS = [
  { label: '所有模块', value: '' },
  { label: '农场', value: 'farm' },
  { label: '好友', value: 'friend' },
  { label: '系统', value: 'system' }
];

const LEVEL_OPTIONS = [
  { label: '所有等级', value: '' },
  { label: '普通', value: 'info' },
  { label: '警告', value: 'warn' }
];

const farmAccountStore = useFarmAccountStore();
const status = ref<Api.Farm.Status | null>(null);
const detailLoading = ref(false);
const bagLoading = ref(false);
const bagItems = ref<Api.Farm.BagItem[]>([]);
const diamond = ref<number | null>(null);
const couponBaseline = ref<number | null>(null);
const logs = ref<FarmLogRow[]>([]);
const logContainer = ref<HTMLElement | null>(null);
const autoScroll = ref(true);
let logSeq = 0;

const filterModule = ref('');
const filterEvent = ref('');
const filterLevel = ref('');
const filterKeyword = ref('');

const localFarmRemain = ref(0);
const localHelpRemain = ref(0);
const localStealRemain = ref(0);
const localUptime = ref(0);
let countdownTimer: ReturnType<typeof setInterval> | null = null;
let bagTimer: ReturnType<typeof setInterval> | null = null;

const currentAccount = computed(
  () => farmAccountStore.accounts.find(a => a.id === farmAccountStore.currentAccountId) || null
);

const isOnline = computed(() => Boolean(status.value?.runStatus === 1));

const displayName = computed(() => {
  const nick = status.value?.nick || '';
  const remark = currentAccount.value?.name || '';
  if (nick && remark && nick !== remark) return `${nick} (${remark})`;
  return nick || remark || $t('page.farm.dashboard.notLoggedIn');
});

const avatarUrl = computed(() => {
  const raw = status.value?.avatar || currentAccount.value?.avatar || '';
  if (!raw) return '';
  if (raw.startsWith('//')) return `https:${raw}`;
  return raw;
});

const avatarFailed = ref(false);
watch(avatarUrl, () => {
  avatarFailed.value = false;
});

const canShowAvatar = computed(() => Boolean(avatarUrl.value) && !avatarFailed.value);

const avatarInitial = computed(() => (displayName.value || '?').slice(0, 1));

const levelProgress = computed(() => status.value?.levelProgress || { current: 0, needed: 0 });

const expPercent = computed(() => {
  const needed = Number(levelProgress.value.needed || 0);
  const current = Number(levelProgress.value.current || 0);
  if (needed <= 0) return 0;
  return Math.min(100, Math.max(0, (current / needed) * 100));
});

const expRate = computed(() => {
  const gain = Number(status.value?.sessionExpGained || 0);
  const uptime = localUptime.value || Number(status.value?.uptime || 0);
  if (!uptime) return '0/时';
  const hours = uptime / 3600;
  const rate = hours > 0 ? gain / hours : 0;
  return `${Math.floor(rate)}/时`;
});

const timeToLevel = computed(() => {
  const gain = Number(status.value?.sessionExpGained || 0);
  const uptime = localUptime.value || Number(status.value?.uptime || 0);
  const current = Number(levelProgress.value.current || 0);
  const needed = Number(levelProgress.value.needed || 0);
  if (!needed || !uptime || gain <= 0) return '';
  const hours = uptime / 3600;
  const ratePerHour = hours > 0 ? gain / hours : 0;
  if (ratePerHour <= 0) return '';
  const mins = (needed - current) / (ratePerHour / 60);
  if (mins < 60) return `约 ${Math.ceil(mins)} 分钟后升级`;
  return `约 ${(mins / 60).toFixed(1)} 小时后升级`;
});

function bagItemById(id: number) {
  return bagItems.value.find(item => Number(item.id) === id);
}

function formatBucketTime(item?: Api.Farm.BagItem) {
  if (!item) return '0.0h';
  if (item.hoursText) return String(item.hoursText).replace('小时', 'h');
  return `${(Number(item.count || 0) / 3600).toFixed(1)}h`;
}

const fertilizerNormal = computed(() => bagItemById(NORMAL_FERTILIZER_ID));
const fertilizerOrganic = computed(() => bagItemById(ORGANIC_FERTILIZER_ID));
const collectionNormal = computed(() => bagItemById(COLLECTION_NORMAL_ID));
const collectionRare = computed(() => bagItemById(COLLECTION_RARE_ID));
const coupon = computed(() => Number(bagItemById(COUPON_ITEM_ID)?.count || 0));
const goldBean = computed(() => Number(bagItemById(GOLD_BEAN_ITEM_ID)?.count || 0));
const sessionCouponGained = computed(() => {
  if (couponBaseline.value == null) return 0;
  return coupon.value - couponBaseline.value;
});

const filteredOperations = computed(() => {
  const ops = status.value?.operations || {};
  const result: { key: string; value: number }[] = [];
  for (const [key, value] of Object.entries(ops)) {
    if (key === 'upgrade' || key === 'levelUp') continue;
    result.push({ key, value: Number(value) || 0 });
  }
  return result;
});

const filteredLogs = computed(() => {
  const keyword = filterKeyword.value.trim().toLowerCase();
  return logs.value.filter(log => {
    if (filterModule.value) {
      const tagMap: Record<string, string[]> = {
        farm: ['农场', '收获', '种植', '施肥', '务农'],
        friend: ['好友', '偷菜', '帮助', '捣乱'],
        system: ['系统', '错误', '状态']
      };
      const tags = tagMap[filterModule.value] || [];
      if (!tags.some(t => log.tag.includes(t) || log.message.includes(t))) return false;
    }
    if (filterEvent.value && log.event !== filterEvent.value) return false;
    if (filterLevel.value === 'warn' && !log.isWarn) return false;
    if (filterLevel.value === 'info' && log.isWarn) return false;
    if (keyword && !`${log.message} ${log.tag} ${log.event}`.toLowerCase().includes(keyword)) return false;
    return true;
  });
});

function formatClock(sec: number): string {
  if (sec <= 0) return '00:00:00';
  const d = Math.floor(sec / 86400);
  const h = Math.floor((sec % 86400) / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = Math.floor(sec % 60);
  const pad = (n: number) => String(n).padStart(2, '0');
  if (d > 0) return `${d}天 ${pad(h)}:${pad(m)}:${pad(s)}`;
  return `${pad(h)}:${pad(m)}:${pad(s)}`;
}

function formatCheckLabel(sec: number): string {
  if (!isOnline.value) return $t('page.farm.dashboard.accountOffline');
  if (sec <= 0) return $t('page.farm.dashboard.checkingNow');
  return formatClock(sec);
}

const nextFarmCheck = computed(() => formatCheckLabel(localFarmRemain.value));
const nextHelpCheck = computed(() => formatCheckLabel(localHelpRemain.value));
const nextStealCheck = computed(() => formatCheckLabel(localStealRemain.value));

function syncRemain(local: { value: number }, serverRaw: number | undefined) {
  const server = Math.max(0, Number(serverRaw || 0));
  // Avoid flicker: ignore tiny server/local drift while local countdown is ticking.
  if (local.value <= 0 || server <= 0 || Math.abs(local.value - server) > 2) {
    local.value = server;
  }
}

function syncNextChecks(next?: Api.Farm.Status['nextChecks']) {
  syncRemain(localFarmRemain, next?.farmRemainSec);
  syncRemain(localHelpRemain, next?.helpRemainSec);
  syncRemain(localStealRemain, next?.stealRemainSec);
}

function syncUptime(next?: number) {
  if (typeof next !== 'number') return;
  if (localUptime.value <= 0 || Math.abs(localUptime.value - next) > 2) {
    localUptime.value = next;
  }
}

function tickCountdowns() {
  if (!isOnline.value) return;
  localUptime.value += 1;
  if (localFarmRemain.value > 0) localFarmRemain.value -= 1;
  if (localHelpRemain.value > 0) localHelpRemain.value -= 1;
  if (localStealRemain.value > 0) localStealRemain.value -= 1;
}

function pushLog(tag: string, message: string, event = '', isWarn = false, ts?: number) {
  const at = ts && ts > 0 ? ts : Date.now();
  logs.value.push({
    id: ++logSeq,
    ts: at,
    time: dayjs(at).format('HH:mm:ss'),
    tag,
    event,
    message,
    isWarn
  });
  if (logs.value.length > 1000) logs.value.splice(0, logs.value.length - 1000);
  nextTick(() => {
    if (logContainer.value && autoScroll.value) {
      logContainer.value.scrollTop = logContainer.value.scrollHeight;
    }
  });
}

function applyLogEntries(entries: Api.Farm.LogEntry[]) {
  logSeq = 0;
  const list = [...(entries || [])].sort((a, b) => (Number(a.ts) || 0) - (Number(b.ts) || 0));
  logs.value = list.map(e => {
    const at = Number(e.ts) || (e.time ? dayjs(e.time).valueOf() : Date.now());
    return {
      id: ++logSeq,
      ts: at,
      time: dayjs(at).format('HH:mm:ss'),
      tag: e.tag || '系统',
      event: getLogEventLabel(e.meta?.event || ''),
      message: humanizeLogMessage(e.msg || ''),
      isWarn: Boolean(e.isWarn)
    };
  });
  nextTick(() => {
    if (logContainer.value) {
      logContainer.value.scrollTop = logContainer.value.scrollHeight;
    }
  });
}

async function loadLogs() {
  if (!farmAccountStore.currentAccountId) {
    logs.value = [];
    return;
  }
  const { data, error } = await fetchGetFarmLogs({
    accountId: farmAccountStore.currentAccountId,
    limit: 1000
  });
  if (!error && Array.isArray(data)) {
    applyLogEntries(data);
  }
}

function formatEventMessage(
  type: string,
  payload: unknown
): { tag: string; message: string; event: string; isWarn: boolean } {
  const body = (payload && typeof payload === 'object' ? payload : {}) as Record<string, unknown>;
  const readable = logText(body);
  const eventKey = String(body.event || body.action || '');
  // Prefer bot-aligned fields when backend already formats them.
  if (readable) {
    return {
      tag: String(body.tag || (body.isWarn ? '错误' : '系统')),
      event: getLogEventLabel(eventKey),
      message: readable,
      isWarn: Boolean(body.isWarn)
    };
  }
  if (type === 'farm_operation' || type === 'farm_tick') {
    const actions = Array.isArray(body.actions) ? (body.actions as string[]).filter(Boolean).join('/') : '';
    const err = body.error ? String(body.error) : '';
    return {
      tag: err ? '错误' : '农场',
      event: type === 'farm_tick' ? '农场巡查' : '农场操作',
      message: [actions || '巡查完成', err].filter(Boolean).join(' · '),
      isWarn: Boolean(err)
    };
  }
  if (type === 'friend_interact') {
    const action = String(body.action || '好友');
    const result = String(body.result || '');
    const name = String(body.friendName || '');
    const gid = body.targetGid ?? body.target_gid;
    const detail = (body.detail && typeof body.detail === 'object' ? body.detail : {}) as Record<string, unknown>;
    const count = Number(detail.count || 0);
    const err = detail.error ? String(detail.error) : '';
    const who = name || (gid ? `GID:${gid}` : '好友');
    const summary = typeof detail.summary === 'string' ? detail.summary.trim() : '';
    const plants = Array.isArray(detail.plants) ? (detail.plants as unknown[]).map(String).filter(Boolean) : [];
    const event =
      action === 'steal'
        ? '偷好友菜'
        : action === 'steal_score'
          ? '偷取积分'
          : action === 'bad'
            ? '放虫放草'
            : action === 'help' || action === 'water' || action === 'weed' || action === 'bug'
              ? '帮助好友'
              : '照顾好友';
    if (result === 'error' || err) {
      return { tag: '好友', event, message: `${who}: ${err || '失败'}`, isWarn: true };
    }
    if (summary) {
      return { tag: '好友', event, message: action === 'steal_score' ? summary : `${who}: ${summary}`, isWarn: false };
    }
    if (action === 'steal_score' && count > 0) {
      return { tag: '好友', event, message: `获得积分x${count}`, isWarn: false };
    }
    const actionLabel =
      action === 'steal'
        ? '偷'
        : action === 'help'
          ? '一键务农'
          : action === 'water'
            ? '浇水'
            : action === 'weed'
              ? '除草'
              : action === 'bug'
                ? '除虫'
                : action === 'bad'
                  ? '捣乱'
                  : action;
    if (action === 'steal' && count > 0) {
      const plantHint = plants.length ? `(${[...new Set(plants)].join('/')})` : '';
      return { tag: '好友', event, message: `${who}: 偷${count}${plantHint}`, isWarn: false };
    }
    return {
      tag: '好友',
      event,
      message: count > 0 ? `${who}: ${actionLabel}${count}` : `${who}: ${actionLabel}`,
      isWarn: false
    };
  }
  if (type === 'account_status') {
    const st = String(body.status || '');
    const detail = String(body.detail || '');
    const statusLabel =
      st === 'running'
        ? '运行中'
        : st === 'starting'
          ? '启动中'
          : st === 'stopped'
            ? '已停止'
            : st === 'error'
              ? '异常'
              : st;
    return {
      tag: st === 'error' ? '错误' : '系统',
      event: '账号状态',
      message: [statusLabel, detail].filter(Boolean).join(' · ') || '-',
      isWarn: st === 'error'
    };
  }
  return { tag: '系统', event: type, message: JSON.stringify(body), isWarn: false };
}

async function clearLogs() {
  const accountId = farmAccountStore.currentAccountId;
  if (accountId) {
    await fetchClearFarmLogs(accountId);
  }
  logs.value = [];
}

function onLogScroll(e: Event) {
  const el = e.target as HTMLElement;
  if (!el) return;
  autoScroll.value = el.scrollHeight - el.scrollTop - el.clientHeight < 50;
}

function getLogTagClass(tag: string) {
  if (tag === '错误') return 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-300';
  if (tag === '系统') return 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300';
  if (tag === '好友') return 'bg-orange-100 text-orange-700 dark:bg-orange-900/30 dark:text-orange-300';
  return 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-300';
}

async function loadBag() {
  if (!farmAccountStore.currentAccountId || !isOnline.value) {
    bagItems.value = [];
    couponBaseline.value = null;
    diamond.value = null;
    return;
  }
  bagLoading.value = true;
  try {
    const accountId = farmAccountStore.currentAccountId;
    const [bagRes, diamondRes] = await Promise.all([fetchGetFarmBag(accountId), fetchGetFarmDiamond(accountId)]);
    if (!bagRes.error && bagRes.data) {
      bagItems.value = bagRes.data.items || [];
      const nextCoupon = Number(bagItems.value.find(item => Number(item.id) === COUPON_ITEM_ID)?.count || 0);
      if (couponBaseline.value == null) couponBaseline.value = nextCoupon;
    }
    if (!diamondRes.error && diamondRes.data) {
      diamond.value = diamondRes.data.diamond;
    }
  } finally {
    bagLoading.value = false;
  }
}

function applyStatus(next: Api.Farm.Status) {
  status.value = next;
  syncNextChecks(next.nextChecks);
  syncUptime(Number(next.uptime || 0));
}

async function loadStatus(opts?: { silent?: boolean; withExtras?: boolean }) {
  if (!farmAccountStore.currentAccountId) {
    status.value = null;
    syncNextChecks(undefined);
    localUptime.value = 0;
    bagItems.value = [];
    couponBaseline.value = null;
    return;
  }
  const silent = Boolean(opts?.silent);
  const withExtras = opts?.withExtras !== false;
  if (!silent) detailLoading.value = true;
  try {
    const { error, data } = await fetchGetFarmStatusDetail(farmAccountStore.currentAccountId);
    if (!error && data) {
      applyStatus(data);
      if (withExtras) {
        if (data.online && data.runStatus === 1) {
          await loadBag();
        } else {
          bagItems.value = [];
          couponBaseline.value = null;
        }
      }
    }
  } finally {
    if (!silent) detailLoading.value = false;
  }
}

async function refresh() {
  await farmAccountStore.loadAccounts();
  await loadStatus();
}

const { connected, connect } = useFarmWs({
  onMessage(type, payload, raw) {
    const body = (payload && typeof payload === 'object' ? payload : {}) as Record<string, unknown>;
    const accountId = Number(body.accountId || raw?.accountId || 0);
    const current = farmAccountStore.currentAccountId;

    if (type === 'logs_snapshot') {
      const list = Array.isArray(payload)
        ? (payload as Api.Farm.LogEntry[])
        : Array.isArray(body)
          ? (body as unknown as Api.Farm.LogEntry[])
          : [];
      const filtered = current ? list.filter(e => !e.accountId || Number(e.accountId) === current) : list;
      // Prefer HTTP seed; only apply snapshot when still empty (race on connect).
      if (!logs.value.length && filtered.length) applyLogEntries(filtered);
      return;
    }

    if (type === 'logs_cleared') {
      const clearedId = Number(raw?.accountId || accountId || 0);
      if (!clearedId || !current || clearedId === current) logs.value = [];
      return;
    }

    if (type === 'status:update' || type === 'status' || type === 'account_status') {
      if (current && accountId && accountId !== current) return;
      // Avoid full-panel refetch storms (NSpin / bag flash). Match Go: apply when
      // payload carries status; otherwise silent status-only refresh.
      const bodyStatus =
        body.status && typeof body.status === 'object'
          ? (body.status as Api.Farm.Status)
          : null;
      if (bodyStatus && (bodyStatus.level != null || bodyStatus.gold != null || bodyStatus.nick)) {
        applyStatus({
          ...bodyStatus,
          accountId: accountId || current || 0,
          runStatus: (status.value?.runStatus ?? bodyStatus.runStatus ?? 1) as Api.Farm.RunStatus,
          online: bodyStatus.online ?? status.value?.online ?? true
        });
      } else {
        void loadStatus({ silent: true, withExtras: false });
      }
      if (type === 'account_status') {
        void farmAccountStore.loadAccounts();
      }
      return;
    }

    if (type === 'log' || type === 'log:new' || type === 'account_log' || type === 'worker_log') {
      if (current && accountId && accountId !== current) return;
      const formatted = formatEventMessage(type, payload);
      const rawTs = Number((payload as { ts?: number; time?: number }).ts || (payload as { time?: number }).time || 0);
      pushLog(formatted.tag, formatted.message, formatted.event, formatted.isWarn, rawTs > 0 ? rawTs : undefined);
      return;
    }

    if (['farm_operation', 'farm_tick', 'friend_interact'].includes(type)) {
      if (current && accountId && accountId !== current) return;
      if (type === 'farm_tick' || type === 'farm_operation') {
        void loadStatus({ silent: true, withExtras: false });
      }
    }
  }
});

watch(
  () => farmAccountStore.currentAccountId,
  () => {
    logs.value = [];
    couponBaseline.value = null;
    void loadStatus();
    void loadLogs();
  }
);

watch(isOnline, online => {
  if (online) {
    void loadBag();
  } else {
    bagItems.value = [];
    couponBaseline.value = null;
  }
});

onMounted(async () => {
  await refresh();
  await loadLogs();
  connect();
  countdownTimer = setInterval(tickCountdowns, 1000);
  bagTimer = setInterval(() => {
    if (isOnline.value) {
      void loadBag();
    }
  }, 30000);
});

onUnmounted(() => {
  if (countdownTimer) clearInterval(countdownTimer);
  if (bagTimer) clearInterval(bagTimer);
});
</script>

<template>
  <div class="min-h-500px flex-col-stretch gap-16px overflow-auto">
    <div class="flex-y-center justify-between">
      <h2 class="text-18px font-medium">{{ $t('page.farm.dashboard.title') }}</h2>
      <NSpace>
        <NTag v-if="connected" size="small" type="success" :bordered="false">WS</NTag>
      </NSpace>
    </div>

    <NEmpty
      v-if="!farmAccountStore.currentAccountId"
      class="py-24px"
      :description="$t('page.farm.common.selectAccount')"
    />

    <NSpin v-else :show="detailLoading">
      <NGrid cols="1 s:2 m:3" responsive="screen" :x-gap="16" :y-gap="16">
        <NGi>
          <NCard :bordered="false" size="small" class="card-wrapper h-full">
            <div class="mb-8px flex-y-center justify-between">
              <span class="text-13px text-gray-500">{{ $t('page.farm.dashboard.account') }}</span>
              <NTag size="small" type="info" :bordered="false">Lv.{{ status?.level ?? 0 }}</NTag>
            </div>
            <div class="mb-12px flex-y-center gap-10px">
              <NAvatar
                v-if="canShowAvatar"
                round
                :size="40"
                :src="avatarUrl"
                :img-props="{ referrerpolicy: 'no-referrer' }"
                @error="avatarFailed = true"
              />
              <NAvatar v-else round :size="40">{{ avatarInitial }}</NAvatar>
              <div class="truncate text-16px font-medium" :title="displayName">{{ displayName }}</div>
            </div>
            <div class="mb-4px flex-y-center justify-between text-12px text-gray-500">
              <span>EXP</span>
              <span>{{ levelProgress.current || 0 }} / {{ levelProgress.needed || '?' }}</span>
            </div>
            <NProgress type="line" :percentage="expPercent" :show-indicator="false" status="info" />
            <div class="mt-8px flex-y-center justify-between text-12px text-gray-400">
              <span>{{ $t('page.farm.dashboard.expRate') }}: {{ expRate }}</span>
              <span>{{ timeToLevel }}</span>
            </div>
            <div v-if="status?.sessionExpGained" class="mt-6px text-12px text-success">
              {{ $t('page.farm.dashboard.todayExp') }}: +{{ status.sessionExpGained }}
            </div>
          </NCard>
        </NGi>

        <NGi>
          <NCard :bordered="false" size="small" class="card-wrapper h-full">
            <div class="grid grid-cols-2 gap-x-12px gap-y-12px">
              <div class="min-w-0">
                <div class="text-12px text-gray-500">{{ $t('page.farm.dashboard.gold') }}</div>
                <div class="break-all text-18px text-warning font-semibold tabular-nums">{{ status?.gold ?? 0 }}</div>
                <div
                  v-if="status?.sessionGoldGained"
                  class="text-11px"
                  :class="(status.sessionGoldGained || 0) >= 0 ? 'text-success' : 'text-error'"
                >
                  {{ (status.sessionGoldGained || 0) > 0 ? '+' : '' }}{{ status.sessionGoldGained }}
                </div>
              </div>
              <div class="min-w-0">
                <div class="text-12px text-gray-500">{{ $t('page.farm.dashboard.coupon') }}</div>
                <div class="break-all text-18px text-success font-semibold tabular-nums">
                  {{ isOnline ? coupon : 0 }}
                </div>
                <div
                  v-if="sessionCouponGained"
                  class="text-11px"
                  :class="sessionCouponGained >= 0 ? 'text-success' : 'text-error'"
                >
                  {{ sessionCouponGained > 0 ? '+' : '' }}{{ sessionCouponGained }}
                </div>
              </div>
              <div class="min-w-0">
                <div class="text-12px text-gray-500">钻石</div>
                <div class="break-all text-18px text-info font-semibold tabular-nums">
                  {{ isOnline ? (diamond ?? '-') : '-' }}
                </div>
              </div>
              <div class="min-w-0">
                <div class="text-12px text-gray-500">{{ $t('page.farm.dashboard.goldBean') }}</div>
                <div class="break-all text-18px text-warning font-semibold tabular-nums">
                  {{ isOnline ? goldBean : 0 }}
                </div>
              </div>
            </div>

            <div class="mt-12px flex-y-center justify-between border-t border-gray-100 pt-12px dark:border-gray-700">
              <div class="flex-y-center gap-8px">
                <span class="h-8px w-8px rounded-full" :class="isOnline ? 'bg-success' : 'bg-error'" />
                <span class="text-12px font-medium">
                  {{ isOnline ? $t('page.farm.dashboard.online') : $t('page.farm.dashboard.offline') }}
                </span>
              </div>
              <span class="font-mono text-12px text-gray-400">{{ formatClock(localUptime) }}</span>
            </div>
            <div v-if="status?.lastError" class="mt-8px text-12px text-error">
              {{ status.lastError }}
            </div>
          </NCard>
        </NGi>

        <NGi>
          <NCard :bordered="false" size="small" class="card-wrapper h-full">
            <div class="mb-8px text-13px text-gray-500">{{ $t('page.farm.dashboard.fertilizerBucket') }}</div>
            <div class="mb-12px grid grid-cols-2 gap-8px">
              <div>
                <div class="text-12px text-gray-400">{{ $t('page.farm.dashboard.fertNormal') }}</div>
                <div class="font-medium">{{ formatBucketTime(fertilizerNormal) }}</div>
              </div>
              <div>
                <div class="text-12px text-gray-400">{{ $t('page.farm.dashboard.fertOrganic') }}</div>
                <div class="font-medium">{{ formatBucketTime(fertilizerOrganic) }}</div>
              </div>
            </div>
            <div class="mb-8px border-t border-gray-100 pt-12px text-13px text-gray-500 dark:border-gray-700">
              {{ $t('page.farm.dashboard.collectionPoints') }}
            </div>
            <div class="grid grid-cols-2 gap-8px">
              <div>
                <div class="text-12px text-gray-400">{{ $t('page.farm.dashboard.collectionNormal') }}</div>
                <div class="font-medium">{{ collectionNormal?.count ?? 0 }}</div>
              </div>
              <div>
                <div class="text-12px text-gray-400">{{ $t('page.farm.dashboard.collectionRare') }}</div>
                <div class="font-medium">{{ collectionRare?.count ?? 0 }}</div>
              </div>
            </div>
            <div v-if="bagLoading" class="mt-8px text-12px text-gray-400">loading...</div>
          </NCard>
        </NGi>
      </NGrid>
    </NSpin>

    <div class="grid gap-16px md:grid-cols-[minmax(0,1fr)_280px]">
      <NCard :bordered="false" size="small" class="card-wrapper">
        <template #header>
          <div class="flex flex-wrap items-center justify-between gap-8px">
            <span>{{ $t('page.farm.dashboard.runningLogs') }}</span>
            <NSpace size="small">
              <NSelect v-model:value="filterModule" size="small" class="w-100px" :options="MODULE_OPTIONS" />
              <NSelect v-model:value="filterEvent" size="small" class="w-120px" :options="LOG_EVENT_FILTER_OPTIONS" />
              <NSelect v-model:value="filterLevel" size="small" class="w-100px" :options="LEVEL_OPTIONS" />
              <NInput
                v-model:value="filterKeyword"
                size="small"
                class="w-120px"
                :placeholder="$t('page.farm.dashboard.keyword')"
                clearable
              />
              <NButton size="small" @click="clearLogs">{{ $t('page.farm.dashboard.clearLogs') }}</NButton>
            </NSpace>
          </div>
        </template>
        <div
          ref="logContainer"
          class="h-300px overflow-y-auto rounded-8px bg-gray-50 p-12px font-mono text-13px dark:bg-gray-900"
          @scroll="onLogScroll"
        >
          <div v-if="!filteredLogs.length" class="py-32px text-center text-gray-400">
            {{ $t('page.farm.dashboard.noEvents') }}
          </div>
          <div v-for="log in filteredLogs" :key="log.id" class="mb-6px break-all">
            <span class="mr-8px text-gray-400">[{{ log.time }}]</span>
            <span class="mr-8px rounded-full px-6px py-1px text-11px font-bold" :class="getLogTagClass(log.tag)">
              {{ log.tag }}
            </span>
            <span
              v-if="shouldShowEventChip(log.tag, log.event)"
              class="mr-8px rounded-full bg-blue-50 px-6px py-1px text-11px text-blue-500 dark:bg-blue-900/20"
            >
              {{ getLogEventLabel(log.event) }}
            </span>
            <span :class="log.isWarn ? 'text-error' : ''">{{ log.message }}</span>
          </div>
        </div>
      </NCard>

      <div class="flex-col gap-16px">
        <NCard :title="$t('page.farm.dashboard.nextChecksTitle')" :bordered="false" size="small" class="card-wrapper">
          <div class="flex-col gap-12px">
            <div class="flex-y-center justify-between">
              <span>🌱 {{ $t('page.farm.dashboard.nextCheckFarm') }}</span>
              <span class="font-mono font-semibold">{{ nextFarmCheck }}</span>
            </div>
            <div class="flex-y-center justify-between">
              <span>🤝 {{ $t('page.farm.dashboard.nextCheckHelp') }}</span>
              <span class="font-mono font-semibold">{{ nextHelpCheck }}</span>
            </div>
            <div class="flex-y-center justify-between">
              <span>🏃 {{ $t('page.farm.dashboard.nextCheckSteal') }}</span>
              <span class="font-mono font-semibold">{{ nextStealCheck }}</span>
            </div>
          </div>
        </NCard>

        <NCard :title="$t('page.farm.dashboard.todayStats')" :bordered="false" size="small" class="card-wrapper">
          <div v-if="!isOnline" class="py-24px text-center text-gray-400">
            <div class="mb-8px text-28px">📡</div>
            <div>{{ $t('page.farm.dashboard.accountOffline') }}</div>
            <div class="mt-4px text-12px">{{ $t('page.farm.personal.notRunning') }}</div>
          </div>
          <div v-else class="grid grid-cols-2 gap-8px">
            <div
              v-for="item in filteredOperations"
              :key="item.key"
              class="flex-y-center justify-between rounded-8px bg-gray-50 px-10px py-8px dark:bg-gray-800"
            >
              <span class="text-13px">
                {{ OP_META[item.key]?.icon || '⭕' }}
                {{ OP_META[item.key]?.label || item.key }}
              </span>
              <span class="font-semibold">{{ item.value }}</span>
            </div>
          </div>
        </NCard>
      </div>
    </div>
  </div>
</template>
