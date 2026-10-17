<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import {
  NButton,
  NCard,
  NInput,
  NInputNumber,
  NModal,
  NSpace,
  NSpin,
  NTag,
  useMessage
} from 'naive-ui';
import { fetchMilkTeaOperate, fetchMilkTeaSnapshot } from '@/service/api';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { resolveCatalogImage } from '@/views/farm/game-config/shared';
import { $t } from '@/locales';

defineOptions({ name: 'FarmMilkTea' });

type MilkTeaTab = 'shop' | 'workshop' | 'hireList' | 'work' | 'exchange' | 'fund' | 'rank' | 'logs';

const MILK_ACTIVITY_ID = '2026110101';

const farmAccountStore = useFarmAccountStore();
const message = useMessage();

const tab = ref<MilkTeaTab>('shop');
const data = ref<any>(null);
const busy = ref(false);
const error = ref('');
const renameOpen = ref(false);
const renameName = ref('');
const createName = ref('');
const boostCount = ref(1);
const craftSelection = ref<string[]>([]);
const hireDuration = ref(1);
const exchangeCount = ref<Record<string, number>>({});
const rankPage = ref(1);
const logsDay = ref(0);
let sequence = 0;

const connected = computed(() => farmAccountStore.currentAccount?.runStatus === 1);
const summary = computed(() => data.value?.summary ?? null);
const shop = computed(() => data.value?.shop ?? null);
const welcome = computed<any[]>(() => data.value?.recommendedNames ?? []);
const opened = computed(() => Number(summary.value?.shopOpenAt || 0) > 0);

const tabs: { key: MilkTeaTab; label: string }[] = [
  { key: 'shop', label: $t('page.farm.milkTea.tabShop') },
  { key: 'workshop', label: $t('page.farm.milkTea.tabWorkshop') },
  { key: 'hireList', label: $t('page.farm.milkTea.tabHire') },
  { key: 'work', label: $t('page.farm.milkTea.tabWork') },
  { key: 'exchange', label: $t('page.farm.milkTea.tabExchange') },
  { key: 'fund', label: $t('page.farm.milkTea.tabFund') },
  { key: 'rank', label: $t('page.farm.milkTea.tabRank') },
  { key: 'logs', label: $t('page.farm.milkTea.tabLogs') }
];

function itemImage(value: any): string {
  return resolveCatalogImage(value?.image);
}

function rewardsText(rewards: any[] | undefined): string {
  return (rewards ?? [])
    .map((reward: any) => `${reward.name} ×${reward.count}`)
    .join('、');
}

function imageOfReward(reward: any): string {
  return resolveCatalogImage(reward?.image);
}

async function request(action = '', input: Record<string, unknown> = {}) {
  const accountId = farmAccountStore.currentAccountId;
  if (!accountId || !connected.value || busy.value) return;
  const revision = ++sequence;
  busy.value = true;
  error.value = '';
  try {
    const response = action
      ? await fetchMilkTeaOperate(accountId, action, input)
      : await fetchMilkTeaSnapshot(accountId, tab.value, tab.value === 'hireList' ? { durationId: hireDuration.value } : input);
    if (revision !== sequence) return;
    if (response.error || !response.data) {
      throw new Error(response.error?.message || $t('page.farm.milkTea.requestFailed'));
    }
    const result: any = response.data;
    if (action) {
      const text = rewardsText(result.rewards) || $t('page.farm.milkTea.actionSuccess');
      if (result.refreshRequired) {
        message.warning(`${text}。${$t('page.farm.milkTea.refreshRequired')}`);
      } else {
        message.success(text);
      }
      data.value = result.activity ?? null;
      if (!data.value) await reload();
    } else {
      data.value = result;
      if (Number(data.value?.id || 0) !== Number(MILK_ACTIVITY_ID)) {
        throw new Error($t('page.farm.milkTea.requestFailed'));
      }
      rankPage.value = 1;
      if (tab.value === 'logs' && !logsDay.value) logsDay.value = Number(result.logs?.day || 0);
    }
  } catch (cause) {
    if (revision === sequence) {
      error.value = (cause as Error)?.message || $t('page.farm.milkTea.requestFailed');
      if (action) message.error(error.value);
    }
  } finally {
    if (revision === sequence) busy.value = false;
  }
}

async function reload() {
  await request();
}

