declare namespace Api {
  namespace Farm {
    type CommonSearchParams = Pick<Common.PaginatingCommonParams, 'current' | 'size'>;

    /** enable status — "1" enable / "2" disable */
    type EnableStatus = Common.EnableStatus;

    /** run status — 0 stopped / 1 running / 2 error */
    type RunStatus = 0 | 1 | 2;

    type Platform = 'qq' | 'wx';

    type FertilizerMode = 'none' | 'normal' | 'organic' | 'both' | 'smart';

    type FertilizerBuyType = 'organic' | 'normal' | 'both';

    type FertilizerBuyMode = 'threshold' | 'unlimited';

    /** farm account */
    type Account = {
      id: number;
      name: string;
      code: string;
      platform: Platform | string;
      uin: string;
      qq: string;
      avatar: string;
      username: string;
      remark: string;
      runStatus: RunStatus;
      lastOnlineAt: number;
      status: EnableStatus;
      createdAt: number;
      updatedAt: number;
    };

    type AccountSearchParams = CommonType.RecordNullable<
      Pick<Account, 'status' | 'runStatus' | 'platform'> & CommonSearchParams & { keyword?: string }
    >;

    type AccountList = Common.PaginatingQueryRecord<Account>;

    type AccountCreateParams = {
      /** bare gateway auth code, or full login URL containing code= */
      code: string;
      name?: string;
      platform: Platform | string;
      remark?: string;
    };

    type AccountUpdateParams = {
      id: number;
      /** refresh one-time login code / URL */
      code: string;
      name?: string;
      platform: Platform | string;
      remark?: string;
      status?: EnableStatus;
    };

    type AccountIdParams = {
      id: number;
    };

    /** automation config (stored as JSON on backend, aligned with Go logic.AutomationConfig) */
    type AutomationConfig = {
      farm?: boolean;
      farm_manage?: boolean;
      farm_water?: boolean;
      farm_weed?: boolean;
      farm_bug?: boolean;
      farm_push?: boolean;
      land_upgrade?: boolean;
      friend?: boolean;
      friend_help_exp_limit?: boolean;
      friend_steal?: boolean;
      friend_help?: boolean;
      friend_bad?: boolean;
      task?: boolean;
      fertilizer_gift?: boolean;
      fertilizer_buy_organic?: boolean;
      fertilizer_buy_normal?: boolean;
      sell?: boolean;
      fertilizer?: FertilizerMode | string;
      fertilizer_multi_season?: boolean;
      fertilizer_land_types?: string[];
      fertilizer_smart_seconds?: number;
      skip_own_weed_bug?: boolean;
      email?: boolean;
      free_gifts?: boolean;
      share_reward?: boolean;
      vip_gift?: boolean;
      month_card?: boolean;
      open_server_gift?: boolean;
    };

    type IntervalsConfig = {
      farm?: number;
      farmMin?: number;
      farmMax?: number;
      helpMin?: number;
      helpMax?: number;
      stealMin?: number;
      stealMax?: number;
    };

    type QuietHoursConfig = {
      enabled?: boolean;
      start?: string;
      end?: string;
    };

    type AccountAutomationDetail = {
      accountId: number;
      automation: AutomationConfig;
      intervals?: IntervalsConfig;
      plantingStrategy?: string;
      preferredSeedId?: number;
      bagSeedPriority?: number[];
      bagSeedFallbackStrategy?: string;
      plantOrderRandom?: boolean;
      plantDelaySeconds?: number;
      stealDelaySeconds?: number;
      friendQuietHours?: QuietHoursConfig;
      friendBlacklist?: number[];
      plantBlacklist?: number[];
      fertilizerBuyOrganicCount?: number;
      fertilizerBuyOrganicThresholdHours?: number;
      fertilizerBuyNormalCount?: number;
      fertilizerBuyNormalThresholdHours?: number;
      fertilizerBuyCheckIntervalMinutes?: number;
      configJson?: string;
    };

    type AccountAutomationModifyParams = {
      accountId: number;
      automation?: AutomationConfig;
      intervals?: IntervalsConfig;
      plantingStrategy?: string;
      preferredSeedId?: number;
      bagSeedPriority?: number[];
      bagSeedFallbackStrategy?: string;
      plantOrderRandom?: boolean;
      plantDelaySeconds?: number;
      stealDelaySeconds?: number;
      friendQuietHours?: QuietHoursConfig;
      friendBlacklist?: number[];
      plantBlacklist?: number[];
      fertilizerBuyOrganicCount?: number;
      fertilizerBuyOrganicThresholdHours?: number;
      fertilizerBuyNormalCount?: number;
      fertilizerBuyNormalThresholdHours?: number;
      fertilizerBuyCheckIntervalMinutes?: number;
      configJson?: string;
    };

