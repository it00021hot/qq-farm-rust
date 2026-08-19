<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NAvatar, NButton, NCollapse, NCollapseItem, NEmpty, NInput, NTag } from 'naive-ui';
import { $t } from '@/locales';
import { resolveCatalogImage } from '@/views/farm/game-config/shared';

export type QixiReward = {
  id?: string | number;
  name?: string;
  count?: number | string;
  image?: string;
};

export type QixiStage = {
  id?: string;
  stage?: number;
  completed?: boolean;
  claimed?: boolean;
  claimable?: boolean;
  current?: boolean;
  cost?: QixiReward;
  rewards?: QixiReward[];
};

export type QixiActivity = {
  name?: string;
  title?: string;
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
    bridge?: { enabled?: boolean };
    gift?: { enabled?: boolean };
  };
  rules?: {
    title?: string;
    paragraphs?: string[];
  };
};

const props = defineProps<{
  activity: QixiActivity;
  friends: Api.Farm.Friend[];
  friendsLoading: boolean;
  pendingBridge: boolean;
  pendingGift: boolean;
}>();

const emit = defineEmits<{
  claimBridge: [];
  gift: [payload: { friendGid: string; count: number }];
  refreshFriends: [];
}>();

const search = ref('');
const selectedFriendGid = ref('');
const giftCount = ref(1);
const failedAvatars = ref(new Set<string>());

const sachetBalance = computed(() => {
  const value = Number(props.activity?.balances?.sachet || 0);
  return Number.isFinite(value) && value > 0 ? Math.trunc(value) : 0;
});

const stages = computed(() => props.activity?.bridge?.stages || []);
const ruleParagraphs = computed(() => {
  const rules = props.activity?.rules as { paragraphs?: unknown; tips?: { txt?: unknown }; lines?: unknown } | undefined;
  if (!rules) return [] as string[];
  const fromParagraphs = Array.isArray(rules.paragraphs) ? rules.paragraphs : [];
  const fromTips = Array.isArray(rules.tips?.txt) ? rules.tips.txt : [];
  const fromLines = Array.isArray(rules.lines) ? rules.lines : [];
  return [...fromParagraphs, ...fromTips, ...fromLines]
    .map(line => String(line || '').trim())
    .filter(Boolean);
});
const ruleTitle = computed(
  () => String(props.activity?.rules?.title || '').trim() || $t('page.farm.activity.qixiRules')
);

const filteredFriends = computed(() => {
  const keyword = search.value.trim().toLowerCase();
  const source = props.friends.filter(friend => Number(friend.gid) > 0);
  if (!keyword) return source;
  return source.filter(friend => friendName(friend).toLowerCase().includes(keyword));
});

const selectedFriend = computed(
  () => props.friends.find(friend => String(friend.gid) === selectedFriendGid.value) || null
);

const canClaimBridge = computed(
  () => props.activity?.actions?.bridge?.enabled === true && props.activity?.bridge?.claimable === true
);

const canGift = computed(() => props.activity?.actions?.gift?.enabled === true && sachetBalance.value > 0);

const giftDisabled = computed(
  () =>
    !selectedFriend.value ||
    giftCount.value < 1 ||
    giftCount.value > Math.max(1, sachetBalance.value) ||
    props.pendingGift ||
    !canGift.value
);

watch(sachetBalance, balance => {
  if (balance <= 0) giftCount.value = 1;
  else if (giftCount.value > balance) giftCount.value = balance;
});

function friendName(friend: Api.Farm.Friend) {
  return (
    String(friend.nickname || friend.name || '').trim() || $t('page.farm.activity.qixiFriendFallback')
  );
}

function friendAvatar(friend: Api.Farm.Friend) {
  return String(friend.avatar || friend.avatarUrl || friend.avatar_url || '');
}

function itemLabel(item?: QixiReward, fallback = '') {
  const name = String(item?.name || '').trim();
  if (name && !/^\d+$/.test(name)) return name;
  return fallback || name;
}

