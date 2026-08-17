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
  enter_farm: '进入农场',
  care_friend: '照顾好友',
  patrol_done: '巡查完成',
  avatar_probe: '人机头像诊断',
  visitor_gid_backfill: '访客补充好友GID',
  bad_action_limit: '放虫放草次数上限',
  heartbeat_timeout: '心跳超时',
  login: '登录'
};

export function getLogEventLabel(event?: string | null): string {
  const key = String(event || '').trim();
  if (!key) return '';
  return EVENT_LABELS[key] || key;
}
