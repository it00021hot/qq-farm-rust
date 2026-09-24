<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { useRoute } from 'vue-router';
import {
  NButton,
  NCard,
  NEmpty,
  NGi,
  NGrid,
  NInput,
  NInputNumber,
  NModal,
  NSpace,
  NSpin,
  NTabPane,
  NTabs,
  NTag
} from 'naive-ui';
import { fetchGetFarmDiamond, fetchGetFarmGameMall, fetchPurchaseFarmGameMall } from '@/service/api';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { resolveCatalogImage } from '@/views/farm/game-config/shared';
import MysteryShopPanel from '@/views/farm/mystery-shop/panel.vue';

defineOptions({ name: 'FarmGameMall' });

type MallTab = 'normal' | 'svip' | 'mystery';

const route = useRoute();
const farmAccountStore = useFarmAccountStore();
const loading = ref(false);
const activeTab = ref<MallTab>(initialTab());

function initialTab(): MallTab {
  const tab = String(route.query.tab || '').toLowerCase();
  return tab === 'svip' || tab === 'mystery' ? (tab as MallTab) : 'normal';
}
const catalogs = ref<{ normal: Api.Farm.MallCatalog | null; svip: Api.Farm.MallCatalog | null }>({
  normal: null,
  svip: null
});
const diamond = ref<number | null>(null);
const keyword = ref('');
const purchaseVisible = ref(false);
const purchaseGoods = ref<Api.Farm.MallGoods | null>(null);
const purchaseCount = ref(1);
const purchasing = ref(false);
let requestSeq = 0;

const accountId = computed(() => farmAccountStore.currentAccountId);
const activeSlot = computed<'normal' | 'svip'>(() => (activeTab.value === 'svip' ? 'svip' : 'normal'));
const catalog = computed(() => catalogs.value[activeSlot.value]);
const membership = computed(() => catalog.value?.membership ?? null);

const filteredGoods = computed(() => {
  const goods = catalog.value?.goods || [];
  const q = keyword.value.trim().toLowerCase();
  if (!q) return goods;
  return goods.filter(item => item.name.toLowerCase().includes(q) || String(item.id).includes(q));
});

async function loadMall() {
  if (!accountId.value) {
    catalogs.value = { normal: null, svip: null };
    diamond.value = null;
    return;
  }
  const seq = ++requestSeq;
  loading.value = true;
  try {
    const slotType = activeSlot.value === 'svip' ? 4 : 1;
    const [mallRes, diamondRes] = await Promise.all([
      fetchGetFarmGameMall({ accountId: accountId.value, slotType, subSlotType: 0 }),
      diamond.value === null
        ? fetchGetFarmDiamond(accountId.value)
        : Promise.resolve({ error: null, data: null })
    ]);
    if (seq !== requestSeq) return;
    if (!mallRes.error && mallRes.data) catalogs.value[activeSlot.value] = mallRes.data;
    if (diamondRes && !diamondRes.error && diamondRes.data) diamond.value = diamondRes.data.diamond;
  } finally {
    if (seq === requestSeq) loading.value = false;
  }
}

function onTabChange(tab: string | number) {
  activeTab.value = (tab === 'svip' || tab === 'mystery' ? tab : 'normal') as MallTab;
  if (activeTab.value !== 'mystery' && !catalogs.value[activeSlot.value]) void loadMall();
}

/** 按钮文案 + 状态（对齐 bot getMallPurchaseAction） */
function purchaseAction(goods: Api.Farm.MallGoods): { label: string; disabled: boolean; type: 'primary' | 'default' } {
  switch (goods.purchaseStatus) {
    case 'owned':
      return { label: '已拥有', disabled: true, type: 'default' };
    case 'sold_out':
      return { label: goods.isFree ? '奖励已领取' : '已售罄', disabled: true, type: 'default' };
    case 'ad_required':
      return { label: '游戏内广告领取', disabled: true, type: 'default' };
    case 'share_required':
      return { label: '游戏内分享后领取', disabled: true, type: 'default' };
    case 'svip_required':
      return { label: '需要 SVIP', disabled: true, type: 'default' };
    case 'unavailable':
      return { label: '暂不可购买', disabled: true, type: 'default' };
    default:
      return { label: '购买', disabled: false, type: 'primary' };
  }
}

function openPurchase(goods: Api.Farm.MallGoods) {
  const action = purchaseAction(goods);
  if (action.disabled || !goods.purchasable) return;
  purchaseGoods.value = goods;
  purchaseCount.value = 1;
  purchaseVisible.value = true;
}

async function confirmPurchase() {
  if (!accountId.value || !purchaseGoods.value) return;
  purchasing.value = true;
  try {
    const { data, error } = await fetchPurchaseFarmGameMall({
      accountId: accountId.value,
      goodsId: purchaseGoods.value.id,
      count: purchaseCount.value,
      slotType: activeSlot.value === 'svip' ? 4 : 1,
      expectedPrice: { id: purchaseGoods.value.price.id, count: purchaseGoods.value.price.count }
    });
    if (error) return;
    window.$message?.success('购买成功');
    purchaseVisible.value = false;
    if (data?.catalog) catalogs.value[activeSlot.value] = data.catalog;
    else await loadMall();
    const diamondRes = await fetchGetFarmDiamond(accountId.value);
    if (!diamondRes.error && diamondRes.data) diamond.value = diamondRes.data.diamond;
  } finally {
    purchasing.value = false;
  }
}