function switchTab(key: MilkTeaTab) {
  if (tab.value === key && data.value) return;
  tab.value = key;
  craftSelection.value = [];
  data.value = null;
  void reload();
}

async function createShop(name?: string) {
  const value = (name ?? createName.value).trim();
  if (!value) return;
  await request('create', { name: value });
  createName.value = '';
}

async function renameShop() {
  const value = renameName.value.trim();
  if (!value) return;
  await request('rename', { name: value });
  renameOpen.value = false;
}

async function claimCashier() {
  await request('claim', {});
}

async function boost() {
  if (!shop.value) return;
  await request('boost', {
    count: boostCount.value,
    expectedCosts: [{ id: shop.value.salesItemId, count: 1 }]
  });
}

async function upgrade() {
  await request('upgrade', { expectedLevel: Number(summary.value?.level || 0) });
}

async function toggleHireDisabled() {
  await request('hireSetting', { disabled: !summary.value?.hireDisabled });
}

async function craft() {
  const fruits = (data.value?.fruits ?? []).filter((fruit: any) =>
    craftSelection.value.includes(String(fruit.uid))
  );
  if (!fruits.length) {
    message.info($t('page.farm.milkTea.craftEmpty'));
    return;
  }
  await request('craft', {
    fruits: fruits.map((fruit: any) => ({ uid: fruit.uid, count: Number(fruit.count) }))
  });
  craftSelection.value = [];
}

async function hire(candidate: any) {
  const gid = candidate?.friend?.gid;
  if (gid == null || !data.value?.hire) return;
  await request('hire', {
    targetGid: String(gid),
    expectedCosts: [{ id: 1041, count: Number(data.value.hire.wageTotal || 0) }]
  });
}

async function claimWages() {
  await request('wages', {});
}

async function interact(kind: number) {
  const shift = data.value?.work?.myShift;
  if (!shift) return;
  await request('interact', {
    interactionType: kind,
    expectedShiftId: String(shift.shiftId)
  });
}

async function exchange(goods: any) {
  const count = Math.max(1, Math.floor(Number(exchangeCount.value[goods.id]) || 1));
  await request('exchange', {
    goodsId: String(goods.id),
    count,
    expectedCosts: (goods.costs ?? []).map((cost: any) => ({ id: cost.id, count: cost.count }))
  });
}

async function unlockFund() {
  await request('unlock', {
    expectedCosts: (data.value?.fund?.costs ?? []).map((cost: any) => ({ id: cost.id, count: cost.count }))
  });
}

async function claimFund() {
  await request('fundClaim', {});
}

async function loadRank(page: number) {
  rankPage.value = page;
  await request('rank', { pageNumber: page });
}

async function loadLogs(day: number) {
  if (day <= 0) return;
  logsDay.value = day;
  await request('logs', { day });
}

async function selectDuration(durationId: number) {
  hireDuration.value = durationId;
  await reload();
}

function durationText(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  return hours > 0 ? `${hours}h ${minutes}m` : `${minutes}m`;
}

function shiftRemaining(shift: any): string {
  const now = Number(data.value?.serverTime || 0);
  return durationText(Math.max(0, Number(shift.endAt || 0) - now));
}

function candidateState(candidate: any): string {
  const now = Number(data.value?.serverTime || 0);
  if (Number(candidate.state || 0) !== 0 || Number(candidate.rehireAvailableAt || 0) > now) {
    return $t('page.farm.milkTea.hireBusy');
  }
  return $t('page.farm.milkTea.hireAvailable');
}

watch(
  () => [farmAccountStore.currentAccountId, connected.value] as const,
  () => {
    sequence++;
    data.value = null;
    error.value = '';
    void reload();
  },
  { immediate: true, flush: 'sync' }
);

onMounted(() => {
  void reload();
});

defineExpose({ refresh: reload });
</script>