function itemImage(item?: QixiReward) {
  return resolveCatalogImage(item?.image);
}

function balanceText(value?: string | null) {
  if (props.activity?.balances?.known === false) return '--';
  return value || '0';
}

function stageState(stage: QixiStage) {
  if (stage.claimable) return $t('page.farm.activity.qixiStageClaimable');
  if (stage.claimed) return $t('page.farm.activity.qixiStageDone');
  if (stage.current) return $t('page.farm.activity.qixiStageCurrent');
  return stage.completed ? $t('page.farm.activity.qixiStageDone') : $t('page.farm.activity.qixiStageLocked');
}

function setGiftCount(value: unknown) {
  const parsed = Math.trunc(Number(value));
  const next = Number.isFinite(parsed) ? parsed : 1;
  giftCount.value = Math.max(1, Math.min(sachetBalance.value || 1, next));
}

function submitGift() {
  if (giftDisabled.value || !selectedFriend.value) return;
  emit('gift', {
    friendGid: String(selectedFriend.value.gid),
    count: Math.trunc(giftCount.value)
  });
}

function markAvatarFailed(gid: string | number) {
  failedAvatars.value = new Set(failedAvatars.value).add(String(gid));
}
</script>

<template>
  <div class="flex-col gap-16px">
    <div class="grid gap-8px sm:grid-cols-2 xl:grid-cols-4">
      <div class="flex items-center gap-10px rounded-8px bg-gray-50 px-12px py-10px dark:bg-gray-800">
        <img v-if="itemImage(activity.feather)" :src="itemImage(activity.feather)" class="h-36px w-36px object-contain" />
        <div>
          <div class="text-12px text-gray-500">{{ itemLabel(activity.feather, $t('page.farm.activity.qixiFeather')) }}</div>
          <div class="text-16px font-semibold">{{ balanceText(activity.balances?.feather) }}</div>
        </div>
      </div>
      <div class="flex items-center gap-10px rounded-8px bg-gray-50 px-12px py-10px dark:bg-gray-800">
        <img v-if="itemImage(activity.sachet)" :src="itemImage(activity.sachet)" class="h-36px w-36px object-contain" />
        <div>
          <div class="text-12px text-gray-500">{{ itemLabel(activity.sachet, $t('page.farm.activity.qixiSachet')) }}</div>
          <div class="text-16px font-semibold">{{ balanceText(activity.balances?.sachet) }}</div>
        </div>
      </div>
      <div class="flex items-center gap-10px rounded-8px bg-gray-50 px-12px py-10px dark:bg-gray-800">
        <img
          v-if="itemImage(activity.receivedSachet)"
          :src="itemImage(activity.receivedSachet)"
          class="h-36px w-36px object-contain"
        />
        <div>
          <div class="text-12px text-gray-500">{{ $t('page.farm.activity.qixiReceived') }}</div>
          <div class="text-16px font-semibold">{{ balanceText(activity.balances?.receivedSachet) }}</div>
        </div>
      </div>
      <div class="flex items-center gap-10px rounded-8px bg-gray-50 px-12px py-10px dark:bg-gray-800">
        <div>
          <div class="text-12px text-gray-500">{{ $t('page.farm.activity.qixiSentFriends') }}</div>
          <div class="text-16px font-semibold">{{ activity.gift?.sentCount || '0' }}</div>
        </div>
      </div>
    </div>

    <div class="rounded-8px border border-gray-200 px-16px py-14px dark:border-gray-700">
      <div class="mb-12px flex flex-wrap items-center justify-between gap-8px">
        <div>
          <div class="text-12px text-gray-500">{{ $t('page.farm.activity.qixiProgress') }}</div>
          <div class="text-16px font-medium">
            {{ $t('page.farm.activity.qixiCurrentStage', { stage: activity.bridge?.currentStage || 0 }) }}
          </div>
        </div>
        <NButton type="primary" :loading="pendingBridge" :disabled="!canClaimBridge" @click="emit('claimBridge')">
          {{
            pendingBridge
              ? $t('page.farm.activity.claiming')
              : canClaimBridge
                ? $t('page.farm.activity.qixiClaimBridge')
                : $t('page.farm.activity.qixiNoReward')
          }}
        </NButton>
      </div>

      <div class="mb-12px grid grid-cols-3 gap-6px">
        <div
          v-for="stage in stages"
          :key="`track-${stage.id || stage.stage}`"
          class="h-8px rounded-4px"
          :class="stage.completed || stage.claimed ? 'bg-primary' : stage.current ? 'bg-primary/40' : 'bg-gray-200 dark:bg-gray-700'"
        />
      </div>

      <NEmpty v-if="!stages.length" :description="$t('page.farm.activity.qixiNoStage')" />
      <div v-else class="grid gap-12px lg:grid-cols-3">
        <div
          v-for="stage in stages"
          :key="String(stage.id || stage.stage)"
          class="rounded-8px border px-12px py-12px dark:border-gray-700"
          :class="stage.current ? 'border-primary' : 'border-gray-200'"
        >
          <div class="mb-10px flex items-center justify-between gap-8px">
            <span class="text-13px font-medium">{{ $t('page.farm.activity.qixiStage', { stage: stage.stage || 0 }) }}</span>
            <NTag size="tiny" :type="stage.claimable ? 'error' : stage.current ? 'warning' : 'default'" :bordered="false">
              {{ stageState(stage) }}
            </NTag>
          </div>
          <div class="mb-10px flex items-center gap-8px text-12px text-gray-500">
            <img v-if="itemImage(stage.cost)" :src="itemImage(stage.cost)" class="h-24px w-24px object-contain" />
            <span>{{ $t('page.farm.activity.qixiNeed') }} {{ itemLabel(stage.cost, $t('page.farm.activity.qixiFeather')) }}</span>
            <strong class="ml-auto text-13px text-[var(--n-text-color)]">{{ stage.cost?.count || 0 }}</strong>
          </div>
          <div class="flex-col gap-6px">
            <div v-for="reward in stage.rewards || []" :key="String(reward.id || reward.name)" class="flex items-center gap-8px text-12px">
              <img v-if="itemImage(reward)" :src="itemImage(reward)" class="h-24px w-24px object-contain" />
              <span class="min-w-0 flex-1 truncate">{{ itemLabel(reward) }}</span>
              <span>×{{ reward.count || 0 }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div class="grid gap-16px lg:grid-cols-[minmax(0,1.4fr)_minmax(280px,0.8fr)]">
      <div class="rounded-8px border border-gray-200 px-12px py-12px dark:border-gray-700">
        <div class="mb-10px flex items-center justify-between gap-8px">
          <div class="text-13px font-medium">{{ $t('page.farm.activity.qixiGift') }}</div>
          <NButton size="small" :loading="friendsLoading" @click="emit('refreshFriends')">
            <template #icon>
              <icon-ic-round-refresh class="text-icon" />
            </template>
            {{ $t('page.farm.friends.refreshList') }}
          </NButton>
        </div>
        <NInput v-model:value="search" size="small" clearable :placeholder="$t('page.farm.activity.qixiSearchFriend')" />
        <div class="mt-10px max-h-320px overflow-auto">
          <NEmpty
            v-if="!filteredFriends.length && !friendsLoading"
            class="py-24px"
            :description="$t('page.farm.activity.qixiNoFriend')"
          />
          <button
            v-for="friend in filteredFriends"
            :key="String(friend.gid)"
            type="button"
            class="mb-6px flex w-full items-center gap-10px rounded-8px border px-8px py-8px text-left dark:border-gray-700"
            :class="
              String(friend.gid) === selectedFriendGid
                ? 'border-primary bg-primary/8'
                : 'border-gray-200 hover:bg-gray-50 dark:hover:bg-gray-800'
            "
            @click="selectedFriendGid = String(friend.gid)"
          >
            <NAvatar
              v-if="friendAvatar(friend) && !failedAvatars.has(String(friend.gid))"
              round
              :size="40"
              :src="friendAvatar(friend)"
              @error="() => markAvatarFailed(friend.gid)"
            />
            <NAvatar v-else round :size="40">{{ friendName(friend).slice(0, 1) }}</NAvatar>
            <div class="min-w-0 flex-1">
              <div class="truncate text-13px font-medium">{{ friendName(friend) }}</div>
              <NTag v-if="friend.level" size="tiny" :bordered="false" class="mt-4px">Lv{{ friend.level }}</NTag>
            </div>
          </button>
        </div>
      </div>

      <div class="rounded-8px border border-gray-200 px-14px py-14px dark:border-gray-700">
        <div v-if="selectedFriend" class="mb-12px">
          <div class="text-12px text-gray-500">{{ $t('page.farm.activity.qixiGiftTo') }}</div>
          <div class="mt-4px truncate text-15px font-medium">{{ friendName(selectedFriend) }}</div>
          <NTag v-if="selectedFriend.level" size="tiny" :bordered="false" class="mt-6px">Lv{{ selectedFriend.level }}</NTag>
        </div>
        <div v-else class="mb-12px text-13px text-gray-500">{{ $t('page.farm.activity.qixiSelectFriend') }}</div>

        <div class="mb-12px flex items-center gap-10px">
          <img v-if="itemImage(activity.sachet)" :src="itemImage(activity.sachet)" class="h-40px w-40px object-contain" />
          <div>
            <div class="text-12px text-gray-500">{{ itemLabel(activity.sachet, $t('page.farm.activity.qixiSachet')) }}</div>
            <div class="text-13px font-medium">
              {{ $t('page.farm.activity.qixiAvailable', { count: balanceText(activity.balances?.sachet) }) }}
            </div>
          </div>
        </div>

        <div class="mb-12px flex items-center gap-6px">
          <NButton size="small" :disabled="giftCount <= 1 || pendingGift" @click="setGiftCount(giftCount - 1)">−</NButton>
          <input
            class="h-32px w-72px rounded-6px border border-gray-300 bg-transparent text-center text-14px dark:border-gray-600"
            :value="giftCount"
            type="number"
            min="1"
            :max="Math.max(1, sachetBalance)"
            :disabled="pendingGift"
            @input="setGiftCount(($event.target as HTMLInputElement).value)"
          />
          <NButton
            size="small"
            :disabled="giftCount >= sachetBalance || pendingGift || sachetBalance <= 0"
            @click="setGiftCount(giftCount + 1)"
          >
            +
          </NButton>
          <NButton size="small" :disabled="sachetBalance <= 0 || giftCount >= sachetBalance || pendingGift" @click="setGiftCount(sachetBalance)">
            {{ $t('page.farm.activity.qixiMax') }}
          </NButton>
        </div>

        <NButton type="primary" block :loading="pendingGift" :disabled="giftDisabled" @click="submitGift">
          {{ pendingGift ? $t('page.farm.activity.claiming') : $t('page.farm.activity.qixiGift') }}
        </NButton>
      </div>
    </div>

    <NCollapse v-if="ruleParagraphs.length" :default-expanded-names="['rules']">
      <NCollapseItem :title="ruleTitle" name="rules">
        <p
          v-for="(line, index) in ruleParagraphs"
          :key="`${index}-${line.slice(0, 24)}`"
          class="mb-8px whitespace-pre-line text-13px leading-22px text-gray-600 last:mb-0 dark:text-gray-300"
        >
          {{ line }}
        </p>
      </NCollapseItem>
    </NCollapse>
  </div>
</template>
