<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NButton, NCard, NEmpty, NGi, NGrid, NInput, NInputNumber, NModal, NSpace, NSpin, NTag } from 'naive-ui';
import { fetchGetFarmDiamond, fetchGetFarmGameMall, fetchPurchaseFarmGameMall } from '@/service/api';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { resolveCatalogImage } from '@/views/farm/game-config/shared';

defineOptions({ name: 'FarmGameMall' });

const farmAccountStore = useFarmAccountStore();
const loading = ref(false);
const catalog = ref<Api.Farm.MallCatalog | null>(null);
const diamond = ref<number | null>(null);
const keyword = ref('');
const purchaseVisible = ref(false);
const purchaseGoods = ref<Api.Farm.MallGoods | null>(null);
const purchaseCount = ref(1);
const purchasing = ref(false);
let requestSeq = 0;

const accountId = computed(() => farmAccountStore.currentAccountId);

const filteredGoods = computed(() => {
  const goods = catalog.value?.goods || [];
  const q = keyword.value.trim().toLowerCase();
  if (!q) return goods;
  return goods.filter(item => item.name.toLowerCase().includes(q) || String(item.id).includes(q));
});

async function loadMall() {
  if (!accountId.value) {
    catalog.value = null;
    diamond.value = null;
    return;
  }
  const seq = ++requestSeq;
  loading.value = true;
  try {
    const [mallRes, diamondRes] = await Promise.all([
      fetchGetFarmGameMall({ accountId: accountId.value, slotType: 1, subSlotType: 0 }),
      fetchGetFarmDiamond(accountId.value)
    ]);
    if (seq !== requestSeq) return;
    if (!mallRes.error && mallRes.data) catalog.value = mallRes.data;
    if (!diamondRes.error && diamondRes.data) diamond.value = diamondRes.data.diamond;
  } finally {
    if (seq === requestSeq) loading.value = false;
  }
}

function openPurchase(goods: Api.Farm.MallGoods) {
  if (!goods.purchasable) {
    window.$message?.warning('商品当前不可购买');
    return;
  }
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
      count: purchaseCount.value
    });
    if (error) return;
    window.$message?.success('购买成功');
    purchaseVisible.value = false;
    if (data?.catalog) catalog.value = data.catalog;
    const diamondRes = await fetchGetFarmDiamond(accountId.value);
    if (!diamondRes.error && diamondRes.data) diamond.value = diamondRes.data.diamond;
  } finally {
    purchasing.value = false;
  }
}

function priceText(goods: Api.Farm.MallGoods) {
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
  <div class="min-h-500px flex-col-stretch gap-16px overflow-hidden lt-sm:overflow-auto">
    <NCard :bordered="false" class="card-wrapper" title="游戏商城">
      <template #header-extra>
        <NSpace align="center">
          <NTag v-if="diamond !== null" type="warning">钻石 {{ diamond }}</NTag>
          <NButton size="small" :loading="loading" @click="loadMall">刷新</NButton>
        </NSpace>
      </template>

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
                <div>{{ priceText(goods) }}</div>
                <div v-if="goods.discountText" class="text-error">{{ goods.discountText }}</div>
                <div v-if="goods.limit">限购 {{ goods.limit.bought }}/{{ goods.limit.max }}</div>
              </div>
              <NButton
                class="mt-auto"
                size="small"
                type="primary"
                block
                :disabled="!goods.purchasable"
                @click="openPurchase(goods)"
              >
                购买
              </NButton>
            </div>
          </NGi>
        </NGrid>
      </NSpin>
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
        <div class="text-12px text-#888">{{ priceText(purchaseGoods) }}</div>
        <NInputNumber v-model:value="purchaseCount" :min="1" :max="99" />
      </div>
    </NModal>
  </div>
</template>
