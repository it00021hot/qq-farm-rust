<script setup lang="ts">
import { computed } from 'vue';
import { NCard, NEmpty, NTag } from 'naive-ui';
import { $t } from '@/locales';

defineOptions({ name: 'FarmPersonalDailyOverview' });

const props = defineProps<{
  dailyGifts: Api.Farm.DailyGiftsResponse | null;
}>();

const GIFT_ICONS: Record<string, string> = {
  task_claim: '✅',
  email_rewards: '📧',
  mall_free_gifts: '🛍️',
  daily_share: '📤',
  vip_daily_gift: '⭐',
  month_card_gift: '📅'
};

const gifts = computed(() => props.dailyGifts?.gifts || []);

function giftIcon(key: string) {
  return GIFT_ICONS[key] || '🎁';
}

function formatTime(timestamp: number) {
  if (!timestamp) return '';
  const d = new Date(timestamp);
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

function giftStatusText(gift: Api.Farm.DailyGiftCard) {
  if (gift.key === 'vip_daily_gift' && gift.hasGift === false) {
    return $t('page.farm.personal.giftNotOpened');
  }
  if (gift.key === 'month_card_gift' && gift.hasCard === false) {
    return $t('page.farm.personal.giftNotOpened');
  }
  if (gift.doneToday) return $t('page.farm.personal.giftDone');
  if (gift.enabled) return $t('page.farm.personal.giftWaiting');
  return $t('page.farm.personal.giftDisabled');
}

function giftStatusType(gift: Api.Farm.DailyGiftCard): NaiveUI.ThemeColor {
  if (
    (gift.key === 'vip_daily_gift' && gift.hasGift === false) ||
    (gift.key === 'month_card_gift' && gift.hasCard === false)
  ) {
    return 'default';
  }
  if (gift.doneToday) return 'success';
  if (gift.enabled) return 'info';
  return 'default';
}

function giftSubText(gift: Api.Farm.DailyGiftCard) {
  if (gift.key === 'vip_daily_gift' && gift.hasGift === false) {
    return '未开通QQ会员或无每日礼包';
  }
  if (gift.key === 'month_card_gift' && gift.hasCard === false) {
    return '未购买月卡或已过期';
  }
  const total = Number(gift.totalCount || 0);
  const current = Number(gift.completedCount || 0);
  if (total > 0) {
    return $t('page.farm.personal.growthProgress', { current, total });
  }
  const ts = Number(gift.lastAt || 0);
  if (ts && gift.doneToday) {
    return $t('page.farm.personal.nextRefresh', { time: formatTime(ts) });
  }
  return '';
}
</script>

<template>
  <NCard :title="$t('page.farm.personal.dailyGiftsTitle')" :bordered="false" size="small" class="card-wrapper">
    <div v-if="!dailyGifts" class="py-24px">
      <NEmpty :description="$t('page.farm.personal.giftNeedLogin')" />
    </div>
    <div v-else-if="!gifts.length" class="py-24px">
      <NEmpty :description="$t('page.farm.personal.giftEmpty')" />
    </div>
    <div v-else class="farm-gift-grid">
      <div v-for="gift in gifts" :key="gift.key" class="farm-gift-card">
        <div class="mb-8px flex-y-center gap-8px">
          <div class="farm-gift-icon">{{ giftIcon(gift.key) }}</div>
          <div class="min-w-0 flex-1">
            <div class="truncate text-14px font-medium">{{ gift.label }}</div>
            <NTag class="mt-4px" size="small" :type="giftStatusType(gift)" :bordered="false">
              {{ giftStatusText(gift) }}
            </NTag>
          </div>
        </div>
        <div v-if="giftSubText(gift)" class="text-12px opacity-55">
          {{ giftSubText(gift) }}
        </div>
      </div>
    </div>
  </NCard>
</template>

<style scoped>
.farm-gift-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: 12px;
}

.farm-gift-card {
  border: 1px solid rgba(0, 0, 0, 0.08);
  border-radius: 12px;
  padding: 12px;
}

.farm-gift-icon {
  width: 32px;
  height: 32px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.04);
  flex-shrink: 0;
}
</style>