    /** runtime status snapshot */
    type Status = {
      accountId: number;
      runStatus: RunStatus;
      online: boolean;
      level?: number;
      exp?: number;
      gold?: number;
      nick?: string;
      avatar?: string;
      landCount?: number;
      friendCount?: number;
      lastError?: string;
      updatedAt?: number;
      uptime?: number;
      sessionExpGained?: number;
      sessionGoldGained?: number;
      levelProgress?: {
        current?: number;
        needed?: number;
        level?: number;
      };
      operations?: Record<string, number>;
      nextChecks?: {
        farmRemainSec?: number;
        friendRemainSec?: number;
        helpRemainSec?: number;
        stealRemainSec?: number;
      };
      [key: string]: unknown;
    };

    /** friend */
    type Friend = {
      id?: number;
      accountId: number;
      gid: number;
      nickname: string;
      level?: number;
      gold?: number;
      avatar?: string;
      syncedAt?: number;
      plant?: {
        stealNum?: number;
        dryNum?: number;
        weedNum?: number;
        insectNum?: number;
      };
      [key: string]: unknown;
    };

    type FriendInteractRecord = {
      key: string;
      serverTimeSec?: number;
      serverTimeMs?: number;
      actionType?: number;
      actionLabel?: string;
      actionDetail?: string;
      visitorGid?: number;
      nick?: string;
      avatarUrl?: string;
      cropId?: number;
      cropName?: string;
      cropCount?: number;
      times?: number;
      level?: number;
      landId?: number;
      [key: string]: unknown;
    };

    type FriendSearchParams = CommonType.RecordNullable<CommonSearchParams & { accountId?: number; keyword?: string }>;

    type FriendList = Common.PaginatingQueryRecord<Friend>;

    type FriendSyncResult = {
      accountId: number;
      count?: number;
      synced: boolean;
    };

    type FriendLandsParams = {
      accountId: number;
      gid: number;
    };

    type FriendOpParams = {
      accountId: number;
      gid: number;
      op: 'steal' | 'help' | 'water' | 'weed' | 'bug' | 'bad';
    };

    type FriendOpResult = {
      accountId: number;
      gid: number;
      op: string;
      ok: boolean;
      count?: number;
      summary?: string;
      skipReason?: string;
      helpSummary?: string;
      plants?: string[];
    };

    type LandInfo = {
      id: number;
      [key: string]: unknown;
    };

    type LandRow = {
      id: number;
      unlocked: boolean;
      status: string;
      plantName?: string;
      seedId?: number;
      seedImage?: string;
      phaseName?: string;
      currentSeason?: number;
      totalSeason?: number;
      matureInSec?: number;
      totalGrowTime?: number;
      needWater?: boolean;
      needWeed?: boolean;
      needBug?: boolean;
      stealable?: boolean;
      level: number;
      maxLevel?: number;
      landsLevel?: number;
      landSize?: number;
      couldUnlock?: boolean;
      couldUpgrade?: boolean;
      masterLandId?: number;
      plantSize?: number;
      occupiedByMaster?: boolean;
      occupiedLandIds?: number[];
    };

    type LandSummary = {
      harvestable: number;
      growing: number;
      empty: number;
      dead: number;
      needWater?: number;
      needWeed?: number;
      needBug?: number;
    };

    type LandsResponse = {
      lands: LandRow[];
      summary: LandSummary;
    };

    type OperateParams = {
      accountId: number;
      op: 'all' | 'harvest' | 'clear' | 'plant' | 'upgrade';
    };

    type OperateResult = {
      accountId: number;
      op: string;
      hadWork?: boolean;
      actions?: string[];
    };

    type BagItem = {
      id: number;
      count: number;
      name: string;
      image?: string;
      category?: string;
      itemType?: number;
      sellable?: boolean;
      sellStatus?: string;
      sellCondition?: string;
      priceId?: number;
      price?: number;
      priceUnit?: string;
      level?: number;
      interactionType?: string;
      hoursText?: string;
    };

    type BagOriginalItem = {
      id: number;
      count: number;
      uid?: number;
    };

    type BagResponse = {
      totalKinds: number;
      items: BagItem[];
      originalItems: BagOriginalItem[];
    };

