<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { NButton, NCard, NEmpty, NRadio, NRadioGroup, NSpin, NTabPane, NTabs, NTag, useMessage } from 'naive-ui';
import { fetchGetFarmActivityAutumn, fetchOperateFarmActivityAutumn } from '@/service/api';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { resolveCatalogImage } from '@/views/farm/game-config/shared';
import dayjs from 'dayjs';

defineOptions({ name: 'FarmActivityAutumnView' });

const props = defineProps<{ gameplay: 'autumnWish' | 'autumnHappy' }>();

type AutumnItem = { id?: number; count?: number | string; name?: string; image?: string; rarity?: number };
type WishChoice = { id: number; name: string };
type WishRewardDay = { day: number; reward: AutumnItem };
type WishPending = { chooseId: number; textId: number; day: number; text?: string; rewards?: AutumnItem[] };
type Milestone = { id: string; threshold: number; state: number; rewards?: AutumnItem[] };
type AutumnLogRow = { seq: number; kind: number; score: number; createdAt: number; actor?: { name?: string } | null };

type AutumnState = {
  key?: 'wish' | 'happy';
  id?: string;
  title?: string;
  serverTime?: number;
  startTime?: number;
  endTime?: number;
  active?: boolean;
  rules?: string[];
  // wish
  remaining?: number;
  day?: number;
  choices?: WishChoice[];
  rewardDays?: WishRewardDay[];
  pending?: WishPending | null;
  canDraw?: boolean;
  canClaim?: boolean;
  // happy
  score?: number;
  scoreItemId?: number;
  dailyReward?: number;
  firstShareReward?: number;
  claimedCount?: number;
  claimLimit?: number;
  poolClaimedCount?: number;
  poolClaimLimit?: number;
  canClaimDaily?: boolean;
  canShare?: boolean;
  firstShareAwarded?: boolean;
  canClaimMilestones?: boolean;
  milestones?: Milestone[];
};

const farmAccountStore = useFarmAccountStore();
const message = useMessage();

const loading = ref(false);
const state = ref<AutumnState | null>(null);
const drawChooseId = ref<number>(0);
const pendingKey = ref('');
const logTab = ref<0 | 1>(1);
const logs = ref<AutumnLogRow[]>([]);
const logsTotal = ref(0);
const logsLoading = ref(false);

const eventKey = computed<'wish' | 'happy'>(() => (props.gameplay === 'autumnHappy' ? 'happy' : 'wish'));

function fmtTime(ms?: number) {
  if (!ms) return '--';
  return dayjs(ms).format('MM-DD HH:mm');
}

function img(item?: AutumnItem) {
  return resolveCatalogImage(item?.image);
}

function itemCount(item?: AutumnItem) {
  return Number(item?.count ?? 0);
}

function stripErrorCode(text?: string) {
  return String(text || '').replace(/^[A-Z_]+：/, '') || '操作失败';
}

async function load() {
  if (!farmAccountStore.currentAccountId) return;
  loading.value = true;
  try {
    const { error, data } = await fetchGetFarmActivityAutumn(farmAccountStore.currentAccountId, eventKey.value);
    if (error) {
      message.error(stripErrorCode(error.message));
      return;
    }
    state.value = (data || null) as AutumnState | null;
    if (state.value && !drawChooseId.value) {
      drawChooseId.value = state.value.choices?.[0]?.id ?? 0;
    }
  } finally {
    loading.value = false;
  }
}

async function operate(action: string, params: Record<string, unknown> = {}) {
  if (!farmAccountStore.currentAccountId) return;
  pendingKey.value = action;
  try {
    const { error, data } = await fetchOperateFarmActivityAutumn(
      farmAccountStore.currentAccountId,
      eventKey.value,
      action,
      params
    );
    if (error) {
      message.error(stripErrorCode(error.message));
      return;
    }
    const rewards = (data?.rewards || []) as AutumnItem[];
    const gained = rewards.map(r => `${r.name}x${r.count}`).filter(Boolean).join('、');
    message.success(gained ? `领取成功：${gained}` : '操作成功');
    if (data?.activity) {
      state.value = data.activity as AutumnState;
    } else {
      await load();
    }
  } finally {
    pendingKey.value = '';
  }
}

