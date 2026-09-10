<script setup lang="ts">
import { computed, ref } from 'vue';
import { NButton, NCollapse, NCollapseItem, NEmpty, NProgress, NTag } from 'naive-ui';
import { $t } from '@/locales';
import { resolveCatalogImage } from '@/views/farm/game-config/shared';

export type CharityReward = {
  id?: string | number;
  name?: string;
  count?: number | string;
  image?: string;
};

export type CharityProgressReward = {
  target?: string;
  reward?: CharityReward;
  statusCode?: string;
  reached?: boolean;
  claimed?: boolean;
  claimable?: boolean;
  claimSupported?: boolean;
};

export type CharityActivity = {
  groupId?: string;
  activityId?: string;
  name?: string;
  title?: string;
  startTime?: string;
  endTime?: string;
  active?: boolean;
  love?: CharityReward;
  loveBalance?: string;
  donatedLove?: string;
  flowStatus?: string;
  seedReward?: {
    statusCode?: string;
    claimable?: boolean;
    claimed?: boolean;
    reward?: CharityReward;
  };
  dailyGift?: {
    statusCode?: string;
    claimed?: boolean;
    harvestedToday?: boolean;
    reward?: CharityReward;
    publicFund?: { date?: string; statusCode?: string } | null;
  };
  progressRewards?: CharityProgressReward[];
  globalProgress?: {
    donated?: string;
    target?: string;
    reached?: boolean;
    rewardTarget?: string;
    reward?: CharityReward;
  };
  settlement?: {
    requiredLove?: string;
    eligible?: boolean;
    reward?: CharityReward;
  };
  actions?: {
    claimSeeds?: { enabled?: boolean };
    donateLove?: { enabled?: boolean; count?: number };
    claimDailyGift?: { enabled?: boolean };
  };
  rules?: {
    title?: string;
    paragraphs?: string[];
  };
};

const props = defineProps<{
  activity: CharityActivity;
  pendingSeeds: boolean;
  pendingDonate: boolean;
  pendingGift: boolean;
  pendingProgress: boolean;
}>();

const emit = defineEmits<{
  claimSeeds: [];
  donateLove: [];
  claimDailyGift: [];
  claimProgress: [target: string];
}>();

const confirmingDonate = ref(false);

const canClaimSeeds = computed(() => props.activity?.actions?.claimSeeds?.enabled === true);
const canDonate = computed(() => props.activity?.actions?.donateLove?.enabled === true);
const canClaimGift = computed(() => props.activity?.actions?.claimDailyGift?.enabled === true);

const globalDonated = computed(() => Number(props.activity?.globalProgress?.donated || 0));
const globalTarget = computed(() => Number(props.activity?.globalProgress?.target || 0));
const globalPercent = computed(() => {
  if (!globalTarget.value) return 0;
  return Math.min(100, Math.max(0, (globalDonated.value / globalTarget.value) * 100));
});

const progressRewards = computed(() => props.activity?.progressRewards || []);

const ruleParagraphs = computed(() => {
  const rules = props.activity?.rules as
    | { paragraphs?: unknown; tips?: { txt?: unknown }; lines?: unknown }
    | undefined;
  if (!rules) return [] as string[];
  const fromParagraphs = Array.isArray(rules.paragraphs) ? rules.paragraphs : [];
  const fromTips = Array.isArray(rules.tips?.txt) ? rules.tips.txt : [];
  const fromLines = Array.isArray(rules.lines) ? rules.lines : [];
  return [...fromParagraphs, ...fromTips, ...fromLines].map(line => String(line || '').trim()).filter(Boolean);
});
const ruleTitle = computed(
  () => String(props.activity?.rules?.title || '').trim() || $t('page.farm.activity.charityRules')
);

function itemLabel(item?: CharityReward, fallback = '') {
  const name = String(item?.name || '').trim();
  if (name && !/^\d+$/.test(name)) return name;
  return fallback || name;
}

function itemImage(item?: CharityReward) {
  return resolveCatalogImage(item?.image);
}

function donate() {
  if (props.pendingDonate) return;
  if (!confirmingDonate.value) {
    confirmingDonate.value = true;
    return;
  }
  confirmingDonate.value = false;
  emit('donateLove');
}

function seedsButtonLabel() {
  if (props.pendingSeeds) return $t('page.farm.activity.claiming');
  if (props.activity?.seedReward?.claimed) return $t('page.farm.activity.charitySeedsClaimed');
  return canClaimSeeds.value ? $t('page.farm.activity.charityClaimSeeds') : $t('page.farm.activity.claimUnavailable');
}