    /** seed entry currently in the bag (bag-priority strategy list) */
    type BagSeed = {
      seedId: number;
      name: string;
      count: number;
      requiredLevel: number;
      plantSize: number;
    };

    type BagSellParams = {
      accountId: number;
      items: BagOriginalItem[];
    };

    type BagSellResult = {
      accountId: number;
      count: number;
      ok: boolean;
    };

    type BagUseParams = {
      accountId: number;
      itemId: number;
      count: number;
    };

    type BagUseResult = {
      accountId: number;
      itemId: number;
      count: number;
      ok: boolean;
    };

    type DailyGiftCard = {
      key: string;
      label: string;
      enabled: boolean;
      doneToday: boolean;
      lastAt?: number;
      completedCount?: number;
      totalCount?: number;
      mode?: string;
      checkStatus?: string;
      canShare?: boolean | null;
      hasGift?: boolean | null;
      canClaim?: boolean | null;
      hasCard?: boolean | null;
      hasClaimable?: boolean | null;
      result?: string;
    };

    type GrowthTaskRow = {
      id: number;
      desc: string;
      progress: number;
      totalProgress: number;
      isClaimed: boolean;
      isUnlocked: boolean;
      isCompleted: boolean;
    };

    type GrowthTaskOverview = {
      key: string;
      label: string;
      doneToday: boolean;
      completedCount: number;
      totalCount: number;
      tasks: GrowthTaskRow[];
    };

    type DailyGiftsResponse = {
      date: string;
      growth: GrowthTaskOverview;
      gifts: DailyGiftCard[];
    };

    /** account-scoped shop seeds (settings preview / preferred select) */
    type AvailableShopSeed = {
      seedId: number;
      goodsId?: number;
      name: string;
      price?: number | null;
      requiredLevel?: number | null;
      size?: number;
      locked: boolean;
      soldOut: boolean;
      unknownMeta?: boolean;
    };

    /** activity */
    type ActivityState = {
      id: number;
      accountId: number;
      activityId: string;
      stateJson: string;
      syncedAt: number;
    };

    type ActivityAction = {
      supported?: boolean;
      enabled?: boolean;
      available?: boolean;
      count?: number;
      reason?: string;
      attemptable?: boolean;
      availabilityKnown?: boolean;
      attemptableCount?: number;
    };

    type ActivitySnapshot = {
      accountId: number;
      states: ActivityState[];
      season: Record<string, unknown>;
      constellation: Record<string, unknown>;
      shop: Record<string, unknown>;
      solarTerms: Record<string, unknown>;
      greenPlum?: Record<string, unknown>;
      capabilities?: Record<string, boolean>;
      actions?: Record<string, ActivityAction>;
      errors?: Record<string, string>;
      rewards?: Array<{
        id?: string | number;
        name?: string;
        count?: string | number;
        image?: string;
      }>;
      message?: string;
      outcome?: string;
      noClaimable?: boolean;
      snapshot?: Omit<ActivitySnapshot, 'accountId' | 'states' | 'snapshot'>;
    };

    type ActivityClaimParams = {
      accountId: number;
      termId?: string;
      itemId?: string;
      count?: number;
      activityId?: string;
      operateType?: number;
      ingredients?: Array<{ uid: number | string; count: number }>;
    };

    /** analytics / daily stats */
    type AnalyticsDetail = {
      accountId: number;
      days?: number;
      sort?: string;
      stats: Array<{
        statDate: string;
        gold: number;
        exp: number;
        harvestCount: number;
        stealCount: number;
        helpCount: number;
        plantCount: number;
      }>;
      rankings?: Array<{
        seedId?: number;
        name?: string;
        level?: number | null;
        growTimeStr?: string;
        expPerHour?: number;
        normalFertilizerExpPerHour?: number;
        profitPerHour?: number;
        normalFertilizerProfitPerHour?: number;
        [key: string]: unknown;
      }>;
      plants?: Array<Record<string, unknown>>;
    };

    type AnalyticsSearchParams = CommonType.RecordNullable<{
      accountId?: number;
      days?: number;
      sort?: string;
    }>;

    /** game config catalog (Plant.json / ItemInfo.json) */
    type GameConfigSeed = {
      seedId: number;
      name: string;
      requiredLevel: number;
      price: number;
      priceId: number;
      image: string;
      seasons: number;
      exp: number;
      growPhases: string;
      growTime: number;
      size: number;
      harvestCount: number;
    };

