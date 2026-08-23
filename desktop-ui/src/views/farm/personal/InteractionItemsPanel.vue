<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import {
  NButton,
  NCard,
  NEmpty,
  NInput,
  NInputNumber,
  NList,
  NListItem,
  NSelect,
  NSpace,
  NTag
} from 'naive-ui';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import {
  fetchGetFarmInteractionItems,
  fetchGetFriendInteractionItems,
  fetchUseFarmInteractionItems,
  fetchUseFriendInteractionItems
} from '@/service/api';
import { $t } from '@/locales';

defineOptions({ name: 'InteractionItemsPanel' });

const props = defineProps<{
  /** self = 对自己农场使用；friend = 对好友农场使用 */
  mode: 'self' | 'friend';
  /** friend 模式下的好友 GID（可选，可手填） */
  friendGid?: number | null;
}>();

const emit = defineEmits<{ refresh: [] }>();

const farmAccountStore = useFarmAccountStore();
const loading = ref(false);
const using = ref(false);
const items = ref<any[]>([]);
const message = ref('');
const selectedItemId = ref<number | null>(null);
const landIdsText = ref('');
const friendGidInput = ref<number | null>(props.friendGid ?? null);

watch(
  () => props.friendGid,
  v => {
    if (v) friendGidInput.value = v;
  }
);

const selectedGid = computed(() => Number(friendGidInput.value ?? 0));

async function load() {
  if (!farmAccountStore.currentAccountId) return;
  loading.value = true;
  try {
    const fetcher =
      props.mode === 'self' ? fetchGetFarmInteractionItems : fetchGetFriendInteractionItems;
    const { data, error } = await fetcher(farmAccountStore.currentAccountId);
    if (!error) {
      items.value = data?.items ?? [];
      message.value = data?.message ?? '';
      if (!selectedItemId.value && items.value.length) {
        selectedItemId.value = Number(items.value[0].itemId);
      }
    }
  } finally {
    loading.value = false;
  }
}

const itemOptions = computed(() =>
  items.value.map((it: any) => ({
    label: `${it.name} x${it.count}`,
    value: Number(it.itemId)
  }))
);

function parseLandIds(): number[] {
  return landIdsText.value
    .split(/[,，\s]+/)
    .map(s => s.trim())
    .filter(Boolean)
    .map(Number)
    .filter(n => Number.isFinite(n) && n > 0);
}

async function useBatch() {
  if (!farmAccountStore.currentAccountId || !selectedItemId.value) return;
  const landIds = parseLandIds();
  if (!landIds.length) {
    window.$message?.warning($t('page.farm.personal.interaction.landIdsPlaceholder'));
    return;
  }
  if (props.mode === 'friend' && selectedGid.value <= 0) {
    window.$message?.warning($t('page.farm.friends.gidPlaceholder'));
    return;
  }
  using.value = true;
  try {
    const { data, error } =
      props.mode === 'self'
        ? await fetchUseFarmInteractionItems(farmAccountStore.currentAccountId, selectedItemId.value, landIds)
        : await fetchUseFriendInteractionItems(
            farmAccountStore.currentAccountId,
            selectedGid.value,
            selectedItemId.value,
            landIds
          );
    if (!error) {
      window.$message?.success(data?.message ?? $t('page.farm.personal.interaction.useSuccess'));
      await load();
      emit('refresh');
    }
  } finally {
    using.value = false;
  }
}

defineExpose({ refresh: load });

void load();
</script>

<template>
  <NCard size="small" :title="$t('page.farm.personal.interaction.title')">
    <template #header-extra>
      <NButton size="small" :loading="loading" @click="load">{{ $t('page.farm.personal.pet.refresh') }}</NButton>
    </template>
    <NEmpty v-if="!items.length" :description="message || $t('page.farm.personal.interaction.empty')" />
    <div v-else class="flex flex-col gap-12px">
      <NList bordered size="small">
        <NListItem v-for="item in items" :key="item.itemId">
          <div class="flex items-center justify-between gap-12px">
            <div class="flex items-center gap-8px">
              <span class="font-medium">{{ item.name }}</span>
              <NTag size="small" :bordered="false">x{{ item.count }}</NTag>
              <NTag v-if="Number(item.saleConditionSatisfiedCount) > 0" size="small" type="warning" :bordered="false">
                {{ $t('page.farm.personal.interaction.sellableCount') }}: {{ item.saleConditionSatisfiedCount }}
              </NTag>
            </div>
            <span class="max-w-360px truncate text-12px text-gray-400">{{ item.description }}</span>
          </div>
        </NListItem>
      </NList>

      <NSpace align="center">
        <NSelect
          v-model:value="selectedItemId"
          :options="itemOptions"
          size="small"
          class="w-180px"
        />
        <NInputNumber
          v-if="mode === 'friend'"
          v-model:value="friendGidInput"
          size="small"
          :show-button="false"
          class="w-160px"
          :placeholder="$t('page.farm.friends.gidPlaceholder')"
        />
        <NInput
          v-model:value="landIdsText"
          size="small"
          class="w-220px"
          :placeholder="$t('page.farm.personal.interaction.landIdsPlaceholder')"
        />
        <NButton type="primary" size="small" :loading="using" @click="useBatch">
          {{ $t('page.farm.personal.interaction.use') }}
        </NButton>
      </NSpace>
    </div>
  </NCard>
</template>
