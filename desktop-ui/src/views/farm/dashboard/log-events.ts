/** Align qq-farm-bot Dashboard event chips: internal keys → Chinese labels. */
const EVENT_LABELS: Record<string, string> = {
  farm_cycle: '农场巡查',
  harvest_crop: '收获作物',
  remove_plant: '清理枯株',
  plant_seed: '种植种子',
  fertilize: '施加化肥',
  lands_notify: '土地推送',
  seed_pick: '选择种子',
  seed_buy: '购买种子',
  fertilizer_buy: '购买化肥',
  fertilizer_gift_open: '开启礼包',
  fertilizer_buy_timer: '购买化肥计时器',
  task_scan: '获取任务',
  task_claim: '完成任务',
  daily_task: '每日任务',
  activity_points: '活跃度',
  mall_free_gifts: '免费礼包',
  daily_share: '分享奖励',
  vip_daily_gift: '会员礼包',
  month_card_gift: '月卡礼包',
  illustrated_rewards: '图鉴奖励',
  email_rewards: '邮箱领取',
  sell_success: '出售成功',
  sell_done: '出售完成',
  upgrade_land: '土地升级',
  unlock_land: '土地解锁',
  friend_cycle: '好友巡查',
  visit_friend: '访问好友',
  friend_scan: '好友扫描',
  friend_request: '好友申请',
  accept_friend_request: '同意好友申请',
  pending_friend_request: '待处理申请',
  get_friend_list: '获取好友列表',
  friend_list_api: '好友列表接口',
  friend_plant_patch: '好友气泡',
  enter_farm: '进入农场',
  care_friend: '照顾好友',
  patrol_done: '巡查完成',
  avatar_probe: '人机头像诊断',
  visitor_gid_backfill: '访客补充好友GID',
  bad_action_limit: '放虫放草次数上限',
  heartbeat_timeout: '心跳超时',
  login: '登录',
  disconnect_stop: '断开停止',
  kickout_stop: '踢下线',
  kickout: '被踢下线',
  ws_400: '登录失效',
  ws_close: '连接关闭',
  offline_delete: '离线删除'
};

const PHASE_LABELS: Record<string, string> = {
  online: '在线',
  login: '登录',
  connecting: '连接中',
  closing: '断开中'
};

export function getLogEventLabel(event?: string | null): string {
  const key = String(event || '').trim();
  if (!key) return '';
  return EVENT_LABELS[key] || key;
}

/** Tag already says the same thing (e.g. 心跳 + 心跳超时). */
export function shouldShowEventChip(tag?: string | null, event?: string | null): boolean {
  const t = String(tag || '').trim();
  const e = String(event || '').trim();
  if (!e) return false;
  if (t === e) return false;
  if (t === '心跳' && e === '心跳超时') return false;
  return true;
}

/** Turn leftover English keys in log text into Chinese labels. */
export function humanizeLogMessage(message?: string | null): string {
  let out = String(message || '');
  if (!out) return '';
  out = out.replace(
    /\(\s*source=([a-z0-9_]+)\s*,\s*phase=([a-z0-9_]+)\s*\)/gi,
    (_m, source: string, phase: string) => {
      const reason = EVENT_LABELS[source] || source;
      if (phase === 'online') return `（${reason}）`;
      const phaseLabel = PHASE_LABELS[phase] || phase;
      return `（${reason}，${phaseLabel}阶段）`;
    }
  );
  out = out.replace(/\b(source|phase)=([a-z0-9_]+)/gi, (_m, key: string, val: string) => {
    const mapped = EVENT_LABELS[val] || PHASE_LABELS[val] || val;
    return String(key).toLowerCase() === 'phase' ? `${mapped}阶段` : mapped;
  });
  out = out.replace(/\b(heartbeat_timeout|kickout|ws_close|disconnect_stop|kickout_stop|ws_400)\b/g, token => {
    return EVENT_LABELS[token] || token;
  });
  return out;
}

export function logText(payload: Record<string, unknown>): string {
  const raw =
    (typeof payload.message === 'string' && payload.message) ||
    (typeof payload.msg === 'string' && payload.msg) ||
    '';
  return humanizeLogMessage(raw);
}

/** Internal event keys hidden from the dashboard log panel. */
export const HIDDEN_LOG_EVENT_KEYS = new Set(['avatar_probe']);

/** Dashboard log event filter options (align qq-farm-bot). */
export const LOG_EVENT_FILTER_OPTIONS: Array<{ label: string; value: string }> = [
  { label: '所有事件', value: '' },
  ...Object.entries(EVENT_LABELS)
    .filter(([value]) => !HIDDEN_LOG_EVENT_KEYS.has(value))
    .map(([value, label]) => ({ label, value }))
];

export function isHiddenLogEvent(eventKey?: string | null, eventLabel?: string | null, message?: string | null): boolean {
  const key = String(eventKey || '').trim();
  if (key && HIDDEN_LOG_EVENT_KEYS.has(key)) return true;
  const label = String(eventLabel || '').trim();
  if (label === EVENT_LABELS.avatar_probe) return true;
  // Avoid fuzzy message matching: it can accidentally hide unrelated logs.
  // We only hide by exact event key / label.
  return false;
}
