<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { NButton, NCard, NModal, NSpin, useMessage } from 'naive-ui';
import { fetchSolarCalendarOperate, fetchSolarCalendarSnapshot } from '@/service/api';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { useManagedInterval } from '@/hooks/common/use-managed-interval';
import { resolveCatalogImage } from '@/views/farm/game-config/shared';
import { $t } from '@/locales';

defineOptions({ name: 'FarmSolarCalendar' });

const farmAccountStore = useFarmAccountStore();
const message = useMessage();

const data = ref<any>(null);
const loading = ref(false);
const busy = ref(false);
const error = ref('');
const seasonId = ref(0);
const questionId = ref(0);
const answer = ref<number | null>(null);
const receivedAt = ref(Date.now());
const tick = ref(Date.now());
let sequence = 0;

const connected = computed(() => farmAccountStore.currentAccount?.runStatus === 1);
const now = computed(() => Number(data.value?.serverTime || 0) + (tick.value - receivedAt.value) / 1000);
const season = computed(() => data.value?.seasons?.find((entry: any) => entry.id === seasonId.value));
const events = computed<any[]>(() =>
  (data.value?.events ?? []).filter(
    (event: any) => !season.value || season.value.eventIds.includes(event.id)
  )
);
const question = computed(() => data.value?.events?.find((entry: any) => entry.id === questionId.value));

/** 下一个需要自动刷新的时间边界（事件开始 / 结束 / 答题重置） */
const nextBoundary = computed(() => {
  const snapshotTime = Number(data.value?.serverTime || 0);
  const times = (data.value?.events ?? []).flatMap((event: any) => [event.startTime, event.endTime + 1]);
  if ((data.value?.events ?? []).some((event: any) => event.quizWrongToday)) {
    times.push((Math.floor((snapshotTime + 28800) / 86400) + 1) * 86400 - 28800);
  }
  const upcoming = times.filter((time: number) => time > snapshotTime);
  return upcoming.length ? Math.min(...upcoming) : 0;
});
let refreshedBoundary = 0;

const clock = useManagedInterval();

function distance(seconds: number): string {
  if (seconds >= 86400) return $t('page.farm.solarCalendar.dayUnit', { n: Math.ceil(seconds / 86400) });
  if (seconds >= 3600) return $t('page.farm.solarCalendar.hourUnit', { n: Math.ceil(seconds / 3600) });
  return $t('page.farm.solarCalendar.minuteUnit', { n: Math.max(1, Math.ceil(seconds / 60)) });
}

function phase(event: any): 'done' | 'expired' | 'upcoming' | 'open' {
  if (event.directClaimed && (!event.question || event.quizClaimed)) return 'done';
  if (now.value > event.endTime || event.status === 3) return 'expired';
  return now.value < event.startTime ? 'upcoming' : 'open';
}

function phaseText(event: any): string {
  const current = phase(event);
  if (current === 'done') return $t('page.farm.solarCalendar.phaseDone');
  if (current === 'expired') return $t('page.farm.solarCalendar.phaseExpired');
  if (current === 'upcoming') {
    return $t('page.farm.solarCalendar.phaseUpcoming', {
      distance: distance(Math.ceil(event.startTime - now.value))
    });
  }
  return $t('page.farm.solarCalendar.phaseOpen', {
    distance: distance(Math.ceil(event.endTime - now.value))
  });
}

function stateText(event: any, quiz = false): string {
  if (quiz ? event.quizClaimed : event.directClaimed) return $t('page.farm.solarCalendar.stateClaimed');
  if (now.value > event.endTime || event.status === 3) return $t('page.farm.solarCalendar.stateExpired');
  const seconds = Math.ceil(event.startTime - now.value);
  if (seconds > 0) {
    return quiz
      ? $t('page.farm.solarCalendar.stateUpcomingQuiz', { distance: distance(seconds) })
      : $t('page.farm.solarCalendar.stateUpcomingClaim', { distance: distance(seconds) });
  }
  if (quiz) return event.quizWrongToday ? $t('page.farm.solarCalendar.tomorrow') : $t('page.farm.solarCalendar.stateQuiz');
  return $t('page.farm.solarCalendar.stateClaim');
}