    type GameConfigFruit = {
      id: number;
      name: string;
      type: number;
      price: number;
      priceId: number;
      level: number;
      assetName: string;
      desc: string;
      effectDesc: string;
      rarity: number;
      maxCount: number;
      maxOwn: number;
      plantId?: number | null;
      seedId?: number | null;
      plantName?: string | null;
      image: string;
    };

    type GameConfigItem = {
      id: number;
      type: number;
      name: string;
      interactionType: string;
      priceId: number;
      price: number;
      level: number;
      assetName: string;
      iconRes: string;
      maxCount: number;
      maxOwn: number;
      canUse: number;
      desc: string;
      effectDesc: string;
      rarity: number;
      rarityColor: string;
      image: string;
    };

    type GameConfigPlant = {
      plantId: number;
      name: string;
      seedId?: number | null;
      fruitId?: number | null;
      fruitCount: number;
      landLevelNeed: number;
      seasons: number;
      growPhases: string;
      exp: number;
      price: number;
      image: string;
    };

    type GameConfigItemType = {
      value: number;
      label: string;
    };

    type GameConfigSeedWriteParams = {
      seedId: number;
      name?: string;
      growPhases?: string;
      landLevelNeed?: number;
      seasons?: number;
      fruitCount?: number;
      price?: number;
      priceId?: number;
      exp?: number;
      size?: number;
    };

    type GameConfigFruitWriteParams = {
      id?: number;
      plantId?: number;
      name?: string;
      price?: number;
      priceId?: number;
      desc?: string;
      effectDesc?: string;
      rarity?: number;
      maxCount?: number;
      level?: number;
      fruitCount?: number;
      assetName?: string;
    };

    type GameConfigItemWriteParams = {
      id: number;
      type?: number;
      name?: string;
      price?: number;
      priceId?: number;
      interactionType?: string;
      canUse?: number;
      desc?: string;
      effectDesc?: string;
      rarity?: number;
      maxCount?: number;
      level?: number;
      assetName?: string;
    };

    /** websocket message envelope */
    type WsMessage<T = unknown> = {
      type: string;
      payload?: T;
      accountId?: number;
    };

    /** In-memory run-log entry (GET /farm/logs) */
    type LogEntry = {
      time: string;
      tag: string;
      msg: string;
      isWarn?: boolean;
      meta?: { module?: string; event?: string };
      accountId?: number;
      ts?: number;
    };

    type LogsSearchParams = {
      accountId?: number;
      module?: string;
      keyword?: string;
      limit?: number;
    };

    type WxLoginTask = {
      task_id: string;
      app_id: string;
      status: string;
      expires_at: number;
      qr_url?: string;
    };

    type WxLoginCodeResult = {
      openid: string;
      app_id: string;
      code: string;
      err_msg: string;
    };

    type CommerceItem = {
      id: number;
      count: number;
      name: string;
      image: string;
      rarity: number;
    };

    type PurchaseLimit = {
      type: number;
      bought: number;
      max: number;
      remaining: number | null;
    };

    type MallPrice = CommerceItem & {
      balance: number | null;
    };

    type MallGoods = {
      id: number;
      name: string;
      type: number;
      rewards: CommerceItem[];
      price: MallPrice;
      isFree: boolean;
      limit: PurchaseLimit | null;
      isLimited: boolean;
      discountText: string;
      isDiscounted: boolean;
      discountEndTime: number;
      available: boolean;
      purchasable: boolean;
    };

    type MallCurrency = CommerceItem & {
      balanceKnown: boolean;
    };

    type MallCatalog = {
      slotType: number;
      subSlotType: number;
      serverTime: number;
      refreshCountdown: number;
      currencies: MallCurrency[];
      goods: MallGoods[];
    };

    type MallPurchaseResult = {
      purchase: {
        goodsId: number;
        count: number;
        rewards: CommerceItem[];
        limit: PurchaseLimit | null;
      };
      catalog: MallCatalog;
    };

    type MysteryShop = {
      active: boolean;
      serverTime: number;
      activeTime?: number;
      expireTime?: number;
      npc: {
        id: number;
        reward: CommerceItem;
        stock: number;
        price: MallPrice;
        originalPrice: number;
        unitPrice: number;
        unitOriginalPrice: number;
        discountPercent: number;
      } | null;
    };

    type MysteryPurchaseResult = {
      purchase: {
        npcId: number;
        reward: CommerceItem;
        price: MallPrice;
        originalPrice: number;
        discountPercent: number;
      };
      shop: MysteryShop;
    };

    type DiamondBalance = {
      diamond: number;
    };
  }
}