<template>
  <div class="mx-auto max-w-1100px">
    <NCard :title="$t('page.farm.milkTea.title')" size="small" :bordered="false" class="card-wrapper">
      <template #header-extra>
        <NSpace size="small">
          <NTag v-if="summary" size="small" :type="data?.active ? 'success' : 'error'">
            {{
              data?.settling
                ? $t('page.farm.milkTea.settling')
                : data?.active
                  ? $t('page.farm.milkTea.active')
                  : $t('page.farm.milkTea.inactive')
            }}
          </NTag>
          <NButton size="small" :loading="busy" :disabled="!farmAccountStore.currentAccountId || !connected" @click="reload">
            {{ $t('page.farm.milkTea.refresh') }}
          </NButton>
        </NSpace>
      </template>

      <NSpin :show="busy">
        <div v-if="!farmAccountStore.currentAccountId || !connected" class="py-32px text-center text-gray-500">
          <div class="text-28px">🧋</div>
          <strong>
            {{
              farmAccountStore.currentAccountId
                ? $t('page.farm.milkTea.startAccount')
                : $t('page.farm.milkTea.selectAccount')
            }}
          </strong>
        </div>
        <template v-else-if="data">
          <!-- 未开业：创建店铺 -->
          <div v-if="!opened" class="rounded-8px bg-gray-50 p-16px dark:bg-gray-800">
            <strong>{{ $t('page.farm.milkTea.createTitle') }}</strong>
            <div class="mt-8px flex flex-wrap items-center gap-8px">
              <NInput v-model:value="createName" class="max-w-220px" :placeholder="$t('page.farm.milkTea.shopName')" />
              <NButton type="primary" :loading="busy" @click="createShop()">
                {{ $t('page.farm.milkTea.createOpen') }}
              </NButton>
            </div>
            <div class="mt-8px flex flex-wrap gap-6px">
              <NButton v-for="name in welcome" :key="name" size="tiny" secondary @click="createShop(name)">
                {{ name }}
              </NButton>
            </div>
          </div>

          <template v-else>
            <nav class="mb-10px flex flex-wrap gap-8px">
              <NButton
                v-for="entry in tabs"
                :key="entry.key"
                size="small"
                :type="tab === entry.key ? 'primary' : 'default'"
                :disabled="busy"
                @click="switchTab(entry.key)"
              >
                {{ entry.label }}
              </NButton>
            </nav>

            <!-- 我的店铺 -->
            <template v-if="tab === 'shop'">
              <div class="mb-10px rounded-8px bg-gray-50 p-12px dark:bg-gray-800">
                <div class="flex flex-wrap items-center gap-8px">
                  <strong>{{ summary?.shopName }}</strong>
                  <NTag size="small" type="info">Lv.{{ summary?.level }}</NTag>
                  <NTag v-if="summary?.hireDisabled" size="small">{{ $t('page.farm.milkTea.hireDisabled') }}</NTag>
                  <span class="ml-auto text-12px text-gray-500">
                    {{ $t('page.farm.milkTea.revenue') }}：{{ summary?.totalRevenue }} /
                    {{ shop?.nextLevelRevenue || '—' }}
                  </span>
                </div>
                <div class="mt-8px flex flex-wrap gap-8px">
                  <NButton size="small" :disabled="busy" @click="renameOpen = true">
                    {{ $t('page.farm.milkTea.rename') }}
                  </NButton>
                  <NButton size="small" type="primary" :disabled="busy || !summary?.canUpgrade" @click="upgrade">
                    {{ $t('page.farm.milkTea.upgrade') }}
                  </NButton>
                  <NButton size="small" :disabled="busy" @click="toggleHireDisabled">
                    {{
                      summary?.hireDisabled
                        ? $t('page.farm.milkTea.enableHire')
                        : $t('page.farm.milkTea.disableHire')
                    }}
                  </NButton>
                </div>
              </div>

              <div class="mb-10px rounded-8px border border-gray-200 p-12px dark:border-gray-700">
                <div class="flex flex-wrap items-center gap-8px">
                  <span>{{ $t('page.farm.milkTea.cashier') }}</span>
                  <span class="text-12px text-gray-500">
                    {{ shop?.cashierElapsedSec || 0 }}s / {{ shop?.cashierCapacitySec || 0 }}s ·
                    {{ shop?.effectiveCupsPerHour || 0 }}{{ $t('page.farm.milkTea.cupsPerHour') }}
                  </span>
                  <div class="ml-auto flex items-center gap-6px">
                    <template v-for="reward in shop?.pendingRewards ?? []" :key="reward.id">
                      <span class="flex items-center gap-4px text-12px">
                        <img v-if="itemImage(reward)" :src="itemImage(reward)" class="h-18px w-18px object-contain" loading="lazy" />
                        {{ reward.name }} ×{{ reward.count }}
                      </span>
                    </template>
                    <NButton size="small" type="primary" :disabled="busy" @click="claimCashier">
                      {{ $t('page.farm.milkTea.claimCashier') }}
                    </NButton>
                  </div>
                </div>
                <div class="mt-8px flex flex-wrap items-center gap-8px">
                  <span>{{ $t('page.farm.milkTea.boost') }}</span>
                  <span class="text-12px text-gray-500">
                    {{ rewardsText(shop?.boostPreviewRewards) || '—' }}
                  </span>
                  <NInputNumber v-model:value="boostCount" size="small" class="w-100px" :min="1" :max="9999" />
                  <NButton size="small" :disabled="busy || shop?.isSettling" @click="boost">
                    {{ $t('page.farm.milkTea.boostAction') }}
                  </NButton>
                </div>
                <div v-if="(shop?.slots ?? []).length" class="mt-8px flex flex-wrap gap-8px text-12px">
                  <span
                    v-for="slot in shop.slots"
                    :key="slot.slotIndex"
                    class="rounded-4px bg-gray-100 px-6px py-2px dark:bg-gray-800"
                  >
                    #{{ slot.slotIndex }} {{ slot.employee?.name || (slot.isRobot ? $t('page.farm.milkTea.robot') : '') }}
                    {{ slot.talentName }}
                  </span>
                </div>
              </div>

              <div class="rounded-8px border border-gray-200 p-12px dark:border-gray-700">
                <strong class="text-13px">{{ $t('page.farm.milkTea.recipes') }}</strong>
                <div class="mt-8px grid grid-cols-2 gap-8px sm:grid-cols-3 lg:grid-cols-4">
                  <div
                    v-for="recipe in data.recipes ?? []"
                    :key="recipe.recipeId"
                    class="rounded-8px border border-gray-200 p-8px text-12px dark:border-gray-700"
                    :class="{ 'opacity-50': !recipe.unlocked }"
                  >
                    <img v-if="itemImage(recipe)" :src="itemImage(recipe)" class="mx-auto h-36px w-36px object-contain" loading="lazy" />
                    <div class="mt-4px text-center font-medium">{{ recipe.recipeName }}</div>
                    <div class="text-center text-gray-500">
                      {{ recipe.unlocked ? `${recipe.stockCups}${$t('page.farm.milkTea.cups')}` : $t('page.farm.milkTea.locked') }}
                    </div>
                    <div class="mt-2px text-center text-gray-400">
                      {{ recipe.tokenPrice }} 💧 / {{ recipe.coinPrice }} 金
                    </div>
                  </div>
                </div>
              </div>
            </template>

            <!-- 工坊 -->
            <template v-else-if="tab === 'workshop'">
              <div class="rounded-8px border border-gray-200 p-12px dark:border-gray-700">
                <strong class="text-13px">{{ $t('page.farm.milkTea.craftTitle') }}</strong>
                <div class="mt-8px flex flex-col gap-6px">
                  <label
                    v-for="fruit in data.fruits ?? []"
                    :key="fruit.uid"
                    class="flex items-center gap-8px rounded-6px border border-gray-200 px-10px py-6px text-13px dark:border-gray-700"
                    :class="{ 'opacity-50': fruit.locked }"
                  >
                    <input
                      v-model="craftSelection"
                      type="checkbox"
                      :value="String(fruit.uid)"
                      :disabled="fruit.locked || busy"
                    />
                    <img v-if="itemImage(fruit)" :src="itemImage(fruit)" class="h-20px w-20px object-contain" loading="lazy" />
                    <span>{{ fruit.name }}</span>
                    <span class="ml-auto text-gray-500">×{{ fruit.count }}</span>
                  </label>
                  <span v-if="!(data.fruits ?? []).length" class="text-12px text-gray-500">
                    {{ $t('page.farm.milkTea.craftEmpty') }}
                  </span>
                </div>
                <div class="mt-8px">
                  <NButton type="primary" size="small" :disabled="busy || !craftSelection.length" @click="craft">
                    {{ $t('page.farm.milkTea.craftAction') }}
                  </NButton>
                </div>
              </div>
            </template>

            <!-- 雇佣 -->
            <template v-else-if="tab === 'hireList'">
              <div class="mb-8px flex flex-wrap items-center gap-8px">
                <NButton
                  v-for="duration in data.durations ?? []"
                  :key="duration.durationId"
                  size="small"
                  :type="hireDuration === duration.durationId ? 'primary' : 'default'"
                  :disabled="busy"
                  @click="selectDuration(duration.durationId)"
                >
                  {{ durationText(duration.durationSec) }} · {{ duration.cost }}💧
                </NButton>
                <span class="text-12px text-gray-500">
                  {{ $t('page.farm.milkTea.wageTotal') }}：{{ data.hire?.wageTotal }}💧
                </span>
              </div>
              <div class="flex flex-col gap-6px">
                <div
                  v-for="candidate in [...(data.hire?.friends ?? []), ...(data.hire?.robots ?? [])]"
                  :key="candidate.friend?.gid"
                  class="flex items-center gap-8px rounded-6px border border-gray-200 px-10px py-8px text-13px dark:border-gray-700"
                >
                  <img v-if="candidate.friend?.avatar" :src="candidate.friend.avatar" class="h-24px w-24px rounded-full object-cover" loading="lazy" />
                  <span>{{ candidate.friend?.name || candidate.friend?.gid }}</span>
                  <span class="text-12px text-gray-500">Lv.{{ candidate.friend?.level }}</span>
                  <NTag size="small" :type="candidateState(candidate) === $t('page.farm.milkTea.hireAvailable') ? 'success' : 'default'">
                    {{ candidateState(candidate) }}
                  </NTag>
                  <NButton
                    class="ml-auto"
                    size="tiny"
                    type="primary"
                    :disabled="busy || candidateState(candidate) !== $t('page.farm.milkTea.hireAvailable')"
                    @click="hire(candidate)"
                  >
                    {{ $t('page.farm.milkTea.hireAction') }}
                  </NButton>
                </div>
                <span v-if="!(data.hire?.friends ?? []).length && !(data.hire?.robots ?? []).length" class="text-12px text-gray-500">
                  {{ $t('page.farm.milkTea.hireEmpty') }}
                </span>
              </div>
            </template>

            <!-- 我的工作 -->
            <template v-else-if="tab === 'work'">
              <div class="mb-10px rounded-8px bg-gray-50 p-12px dark:bg-gray-800">
                <div class="flex flex-wrap items-center gap-8px text-13px">
                  <strong>{{ data.work?.talentName || '—' }}</strong>
                  <span class="text-12px text-gray-500">{{ data.work?.talentDescription }}</span>
                  <NButton class="ml-auto" size="small" :disabled="busy" @click="claimWages">
                    {{ $t('page.farm.milkTea.claimWages') }}
                  </NButton>
                </div>
                <div v-if="data.work?.myShift" class="mt-8px text-12px text-gray-600 dark:text-gray-300">
                  {{ $t('page.farm.milkTea.shiftAt') }} {{ data.work.myShift.shopName }}
                  · {{ shiftRemaining(data.work.myShift) }}
                  · {{ data.work.myShift.interactCount }}/{{ data.work.myShift.interactLimit }}
                </div>
              </div>
              <div class="mb-10px flex flex-col gap-6px">
                <div
                  v-for="interaction in data.interactions ?? []"
                  :key="interaction.interactionId"
                  class="flex flex-wrap items-center gap-8px rounded-6px border border-gray-200 px-10px py-8px text-13px dark:border-gray-700"
                >
                  <img v-if="itemImage(interaction)" :src="itemImage(interaction)" class="h-24px w-24px object-contain" loading="lazy" />
                  <span class="font-medium">{{ interaction.name }}</span>
                  <span class="text-12px text-gray-500">{{ interaction.description }}</span>
                  <NButton
                    class="ml-auto"
                    size="tiny"
                    :disabled="busy || !data.work?.myShift || data.work?.interactDone || !interaction.ready"
                    @click="interact(interaction.interactionId)"
                  >
                    {{ $t('page.farm.milkTea.interactAction') }}
                  </NButton>
                </div>
              </div>
              <div class="flex flex-col gap-6px">
                <div
                  v-for="slip in data.work?.slips ?? []"
                  :key="slip.shiftId"
                  class="flex flex-wrap items-center gap-8px rounded-6px border border-gray-200 px-10px py-6px text-12px dark:border-gray-700"
                >
                  <span>{{ slip.shopName }}</span>
                  <span class="text-gray-500">{{ durationText(slip.workSec) }}</span>
                  <span>{{ slip.wages?.finalWage }}{{ $t('page.farm.milkTea.token') }}</span>
                  <NTag size="tiny" :type="slip.wageStatus === 1 ? 'success' : 'default'">
                    {{ slip.wageStatus === 1 ? $t('page.farm.milkTea.wageClaimable') : $t('page.farm.milkTea.wageSettled') }}
                  </NTag>
                </div>
              </div>
            </template>

            <!-- 兑换商店 -->
            <template v-else-if="tab === 'exchange'">
              <div class="grid grid-cols-2 gap-8px sm:grid-cols-3 lg:grid-cols-4">
                <div
                  v-for="goods in data.goods ?? []"
                  :key="goods.id"
                  class="rounded-8px border border-gray-200 p-10px text-12px dark:border-gray-700"
                  :class="{ 'opacity-50': !goods.available }"
                >
                  <div class="font-medium">{{ goods.name }}</div>
                  <div class="mt-4px flex flex-wrap gap-6px">
                    <span v-for="reward in goods.rewards" :key="reward.id" class="flex items-center gap-4px">
                      <img v-if="imageOfReward(reward)" :src="imageOfReward(reward)" class="h-18px w-18px object-contain" loading="lazy" />
                      {{ reward.name }} ×{{ reward.count }}
                    </span>
                  </div>
                  <div class="mt-4px text-gray-500">
                    {{ $t('page.farm.milkTea.cost') }}：
                    {{ (goods.costs ?? []).map((cost: any) => `${cost.name} ×${cost.count}`).join('、') }}
                  </div>
                  <div v-if="(goods.conditions ?? []).length" class="mt-2px text-gray-400">
                    {{ (goods.conditions ?? []).map((cond: any) => `${cond.type} ${cond.current}/${cond.target}`).join(' · ') }}
                  </div>
                  <div class="mt-6px flex items-center gap-6px">
                    <NInputNumber
                      v-model:value="exchangeCount[goods.id]"
                      size="tiny"
                      class="w-80px"
                      :min="1"
                      :max="goods.remaining ?? 9999"
                    />
                    <NButton size="tiny" type="primary" :disabled="busy || !goods.available" @click="exchange(goods)">
                      {{ $t('page.farm.milkTea.exchange') }}
                    </NButton>
                  </div>
                </div>
              </div>
            </template>

            <!-- 成长基金 -->
            <template v-else-if="tab === 'fund'">
              <div class="mb-8px flex flex-wrap items-center gap-8px">
                <NTag size="small" :type="data.fund?.unlocked ? 'success' : 'default'">
                  {{ data.fund?.unlocked ? $t('page.farm.milkTea.fundUnlocked') : $t('page.farm.milkTea.fundLocked') }}
                </NTag>
                <span class="text-12px text-gray-500">
                  {{ (data.fund?.costs ?? []).map((cost: any) => `${cost.name} ×${cost.count}`).join('、') }}
                </span>
                <NButton
                  size="small"
                  type="primary"
                  :disabled="busy || data.fund?.unlocked"
                  @click="unlockFund"
                >
                  {{ $t('page.farm.milkTea.unlock') }}
                </NButton>
                <NButton size="small" :disabled="busy" @click="claimFund">
                  {{ $t('page.farm.milkTea.fundClaim') }}
                </NButton>
              </div>
              <div class="flex flex-col gap-6px">
                <div
                  v-for="tier in data.fund?.tiers ?? []"
                  :key="tier.id"
                  class="rounded-6px border border-gray-200 px-10px py-8px text-12px dark:border-gray-700"
                  :class="{ 'border-emerald-400': tier.state === 2 }"
                >
                  <div class="flex items-center gap-8px">
                    <strong>#{{ tier.id }}</strong>
                    <span>{{ rewardsText(tier.rewards) }}</span>
                    <NTag size="tiny" class="ml-auto">{{ tier.state }}</NTag>
                  </div>
                  <div class="mt-4px text-gray-500">
                    {{ (tier.conditions ?? []).map((cond: any) => `${cond.type} ${cond.current}/${cond.target}${cond.reached ? ' ✓' : ''}`).join(' · ') }}
                  </div>
                </div>
              </div>
            </template>

            <!-- 排行 -->
            <template v-else-if="tab === 'rank'">
              <div class="mb-8px flex items-center gap-8px text-12px text-gray-500">
                <span>{{ $t('page.farm.milkTea.myRank') }}：{{ data.rank?.myRank ?? '—' }}</span>
                <NButton size="tiny" :disabled="busy || rankPage <= 1" @click="loadRank(rankPage - 1)">
                  {{ $t('page.farm.milkTea.prevPage') }}
                </NButton>
                <NButton size="tiny" :disabled="busy || !data.rank?.hasMore" @click="loadRank(rankPage + 1)">
                  {{ $t('page.farm.milkTea.nextPage') }}
                </NButton>
              </div>
              <div class="flex flex-col gap-6px">
                <div
                  v-for="entry in data.rank?.ranks ?? []"
                  :key="entry.rank"
                  class="flex items-center gap-8px rounded-6px border border-gray-200 px-10px py-6px text-13px dark:border-gray-700"
                >
                  <b>{{ entry.rank }}</b>
                  <span>{{ entry.shopName }}</span>
                  <span class="text-12px text-gray-500">{{ entry.owner?.name }}</span>
                  <span class="ml-auto text-12px">{{ entry.totalRevenue }} · {{ $t('page.farm.milkTea.levelShort') }}{{ entry.level }}</span>
                </div>
              </div>
            </template>

            <!-- 营业记录 -->
            <template v-else-if="tab === 'logs'">
              <div class="mb-8px flex items-center gap-8px text-12px text-gray-500">
                <span>Day {{ data.logs?.day }}</span>
                <NButton size="tiny" :disabled="busy" @click="loadLogs((data.logs?.day || 0) - 86400)">
                  {{ $t('page.farm.milkTea.prevDay') }}
                </NButton>
              </div>
              <div class="mb-8px text-13px">
                {{ $t('page.farm.milkTea.token') }}：{{ data.logs?.token }} · 金币：{{ data.logs?.coin }} ·
                {{ $t('page.farm.milkTea.cups') }}：{{ data.logs?.cups }}
              </div>
              <div class="mb-8px flex flex-wrap gap-8px text-12px">
                <span v-for="cup in data.logs?.items ?? []" :key="cup.id" class="flex items-center gap-4px">
                  <img v-if="itemImage(cup)" :src="itemImage(cup)" class="h-18px w-18px object-contain" loading="lazy" />
                  {{ cup.name }} ×{{ cup.count }}
                </span>
              </div>
              <div class="flex flex-col gap-6px">
                <div
                  v-for="hireEntry in data.logs?.hires ?? []"
                  :key="hireEntry.shiftId"
                  class="flex flex-wrap items-center gap-8px rounded-6px border border-gray-200 px-10px py-6px text-12px dark:border-gray-700"
                >
                  <span>{{ hireEntry.friend?.name || hireEntry.friend?.gid }}</span>
                  <span class="text-gray-500">{{ durationText(hireEntry.workSec) }}</span>
                  <span>{{ hireEntry.wages?.finalWage }}{{ $t('page.farm.milkTea.token') }}</span>
                  <NTag size="tiny" :type="hireEntry.settled ? 'success' : 'default'">
                    {{ hireEntry.settled ? $t('page.farm.milkTea.wageSettled') : $t('page.farm.milkTea.wageClaimable') }}
                  </NTag>
                  <span v-if="hireEntry.interactionResult" class="ml-auto">
                    {{ hireEntry.interactionResult.name }} · {{ hireEntry.interactionResult.reward?.name }} ×{{ hireEntry.interactionResult.reward?.count }}
                  </span>
                </div>
              </div>
            </template>
          </template>
        </template>
        <div v-else-if="error" class="py-32px text-center text-gray-500">
          <strong>{{ error }}</strong>
          <div class="mt-8px">
            <NButton size="small" :loading="busy" @click="reload">{{ $t('page.farm.milkTea.refresh') }}</NButton>
          </div>
        </div>
        <div v-else class="py-32px text-center text-gray-500">{{ $t('page.farm.milkTea.loading') }}</div>
      </NSpin>
    </NCard>

    <NModal v-model:show="renameOpen">
      <NCard
        class="w-[calc(100vw-32px)] max-w-sm"
        :title="$t('page.farm.milkTea.rename')"
        closable
        @close="renameOpen = false"
      >
        <NInput v-model:value="renameName" :placeholder="$t('page.farm.milkTea.shopName')" />
        <template #footer>
          <div class="flex justify-end gap-8px">
            <NButton size="small" @click="renameOpen = false">{{ $t('common.cancel') }}</NButton>
            <NButton size="small" type="primary" :loading="busy" @click="renameShop">
              {{ $t('page.farm.milkTea.renameConfirm') }}
            </NButton>
          </div>
        </template>
      </NCard>
    </NModal>
  </div>
</template>