function available(event: any, quiz = false): boolean {
  return (
    connected.value &&
    !busy.value &&
    (quiz ? event.canAnswer : event.canClaim) &&
    now.value >= event.startTime &&
    now.value <= event.endTime
  );
}

/** 撕页日历行：首行为农历日期，其余为节令诗句 */
function lunarOf(event: any): string {
  const first = String(event.description || '').split('\n')[0] || '';
  return first.startsWith('农历') ? first : '';
}

function poemOf(event: any): string[] {
  const lines = String(event.description || '').split('\n').filter(Boolean);
  return lunarOf(event) ? lines.slice(1) : lines;
}

/** "A.敬老节" 拆成字母标和选项文字 */
function optionParts(option: string, index: number | string) {
  const match = /^([A-Z])[.．、](.*)$/.exec(option);
  return match
    ? { letter: match[1], text: (match[2] || '').trim() }
    : { letter: String.fromCharCode(65 + Number(index)), text: option };
}

function rewardImage(reward: any): string {
  return resolveCatalogImage(reward.image);
}

async function request(action = '', input: Record<string, unknown> = {}) {
  const accountId = farmAccountStore.currentAccountId;
  if (!accountId || !connected.value || busy.value) return;
  const revision = ++sequence;
  busy.value = true;
  error.value = '';
  try {
    const response = action
      ? await fetchSolarCalendarOperate(accountId, action as 'claim' | 'answer' | 'season', input)
      : await fetchSolarCalendarSnapshot(accountId);
    if (revision !== sequence) return;
    if (response.error || !response.data) {
      throw new Error(response.error?.message || $t('page.farm.solarCalendar.requestFailed'));
    }
    const result: any = response.data;
    if (action) {
      questionId.value = 0;
      const text =
        (result.rewards ?? []).map((reward: any) => `${reward.name} ×${reward.count}`).join('、') ||
        (action === 'answer' && !result.correct
          ? $t('page.farm.solarCalendar.tomorrow')
          : $t('page.farm.solarCalendar.claimSuccess'));
      if (result.refreshRequired) {
        message.warning(`${text}。${$t('page.farm.solarCalendar.refreshRequired')}`);
      } else {
        message.success(text);
      }
    }
    data.value = action ? result.calendar : result;
    receivedAt.value = Date.now();
    tick.value = receivedAt.value;
    if (!data.value?.seasons?.some((entry: any) => entry.id === seasonId.value)) {
      const target =
        data.value?.events?.find((event: any) => !event.directClaimed && event.endTime >= now.value)?.id ??
        data.value?.defaultEventId;
      seasonId.value =
        data.value?.seasons?.find((entry: any) => entry.eventIds.includes(target))?.id ??
        data.value?.seasons?.at(-1)?.id ??
        0;
    }
  } catch (cause) {
    if (revision === sequence) {
      error.value = (cause as Error)?.message || $t('page.farm.solarCalendar.requestFailed');
      if (action) {
        message.error(error.value);
        data.value = null;
        questionId.value = 0;
      }
    }
  } finally {
    if (revision === sequence) busy.value = false;
  }
}

function openQuestion(event: any) {
  answer.value = null;
  questionId.value = event.id;
}

function submitAnswer() {
  const event = question.value;
  if (!event || answer.value === null) return;
  void request('answer', {
    id: event.id,
    optionIndex: answer.value,
    expectedQuestion: event.question
  });
}

watch(
  () => [farmAccountStore.currentAccountId, connected.value] as const,
  () => {
    sequence++;
    data.value = null;
    busy.value = false;
    questionId.value = 0;
    answer.value = null;
    seasonId.value = 0;
    error.value = '';
    refreshedBoundary = 0;
    void load();
  },
  { immediate: true }
);