function giftButtonLabel() {
  if (props.pendingGift) return $t('page.farm.activity.claiming');
  if (props.activity?.dailyGift?.claimed) return $t('page.farm.activity.charityGiftClaimed');
  return canClaimGift.value ? $t('page.farm.activity.charityClaimGift') : $t('page.farm.activity.claimUnavailable');
}
</script>

<template>
  <div class="flex-col gap-16px">
    <div class="grid gap-8px sm:grid-cols-2 xl:grid-cols-4">
      <div class="flex items-center gap-10px rounded-8px bg-gray-50 px-12px py-10px dark:bg-gray-800">
        <img v-if="itemImage(activity.love)" :src="itemImage(activity.love)" class="h-36px w-36px object-contain" />
        <div>
          <div class="text-12px text-gray-500">
            {{ itemLabel(activity.love, $t('page.farm.activity.charityLove')) }}
          </div>
          <div class="text-16px font-semibold">{{ activity.loveBalance || '0' }}</div>
        </div>
      </div>
      <div class="flex items-center gap-10px rounded-8px bg-gray-50 px-12px py-10px dark:bg-gray-800">
        <div>
          <div class="text-12px text-gray-500">{{ $t('page.farm.activity.charityDonated') }}</div>
          <div class="text-16px font-semibold">{{ activity.donatedLove || '0' }}</div>
        </div>
      </div>
      <div class="flex items-center gap-10px rounded-8px bg-gray-50 px-12px py-10px dark:bg-gray-800">
        <img
          v-if="itemImage(activity.settlement?.reward)"
          :src="itemImage(activity.settlement?.reward)"
          class="h-36px w-36px object-contain"
        />
        <div>
          <div class="text-12px text-gray-500">{{ $t('page.farm.activity.charitySettlement') }}</div>
          <div class="text-16px font-semibold">
            {{ activity.settlement?.requiredLove || '0' }}
            <NTag
              size="tiny"
              :type="activity.settlement?.eligible ? 'success' : 'default'"
              :bordered="false"
              class="ml-4px"
            >
              {{
                activity.settlement?.eligible
                  ? $t('page.farm.activity.charityEligible')
                  : $t('page.farm.activity.charityNotYet')
              }}
            </NTag>
          </div>
        </div>
      </div>
      <div class="flex items-center gap-10px rounded-8px bg-gray-50 px-12px py-10px dark:bg-gray-800">
        <div>
          <div class="text-12px text-gray-500">{{ $t('page.farm.activity.charityGlobalDonated') }}</div>
          <div class="text-16px font-semibold">
            {{ activity.globalProgress?.donated || '0' }} / {{ activity.globalProgress?.target || '0' }}
          </div>
        </div>
      </div>
    </div>

    <div class="rounded-8px border border-gray-200 px-16px py-14px dark:border-gray-700">
      <div class="mb-10px flex items-center justify-between gap-8px">
        <div class="text-13px font-medium">{{ $t('page.farm.activity.charityGlobalProgress') }}</div>
        <div class="flex items-center gap-8px text-12px text-gray-500">
          <img
            v-if="itemImage(activity.globalProgress?.reward)"
            :src="itemImage(activity.globalProgress?.reward)"
            class="h-24px w-24px object-contain"
          />
          <span>{{ itemLabel(activity.globalProgress?.reward) }}</span>
          <span v-if="activity.globalProgress?.rewardTarget">
            {{ $t('page.farm.activity.charityRewardAt', { count: activity.globalProgress.rewardTarget }) }}
          </span>
        </div>
      </div>
      <NProgress
        type="line"
        :percentage="globalPercent"
        :height="10"
        :border-radius="5"
        :show-indicator="false"
        color="#f56c6c"
      />
      <div class="mt-6px text-right text-12px text-gray-500">{{ globalPercent.toFixed(1) }}%</div>
    </div>

    <div class="grid gap-12px lg:grid-cols-3">
      <div class="rounded-8px border border-gray-200 px-14px py-14px dark:border-gray-700">
        <div class="mb-8px text-13px font-medium">{{ $t('page.farm.activity.charitySeedTitle') }}</div>
        <div class="mb-10px flex items-center gap-8px text-12px text-gray-500">
          <img
            v-if="itemImage(activity.seedReward?.reward)"
            :src="itemImage(activity.seedReward?.reward)"
            class="h-24px w-24px object-contain"
          />
          <span>{{ itemLabel(activity.seedReward?.reward) }}</span>
          <span v-if="activity.seedReward?.reward?.count">×{{ activity.seedReward?.reward?.count }}</span>
        </div>
        <NButton type="primary" block :loading="pendingSeeds" :disabled="!canClaimSeeds" @click="emit('claimSeeds')">
          {{ seedsButtonLabel() }}
        </NButton>
      </div>

      <div class="rounded-8px border border-gray-200 px-14px py-14px dark:border-gray-700">
        <div class="mb-8px text-13px font-medium">{{ $t('page.farm.activity.charityDonateTitle') }}</div>
        <div class="mb-10px text-12px text-gray-500">
          {{ $t('page.farm.activity.charityDonateHint', { count: activity.loveBalance || '0' }) }}
        </div>
        <div v-if="confirmingDonate" class="mb-10px flex gap-8px">
          <NButton size="small" class="flex-1" :disabled="pendingDonate" @click="confirmingDonate = false">
            {{ $t('common.cancel') }}
          </NButton>
          <NButton
            size="small"
            type="error"
            class="flex-1"
            :loading="pendingDonate"
            :disabled="!canDonate"
            @click="donate()"
          >
            {{ $t('page.farm.activity.charityDonateConfirm') }}
          </NButton>
        </div>
        <NButton
          block
          :type="confirmingDonate ? 'error' : 'primary'"
          :loading="pendingDonate"
          :disabled="!canDonate"
          @click="donate()"
        >
          {{
            confirmingDonate ? $t('page.farm.activity.charityDonateConfirm') : $t('page.farm.activity.charityDonateAll')
          }}
        </NButton>
      </div>

      <div class="rounded-8px border border-gray-200 px-14px py-14px dark:border-gray-700">
        <div class="mb-8px text-13px font-medium">{{ $t('page.farm.activity.charityGiftTitle') }}</div>
        <div class="mb-10px flex items-center gap-8px text-12px text-gray-500">
          <img
            v-if="itemImage(activity.dailyGift?.reward)"
            :src="itemImage(activity.dailyGift?.reward)"
            class="h-24px w-24px object-contain"
          />
          <span>{{ itemLabel(activity.dailyGift?.reward) }}</span>
          <NTag v-if="activity.dailyGift?.claimed" size="tiny" type="success" :bordered="false">
            {{ $t('page.farm.activity.charityGiftClaimed') }}
          </NTag>
        </div>
        <NButton type="primary" block :loading="pendingGift" :disabled="!canClaimGift" @click="emit('claimDailyGift')">
          {{ giftButtonLabel() }}
        </NButton>
      </div>
    </div>

    <div class="rounded-8px border border-gray-200 px-16px py-14px dark:border-gray-700">
      <div class="mb-12px text-13px font-medium">{{ $t('page.farm.activity.charityProgressRewards') }}</div>
      <NEmpty v-if="!progressRewards.length" :description="$t('page.farm.activity.noRewards')" />
      <div v-else class="grid gap-10px sm:grid-cols-2 xl:grid-cols-3">
        <div
          v-for="(reward, index) in progressRewards"
          :key="String(reward.target || index)"
          class="flex items-center gap-10px rounded-8px border px-12px py-10px dark:border-gray-700"
          :class="reward.claimable || reward.claimed ? 'border-primary' : 'border-gray-200'"
        >
          <NTag size="tiny" :type="reward.reached ? 'success' : 'default'" :bordered="false">
            {{
              reward.reached
                ? $t('page.farm.activity.charityReached')
                : $t('page.farm.activity.charityTarget', { count: reward.target || '0' })
            }}
          </NTag>
          <div class="flex min-w-0 flex-1 items-center gap-6px text-12px">
            <img v-if="itemImage(reward.reward)" :src="itemImage(reward.reward)" class="h-24px w-24px object-contain" />
            <span class="min-w-0 flex-1 truncate">{{ itemLabel(reward.reward) }}</span>
            <span v-if="reward.reward?.count">×{{ reward.reward.count }}</span>
          </div>
          <NButton
            v-if="reward.claimSupported && reward.claimable"
            size="small"
            type="primary"
            :loading="pendingProgress"
            :disabled="pendingProgress"
            @click="emit('claimProgress', String(reward.target || ''))"
          >
            {{
              pendingProgress
                ? $t('page.farm.activity.charityProgressClaiming')
                : $t('page.farm.activity.charityClaimProgress')
            }}
          </NButton>
          <small v-else class="whitespace-nowrap text-12px text-gray-500">
            {{
              reward.claimed
                ? $t('page.farm.activity.charityProgressClaimed')
                : reward.reached
                  ? $t('page.farm.activity.charityProgressReached')
                  : ''
            }}
          </small>
        </div>
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