function priceText(goods: Pick<Api.Farm.MallGoods, 'isFree' | 'price'>) {
  if (goods.isFree) return '免费';
  return `${goods.price.count} ${goods.price.name || `#${goods.price.id}`}`;
}

watch(
  accountId,
  () => {
    void loadMall();
  },
  { immediate: true }
);
</script>

<template>
  <div class="h-full min-h-500px flex-col-stretch gap-16px overflow-auto">
    <NCard :bordered="false" class="card-wrapper" title="游戏商城">
      <template #header-extra>
        <NSpace align="center">
          <NTag v-if="membership" :type="membership.isSvip ? 'warning' : 'default'">
            {{ membership.isSvip ? `SVIP · 剩余 ${membership.remainingDays} 天` : '非 SVIP 会员' }}
          </NTag>
          <NTag v-if="diamond !== null" type="warning">钻石 {{ diamond }}</NTag>
          <NButton size="small" :loading="loading" @click="loadMall">刷新</NButton>
        </NSpace>
      </template>

      <NTabs :value="activeTab" type="segment" size="small" class="mb-12px max-w-520px" @update:value="onTabChange">
        <!-- 自闭合 NTabPane 在 naive-ui 2.45 的 label 会渲染成空（normalizeSlots 把
             undefined children 包成空插槽），label 必须走 #tab 具名插槽 -->
        <NTabPane name="normal">
          <template #tab>普通商城</template>
        </NTabPane>
        <NTabPane name="svip">
          <template #tab>SVIP 商城</template>
        </NTabPane>
        <NTabPane name="mystery">
          <template #tab>神秘商人</template>
        </NTabPane>
      </NTabs>

      <MysteryShopPanel v-if="activeTab === 'mystery'" />

      <template v-else>
        <NSpace class="mb-16px" align="center">
          <NInput v-model:value="keyword" clearable placeholder="搜索商品名称 / ID" class="w-260px" />
          <span v-if="catalog" class="text-12px text-#888">
            {{ catalog.goods?.length || 0 }} 件商品
            <template v-if="catalog.refreshCountdown > 0">· {{ catalog.refreshCountdown }}s 后刷新</template>
          </span>
        </NSpace>

        <NSpin :show="loading">
          <NEmpty v-if="!accountId" description="请先选择农场账号" />
          <NEmpty v-else-if="!filteredGoods.length" description="暂无商品" />
          <NGrid v-else cols="2 s:3 m:4 l:5" responsive="screen" :x-gap="12" :y-gap="12" item-responsive>
            <NGi v-for="goods in filteredGoods" :key="goods.id" class="h-full">
              <div class="h-full min-h-220px flex flex-col gap-8px rounded-8px bg-#fafafa p-12px dark:bg-#222">
                <div class="h-72px flex shrink-0 items-center justify-center">
                  <img
                    v-if="resolveCatalogImage(goods.rewards?.[0]?.image)"
                    :src="resolveCatalogImage(goods.rewards?.[0]?.image)"
                    class="max-h-64px max-w-64px object-contain"
                    alt=""
                  />
                  <span v-else class="text-28px">🛒</span>
                </div>
                <div class="text-14px font-medium line-clamp-2">{{ goods.name || `商品 #${goods.id}` }}</div>
                <div class="min-h-40px flex flex-col gap-4px text-12px text-#888">
                  <div>
                    <span
                      v-if="goods.originalPrice != null && !goods.isFree"
                      class="mr-6px text-#aaa line-through"
                    >
                      {{ goods.originalPrice }}
                    </span>
                    <span class="text-#18a058">{{ priceText(goods) }}</span>
                  </div>
                  <div v-if="goods.discountText" class="text-error">{{ goods.discountText }}</div>
                  <div v-if="goods.limit">限购 {{ goods.limit.bought }}/{{ goods.limit.max }}</div>
                  <div v-if="goods.unavailableReason && goods.purchaseStatus !== 'available'" class="text-#d03050">
                    {{ goods.unavailableReason }}
                  </div>
                </div>
                <NButton
                  class="mt-auto"
                  size="small"
                  :type="purchaseAction(goods).type"
                  :disabled="purchaseAction(goods).disabled"
                  block
                  @click="openPurchase(goods)"
                >
                  {{ purchaseAction(goods).label }}
                </NButton>
              </div>
            </NGi>
          </NGrid>
        </NSpin>
      </template>
    </NCard>

    <NModal
      v-model:show="purchaseVisible"
      preset="dialog"
      title="确认购买"
      positive-text="购买"
      negative-text="取消"
      :loading="purchasing"
      @positive-click="confirmPurchase"
    >
      <div v-if="purchaseGoods" class="flex-col gap-12px">
        <div>{{ purchaseGoods.name }}</div>
        <div class="text-12px text-#888">
          <span
            v-if="purchaseGoods.originalPrice != null && !purchaseGoods.isFree"
            class="mr-6px line-through"
          >
            {{ purchaseGoods.originalPrice }}
          </span>
          {{ priceText(purchaseGoods) }}
        </div>
        <NInputNumber v-model:value="purchaseCount" :min="1" :max="99" />
      </div>
    </NModal>
  </div>
</template>