watch([now, busy], () => {
  const boundary = nextBoundary.value;
  if (
    connected.value &&
    !busy.value &&
    boundary > 0 &&
    now.value >= boundary &&
    refreshedBoundary !== boundary
  ) {
    refreshedBoundary = boundary;
    void request();
  }
});

async function load() {
  loading.value = true;
  try {
    await request();
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  clock.start(() => {
    tick.value = Date.now();
  }, 1000);
});

onUnmounted(() => {
  clock.stop();
  sequence++;
});

defineExpose({ refresh: load });
</script>

<template>
  <div class="solar mx-auto max-w-1100px">
    <NCard :title="$t('page.farm.solarCalendar.title')" size="small" :bordered="false" class="card-wrapper">
      <template #header-extra>
        <NButton size="small" :loading="busy" :disabled="!farmAccountStore.currentAccountId || !connected" @click="load">
          {{ $t('page.farm.solarCalendar.refresh') }}
        </NButton>
      </template>

      <NSpin :show="loading">
        <div v-if="!farmAccountStore.currentAccountId || !connected" class="py-32px text-center text-gray-500">
          <div class="text-28px">📅</div>
          <strong>
            {{
              farmAccountStore.currentAccountId
                ? $t('page.farm.solarCalendar.startAccount')
                : $t('page.farm.solarCalendar.selectAccount')
            }}
          </strong>
        </div>
        <div v-else-if="error || !data" class="py-32px text-center text-gray-500">
          <strong>{{ error || $t('page.farm.solarCalendar.empty') }}</strong>
        </div>
        <template v-else>
          <nav class="mb-10px flex flex-wrap gap-8px" :aria-label="$t('page.farm.solarCalendar.seasonAria')">
            <NButton
              v-for="entry in data.seasons"
              :key="entry.id"
              size="small"
              :type="seasonId === entry.id ? 'primary' : 'default'"
              :disabled="busy"
              @click="seasonId = entry.id"
            >
              {{ entry.name }}（{{ entry.litCount }}/{{ entry.totalCount }}）
            </NButton>
          </nav>

          <div v-if="season" class="mb-12px rounded-8px bg-gray-50 p-12px dark:bg-gray-800">
            <div class="mb-8px flex items-center gap-12px">
              <strong>{{ season.name }}{{ $t('page.farm.solarCalendar.seasonLit') }}</strong>
              <span class="flex-1" />
              <b>{{ season.litCount }}/{{ season.totalCount }}</b>
            </div>
            <div v-if="season.rewards.length" class="flex flex-wrap items-center gap-8px">
              <span class="text-12px text-gray-500">{{ $t('page.farm.solarCalendar.seasonRewards') }}</span>
              <span
                v-for="reward in season.rewards"
                :key="reward.id"
                class="flex items-center gap-4px text-12px"
                :title="`${reward.name} ×${reward.count}`"
              >
                <img v-if="rewardImage(reward)" :src="rewardImage(reward)" class="h-18px w-18px object-contain" loading="lazy" />
                {{ reward.name }} ×{{ reward.count }}
              </span>
              <NButton
                size="small"
                :type="season.canClaim ? 'primary' : 'default'"
                :disabled="busy || !season.canClaim"
                @click="request('season', { id: season.id })"
              >
                {{
                  season.claimed
                    ? $t('page.farm.solarCalendar.claimed')
                    : season.canClaim
                      ? $t('page.farm.solarCalendar.claim')
                      : $t('page.farm.solarCalendar.claimLocked')
                }}
              </NButton>
            </div>
          </div>

          <ol class="flex flex-col gap-8px">
            <li v-for="event in events" :key="event.id" class="rounded-8px border border-gray-200 p-12px dark:border-gray-700">
              <div class="flex items-center gap-8px">
                <h3 class="text-15px font-semibold">{{ event.name }}</h3>
                <span v-if="lunarOf(event)" class="text-12px text-gray-500">{{ lunarOf(event) }}</span>
                <span class="ml-auto text-12px text-gray-500">{{ phaseText(event) }}</span>
              </div>
              <p v-if="poemOf(event).length" class="my-6px whitespace-pre-line text-12px text-gray-500">
                {{ poemOf(event).join('\n') }}
              </p>
              <div class="flex flex-wrap items-center gap-8px">
                <span class="text-12px text-gray-500">{{ $t('page.farm.solarCalendar.termRewards') }}</span>
                <span
                  v-for="reward in event.rewards"
                  :key="reward.id"
                  class="flex items-center gap-4px text-12px"
                  :title="`${reward.name} ×${reward.count}`"
                >
                  <img v-if="rewardImage(reward)" :src="rewardImage(reward)" class="h-18px w-18px object-contain" loading="lazy" />
                  {{ reward.name }} ×{{ reward.count }}
                </span>
                <NButton
                  size="small"
                  class="ml-auto"
                  :type="available(event) ? 'primary' : 'default'"
                  :disabled="!available(event)"
                  @click="request('claim', { id: event.id })"
                >
                  {{ stateText(event) }}
                </NButton>
              </div>
              <div v-if="event.question" class="mt-6px flex flex-wrap items-center gap-8px">
                <span class="text-12px text-gray-500">{{ $t('page.farm.solarCalendar.quizRewards') }}</span>
                <span
                  v-for="reward in event.questionRewards"
                  :key="reward.id"
                  class="flex items-center gap-4px text-12px"
                  :title="`${reward.name} ×${reward.count}`"
                >
                  <img v-if="rewardImage(reward)" :src="rewardImage(reward)" class="h-18px w-18px object-contain" loading="lazy" />
                  {{ reward.name }} ×{{ reward.count }}
                </span>
                <NButton
                  size="small"
                  class="ml-auto"
                  secondary
                  :type="available(event, true) ? 'primary' : 'default'"
                  :disabled="!available(event, true)"
                  @click="openQuestion(event)"
                >
                  {{ stateText(event, true) }}
                </NButton>
              </div>
              <p v-if="event.quizWrongToday && event.errorTips" class="mt-6px text-12px text-gray-500">
                {{ event.errorTips }}
              </p>
            </li>
          </ol>
        </template>
      </NSpin>
    </NCard>

    <NModal v-if="question" :show="true" :mask-closable="!busy" :close-on-esc="!busy" @update:show="!busy && (questionId = 0)">
      <NCard
        class="w-[calc(100vw-32px)] max-w-lg"
        :title="`${question.name} · ${$t('page.farm.solarCalendar.quizTitle')}`"
        :closable="!busy"
        @close="questionId = 0"
      >
        <p class="mb-12px text-14px">{{ question.question }}</p>
        <div class="flex flex-col gap-8px">
          <label
            v-for="(option, index) in question.options"
            :key="index"
            class="flex items-center gap-8px rounded-6px border border-gray-200 px-10px py-8px text-13px dark:border-gray-700"
            :class="{
              'border-emerald-400 bg-emerald-50/50 dark:bg-emerald-900/20': answer === index,
              'opacity-50': question.wrongOptions.includes(index)
            }"
          >
            <input
              v-model="answer"
              type="radio"
              name="solar-answer"
              :value="index"
              :disabled="busy || question.wrongOptions.includes(index)"
            />
            <b>{{ optionParts(option, index).letter }}</b>
            <span>{{ optionParts(option, index).text }}</span>
            <small v-if="question.wrongOptions.includes(index)" class="ml-auto text-gray-400">
              {{ $t('page.farm.solarCalendar.wrongOption') }}
            </small>
          </label>
        </div>
        <template #footer>
          <NButton
            block
            type="primary"
            :loading="busy"
            :disabled="answer === null || !available(question, true)"
            @click="submitAnswer"
          >
            {{ $t('page.farm.solarCalendar.submitAnswer') }}
          </NButton>
        </template>
      </NCard>
    </NModal>
  </div>
</template>