async function loadLogs() {
  if (!farmAccountStore.currentAccountId) return;
  logsLoading.value = true;
  try {
    const { error, data } = await fetchOperateFarmActivityAutumn(farmAccountStore.currentAccountId, eventKey.value, 'logs', {
      tab: logTab.value
    });
    if (error) {
      message.error(stripErrorCode(error.message));
      return;
    }
    const result = (data?.result || {}) as { total?: number; logs?: AutumnLogRow[] };
    logs.value = result.logs || [];
    logsTotal.value = Number(result.total ?? logs.value.length);
  } finally {
    logsLoading.value = false;
  }
}

function onLogTab(tab: number) {
  logTab.value = tab === 0 ? 0 : 1;
  loadLogs();
}

async function draw() {
  if (!state.value?.canDraw || !drawChooseId.value) return;
  await operate('draw', { chooseId: drawChooseId.value });
}

onMounted(() => {
  load();
  if (eventKey.value === 'happy') loadLogs();
});
</script>

<template>
  <NSpin :show="loading">
    <NCard :bordered="false" size="small" class="card-wrapper">
      <template v-if="!state">
        <NEmpty :description="state === null ? '活动动态状态未返回' : '加载中'" />
      </template>
      <template v-else>
        <div class="mb-12px flex flex-wrap items-center gap-12px">
          <img
            :src="eventKey === 'wish' ? '/activity-assets/autumn/wish.png' : '/activity-assets/autumn/happy.png'"
            alt="autumn"
            class="h-40px w-40px rounded-8px object-contain"
          />
          <div class="text-16px font-medium">{{ state.title }}</div>
          <NTag size="small" :type="state.active ? 'success' : 'default'" :bordered="false">
            {{ state.active ? '进行中' : '已结束' }}
          </NTag>
          <span class="text-12px text-gray-500">{{ fmtTime(state.startTime) }} ~ {{ fmtTime(state.endTime) }}</span>
          <NButton size="tiny" quaternary :loading="loading" @click="load">刷新</NButton>
        </div>
        <div v-if="state.rules?.length" class="mb-12px rounded-8px bg-gray-100 px-14px py-10px text-12px text-gray-500 dark:bg-gray-800">
          <div v-for="(rule, i) in state.rules" :key="i">{{ rule }}</div>
        </div>

        <!-- 秋祈良愿 -->
        <template v-if="eventKey === 'wish'">
          <div class="mb-12px flex flex-wrap items-center gap-12px">
            <NTag size="small" :bordered="false">第 {{ state.day ?? 0 }} 天</NTag>
            <NTag size="small" type="info" :bordered="false">今日剩余祈愿 {{ state.remaining ?? 0 }} 次</NTag>
          </div>
          <NRadioGroup v-model:value="drawChooseId" :disabled="!state.canDraw" class="mb-12px">
            <NRadio v-for="choice in state.choices" :key="choice.id" :value="choice.id" class="mr-16px">
              {{ choice.name }}
            </NRadio>
          </NRadioGroup>
          <div class="mb-16px">
            <NButton
              size="small"
              type="primary"
              :disabled="!state.canDraw || !drawChooseId"
              :loading="pendingKey === 'draw'"
              @click="draw"
            >
              {{ state.canDraw ? '抽签祈愿' : state.pending ? '先领取待领奖励' : '今日祈愿已完成' }}
            </NButton>
          </div>
          <NCard v-if="state.pending" :bordered="true" size="small" class="mb-12px rounded-8px">
            <div class="mb-6px text-14px font-medium">
              {{ state.pending.text || '祈愿结果' }}
              <NTag size="tiny" class="ml-8px" :bordered="false">第 {{ state.pending.day }} 天</NTag>
            </div>
            <div class="flex flex-wrap items-center gap-12px">
              <div v-for="(reward, i) in state.pending.rewards || []" :key="i" class="flex items-center gap-4px">
                <img :src="img(reward)" alt="" class="h-28px w-28px object-contain" />
                <span class="text-12px">{{ reward.name }} x{{ itemCount(reward) }}</span>
              </div>
              <NButton
                size="tiny"
                type="primary"
                :disabled="!state.canClaim"
                :loading="pendingKey === 'claim'"
                @click="operate('claim')"
              >
                领取奖励
              </NButton>
            </div>
          </NCard>
          <div class="text-13px font-medium mb-6px">连续祈愿奖励</div>
          <div class="flex flex-wrap gap-12px">
            <div
              v-for="entry in state.rewardDays || []"
              :key="entry.day"
              class="flex flex-col items-center rounded-8px bg-gray-50 px-12px py-8px dark:bg-gray-800"
            >
              <span class="text-12px text-gray-500">第 {{ entry.day }} 天</span>
              <img :src="img(entry.reward)" alt="" class="my-2px h-32px w-32px object-contain" />
              <span class="text-12px">{{ entry.reward.name }} x{{ itemCount(entry.reward) }}</span>
            </div>
          </div>
        </template>

        <!-- 快乐不独享 -->
        <template v-else>
          <div class="mb-12px flex flex-wrap items-center gap-12px">
            <NTag size="small" type="info" :bordered="false">当前快乐值 {{ state.score ?? 0 }}</NTag>
            <NTag size="small" :bordered="false">
              今日领取 {{ state.claimedCount ?? 0 }}/{{ state.claimLimit ?? 0 }}
            </NTag>
            <NTag size="small" :bordered="false">
              池子 {{ state.poolClaimedCount ?? 0 }}/{{ state.poolClaimLimit ?? 0 }}
            </NTag>
            <NTag v-if="state.firstShareAwarded" size="small" type="success" :bordered="false">首次分享奖励已领</NTag>
          </div>
          <div class="mb-16px flex flex-wrap gap-12px">
            <NButton
              size="small"
              type="primary"
              :disabled="!state.canClaimDaily"
              :loading="pendingKey === 'daily'"
              @click="operate('daily')"
            >
              领取每日快乐值
            </NButton>
            <NButton
              size="small"
              :disabled="!state.canShare"
              :loading="pendingKey === 'share'"
              @click="operate('share')"
            >
              {{ state.firstShareAwarded ? '分享（+快乐值）' : `分享领首次奖励 +${state.firstShareReward ?? 0}` }}
            </NButton>
            <NButton
              size="small"
              :disabled="!state.canClaimMilestones"
              :loading="pendingKey === 'milestones'"
              @click="operate('milestones')"
            >
              领取全部档位奖励
            </NButton>
          </div>
          <div class="mb-16px">
            <div class="text-13px font-medium mb-6px">快乐值档位</div>
            <div class="flex flex-wrap gap-12px">
              <div
                v-for="milestone in state.milestones || []"
                :key="milestone.id"
                class="flex flex-col items-center rounded-8px px-12px py-8px"
                :class="milestone.state === 2 ? 'bg-emerald-50 dark:bg-emerald-900/20' : 'bg-gray-50 dark:bg-gray-800'"
              >
                <span class="text-12px text-gray-500">{{ milestone.threshold }} 快乐值</span>
                <div class="my-2px flex gap-6px">
                  <div v-for="(reward, i) in milestone.rewards || []" :key="i" class="flex flex-col items-center">
                    <img :src="img(reward)" alt="" class="h-28px w-28px object-contain" />
                    <span class="text-12px">x{{ itemCount(reward) }}</span>
                  </div>
                </div>
                <NTag size="tiny" :type="milestone.state === 2 ? 'success' : milestone.state === 3 ? 'info' : 'default'" :bordered="false">
                  {{ milestone.state === 2 ? '可领取' : milestone.state === 3 ? '已领取' : '未达成' }}
                </NTag>
              </div>
            </div>
          </div>
          <NTabs :value="logTab" type="segment" size="small" class="max-w-420px" @update:value="onLogTab">
            <!-- 自闭合 NTabPane 的 label 在 naive-ui 2.45 会渲染成空，走 #tab 插槽 -->
            <NTabPane :name="0">
              <template #tab>领取我的</template>
            </NTabPane>
            <NTabPane :name="1">
              <template #tab>我领取的</template>
            </NTabPane>
          </NTabs>
          <NSpin :show="logsLoading">
            <div v-if="!logs.length" class="py-16px"><NEmpty size="small" description="暂无互动记录" /></div>
            <div v-else class="mt-8px flex flex-col gap-4px">
              <div v-for="row in logs" :key="row.seq" class="flex items-center justify-between rounded-6px bg-gray-50 px-10px py-6px text-12px dark:bg-gray-800">
                <span>{{ row.actor?.name || `玩家#${row.seq}` }}</span>
                <span class="text-gray-500">快乐值 {{ row.score > 0 ? `+${row.score}` : row.score }} · {{ dayjs(row.createdAt * 1000).format('MM-DD HH:mm') }}</span>
              </div>
            </div>
          </NSpin>
        </template>
      </template>
    </NCard>
  </NSpin>
</template>
