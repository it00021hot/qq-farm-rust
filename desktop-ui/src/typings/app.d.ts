/** The global namespace for the app */
declare namespace App {
  /** Theme namespace */
  namespace Theme {
    type ColorPaletteNumber = import('@sa/color').ColorPaletteNumber;

    /** NaiveUI theme overrides that can be specified in preset */
    type NaiveUIThemeOverride = import('naive-ui').GlobalThemeOverrides;

    /** Theme setting */
    interface ThemeSetting {
      /** Theme scheme */
      themeScheme: UnionKey.ThemeScheme;
      /** grayscale mode */
      grayscale: boolean;
      /** colour weakness mode */
      colourWeakness: boolean;
      /** Whether to recommend color */
      recommendColor: boolean;
      /** Theme color */
      themeColor: string;
      /** Theme radius */
      themeRadius: number;
      /** Other color */
      otherColor: OtherColor;
      /** Whether info color is followed by the primary color */
      isInfoFollowPrimary: boolean;
      /** Layout */
      layout: {
        /** Layout mode */
        mode: UnionKey.ThemeLayoutMode;
        /** Scroll mode */
        scrollMode: UnionKey.ThemeScrollMode;
      };
      /** Page */
      page: {
        /** Whether to show the page transition */
        animate: boolean;
        /** Page animate mode */
        animateMode: UnionKey.ThemePageAnimateMode;
      };
      /** Header */
      header: {
        /** Header height */
        height: number;
        /** Header breadcrumb */
        breadcrumb: {
          /** Whether to show the breadcrumb */
          visible: boolean;
          /** Whether to show the breadcrumb icon */
          showIcon: boolean;
        };
        /** Multilingual */
        multilingual: {
          /** Whether to show the multilingual */
          visible: boolean;
        };
        globalSearch: {
          /** Whether to show the GlobalSearch */
          visible: boolean;
        };
      };
      /** Tab */
      tab: {
        /** Whether to show the tab */
        visible: boolean;
        /**
         * Whether to cache the tab
         *
         * If cache, the tabs will get from the local storage when the page is refreshed
         */
        cache: boolean;
        /** Tab height */
        height: number;
        /** Tab mode */
        mode: UnionKey.ThemeTabMode;
        /** Whether to close tab by middle click */
        closeTabByMiddleClick: boolean;
      };
      /** Fixed header and tab */
      fixedHeaderAndTab: boolean;
      /** Sider */
      sider: {
        /** Inverted sider */
        inverted: boolean;
        /** Sider width */
        width: number;
        /** Collapsed sider width */
        collapsedWidth: number;
        /** Sider width when the layout is 'vertical-mix', 'top-hybrid-sidebar-first', or 'top-hybrid-header-first' */
        mixWidth: number;
        /**
         * Collapsed sider width when the layout is 'vertical-mix', 'top-hybrid-sidebar-first', or
         * 'top-hybrid-header-first'
         */
        mixCollapsedWidth: number;
        /** Child menu width when the layout is 'vertical-mix', 'top-hybrid-sidebar-first', or 'top-hybrid-header-first' */
        mixChildMenuWidth: number;
        /** Whether to auto select the first submenu */
        autoSelectFirstMenu: boolean;
      };
      /** Footer */
      footer: {
        /** Whether to show the footer */
        visible: boolean;
        /** Whether fixed the footer */
        fixed: boolean;
        /** Footer height */
        height: number;
        /**
         * Whether float the footer to the right when the layout is 'top-hybrid-sidebar-first' or
         * 'top-hybrid-header-first'
         */
        right: boolean;
      };
      /** Watermark */
      watermark: {
        /** Whether to show the watermark */
        visible: boolean;
        /** Watermark text */
        text: string;
        /** Whether to use user name as watermark text */
        enableUserName: boolean;
        /** Whether to use current time as watermark text */
        enableTime: boolean;
        /** Time format for watermark text */
        timeFormat: string;
      };
      /** define some theme settings tokens, will transform to css variables */
      tokens: {
        light: ThemeSettingToken;
        dark?: {
          [K in keyof ThemeSettingToken]?: Partial<ThemeSettingToken[K]>;
        };
      };
    }

    interface OtherColor {
      info: string;
      success: string;
      warning: string;
      error: string;
    }

    interface ThemeColor extends OtherColor {
      primary: string;
    }

    type ThemeColorKey = keyof ThemeColor;

    type ThemePaletteColor = {
      [key in ThemeColorKey | `${ThemeColorKey}-${ColorPaletteNumber}`]: string;
    };

    type BaseToken = Record<string, Record<string, string>>;

    interface ThemeSettingTokenColor {
      /** the progress bar color, if not set, will use the primary color */
      nprogress?: string;
      container: string;
      layout: string;
      inverted: string;
      'base-text': string;
    }

    interface ThemeSettingTokenBoxShadow {
      header: string;
      sider: string;
      tab: string;
    }

    interface ThemeSettingToken {
      colors: ThemeSettingTokenColor;
      boxShadow: ThemeSettingTokenBoxShadow;
    }

    type ThemeTokenColor = ThemePaletteColor & ThemeSettingTokenColor;

    /** Theme token CSS variables */
    type ThemeTokenCSSVars = {
      colors: ThemeTokenColor & { [key: string]: string };
      boxShadow: ThemeSettingTokenBoxShadow & { [key: string]: string };
    };
  }

  /** Global namespace */
  namespace Global {
    type VNode = import('vue').VNode;
    type RouteLocationNormalizedLoaded = import('vue-router').RouteLocationNormalizedLoaded;
    type RouteKey = import('@elegant-router/types').RouteKey;
    type RouteMap = import('@elegant-router/types').RouteMap;
    type RoutePath = import('@elegant-router/types').RoutePath;
    type LastLevelRouteKey = import('@elegant-router/types').LastLevelRouteKey;

    /** The router push options */
    type RouterPushOptions = {
      query?: Record<string, string>;
      params?: Record<string, string>;
      force?: boolean;
    };

    /** The global header props */
    interface HeaderProps {
      /** Whether to show the logo */
      showLogo?: boolean;
      /** Whether to show the menu toggler */
      showMenuToggler?: boolean;
      /** Whether to show the menu */
      showMenu?: boolean;
    }

    /** The global menu */
    type Menu = {
      /**
       * The menu key
       *
       * Equal to the route key
       */
      key: string;
      /** The menu label */
      label: string;
      /** The menu i18n key */
      i18nKey?: I18n.I18nKey | null;
      /** The route key */
      routeKey: RouteKey;
      /** The route path */
      routePath: RoutePath;
      /** The menu icon */
      icon?: () => VNode;
      /** The menu children */
      children?: Menu[];
    };

    type Breadcrumb = Omit<Menu, 'children'> & {
      options?: Breadcrumb[];
    };

    /** Tab route */
    type TabRoute = Pick<RouteLocationNormalizedLoaded, 'name' | 'path' | 'meta'> &
      Partial<Pick<RouteLocationNormalizedLoaded, 'fullPath' | 'query' | 'matched'>>;

    /** The global tab */
    type Tab = {
      /** The tab id */
      id: string;
      /** The tab label */
      label: string;
      /**
       * The new tab label
       *
       * If set, the tab label will be replaced by this value
       */
      newLabel?: string;
      /**
       * The old tab label
       *
       * when reset the tab label, the tab label will be replaced by this value
       */
      oldLabel?: string;
      /** The tab route key */
      routeKey: LastLevelRouteKey;
      /** The tab route path */
      routePath: RouteMap[LastLevelRouteKey];
      /** The tab route full path */
      fullPath: string;
      /** The tab fixed index */
      fixedIndex?: number | null;
      /**
       * Tab icon
       *
       * Iconify icon
       */
      icon?: string;
      /**
       * Tab local icon
       *
       * Local icon
       */
      localIcon?: string;
      /** I18n key */
      i18nKey?: I18n.I18nKey | null;
    };

    /** Form rule */
    type FormRule = import('naive-ui').FormItemRule;

    /** The global dropdown key */
    type DropdownKey = 'closeCurrent' | 'closeOther' | 'closeLeft' | 'closeRight' | 'closeAll' | 'pin' | 'unpin';
  }

  /**
   * I18n namespace
   *
   * Locales type
   */
  namespace I18n {
    type RouteKey = import('@elegant-router/types').RouteKey;

    type LangType = 'en-US' | 'zh-CN';

    type LangOption = {
      label: string;
      key: LangType;
    };

    type I18nRouteKey = Exclude<RouteKey, 'root' | 'not-found'>;

    type FormMsg = {
      required: string;
      invalid: string;
    };

    type Schema = {
      system: {
        title: string;
        updateTitle: string;
        updateContent: string;
        updateConfirm: string;
        updateCancel: string;
      };
      common: {
        action: string;
        add: string;
        addSuccess: string;
        backToHome: string;
        batchDelete: string;
        cancel: string;
        close: string;
        check: string;
        selectAll: string;
        select: string;
        expandColumn: string;
        columnSetting: string;
        config: string;
        confirm: string;
        delete: string;
        deleteSuccess: string;
        confirmDelete: string;
        edit: string;
        warning: string;
        error: string;
        index: string;
        keywordSearch: string;
        logout: string;
        logoutConfirm: string;
        lookForward: string;
        modify: string;
        modifySuccess: string;
        changePassword: string;
        oldPassword: string;
        newPassword: string;
        confirmNewPassword: string;
        oldPasswordRequired: string;
        changePasswordSuccess: string;
        noData: string;
        operate: string;
        pleaseCheckValue: string;
        refresh: string;
        reset: string;
        search: string;
        switch: string;
        tip: string;
        trigger: string;
        update: string;
        updateSuccess: string;
        userCenter: string;
        yesOrNo: {
          yes: string;
          no: string;
        };
      };
      request: {
        logout: string;
        logoutMsg: string;
        logoutWithModal: string;
        logoutWithModalMsg: string;
        refreshToken: string;
        tokenExpired: string;
      };
      theme: {
        themeDrawerTitle: string;
        tabs: {
          appearance: string;
          layout: string;
          general: string;
          preset: string;
        };
        appearance: {
          themeSchema: { title: string } & Record<UnionKey.ThemeScheme, string>;
          grayscale: string;
          colourWeakness: string;
          themeColor: {
            title: string;
            followPrimary: string;
          } & Record<Theme.ThemeColorKey, string>;
          recommendColor: string;
          recommendColorDesc: string;
          themeRadius: {
            title: string;
          };
          preset: {
            title: string;
            apply: string;
            applySuccess: string;
            [key: string]:
              | {
                  name: string;
                  desc: string;
                }
              | string;
          };
        };
        layout: {
          layoutMode: { title: string } & Record<UnionKey.ThemeLayoutMode, string> & {
              [K in `${UnionKey.ThemeLayoutMode}_detail`]: string;
            };
          tab: {
            title: string;
            visible: string;
            cache: string;
            cacheTip: string;
            height: string;
            mode: { title: string } & Record<UnionKey.ThemeTabMode, string>;
            closeByMiddleClick: string;
            closeByMiddleClickTip: string;
          };
          header: {
            title: string;
            height: string;
            breadcrumb: {
              visible: string;
              showIcon: string;
            };
          };
          sider: {
            title: string;
            inverted: string;
            width: string;
            collapsedWidth: string;
            mixWidth: string;
            mixCollapsedWidth: string;
            mixChildMenuWidth: string;
            autoSelectFirstMenu: string;
            autoSelectFirstMenuTip: string;
          };
          footer: {
            title: string;
            visible: string;
            fixed: string;
            height: string;
            right: string;
          };
          content: {
            title: string;
            scrollMode: { title: string; tip: string } & Record<UnionKey.ThemeScrollMode, string>;
            page: {
              animate: string;
              mode: { title: string } & Record<UnionKey.ThemePageAnimateMode, string>;
            };
            fixedHeaderAndTab: string;
          };
        };
        general: {
          title: string;
          watermark: {
            title: string;
            visible: string;
            text: string;
            enableUserName: string;
            enableTime: string;
            timeFormat: string;
          };
          multilingual: {
            title: string;
            visible: string;
          };
          globalSearch: {
            title: string;
            visible: string;
          };
        };
        configOperation: {
          copyConfig: string;
          copySuccessMsg: string;
          resetConfig: string;
          resetSuccessMsg: string;
        };
      };
      route: Record<I18nRouteKey, string>;
      page: {
        login: {
          common: {
            loginOrRegister: string;
            userNamePlaceholder: string;
            phonePlaceholder: string;
            codePlaceholder: string;
            passwordPlaceholder: string;
            confirmPasswordPlaceholder: string;
            codeLogin: string;
            confirm: string;
            back: string;
            validateSuccess: string;
            loginSuccess: string;
            welcomeBack: string;
          };
          pwdLogin: {
            title: string;
            rememberMe: string;
            forgetPassword: string;
            register: string;
            otherAccountLogin: string;
            otherLoginMode: string;
            superAdmin: string;
            admin: string;
            user: string;
          };
          codeLogin: {
            title: string;
            getCode: string;
            reGetCode: string;
            sendCodeSuccess: string;
            imageCodePlaceholder: string;
          };
          register: {
            title: string;
            agreement: string;
            protocol: string;
            policy: string;
          };
          resetPwd: {
            title: string;
          };
          bindWeChat: {
            title: string;
          };
        };
        system: {
          common: {
            status: {
              enable: string;
              disable: string;
              normal: string;
              deleted: string;
            };
          };
          admin: {
            title: string;
            keyword: string;
            account: string;
            password: string;
            newPassword: string;
            nickName: string;
            realName: string;
            phone: string;
            email: string;
            status: string;
            addAdmin: string;
            editAdmin: string;
            toggleStatusConfirm: string;
          };
        };
        farm: {
          common: {
            status: {
              enable: string;
              disable: string;
            };
            runStatus: {
              stopped: string;
              running: string;
              error: string;
            };
            fertilizerMode: {
              none: string;
              normal: string;
              organic: string;
              both: string;
              smart: string;
            };
            platform: {
              qq: string;
              wx: string;
            };
            accountId: string;
            selectAccount: string;
          };
          dashboard: {
            title: string;
            totalAccounts: string;
            runningAccounts: string;
            stoppedAccounts: string;
            errorAccounts: string;
            accountStatus: string;
            account: string;
            level: string;
            gold: string;
            exp: string;
            landCount: string;
            friendCount: string;
            lastError: string;
            refresh: string;
            recentEvents: string;
            eventType: string;
            eventMessage: string;
            eventTime: string;
            noEvents: string;
            nextCheckFarm: string;
            nextCheckFriend: string;
            nextCheckSteal: string;
            nextCheckHelp: string;
            nextChecksTitle: string;
            checkingNow: string;
            accountOffline: string;
            notLoggedIn: string;
            online: string;
            offline: string;
            coupon: string;
            goldBean: string;
            travelPass: string;
            expRate: string;
            todayExp: string;
            fertilizerBucket: string;
            fertNormal: string;
            fertOrganic: string;
            collectionPoints: string;
            collectionNormal: string;
            collectionRare: string;
            runningLogs: string;
            keyword: string;
            clearLogs: string;
            todayStats: string;
          };
          personal: {
            title: string;
            landsTitle: string;
            bagTitle: string;
            taskTitle: string;
            tabFarm: string;
            tabBag: string;
            tabTask: string;
            notRunning: string;
            summaryHarvestable: string;
            summaryGrowing: string;
            summaryEmpty: string;
            summaryDead: string;
            statusHarvestable: string;
            statusGrowing: string;
            statusEmpty: string;
            statusDead: string;
            statusLocked: string;
            statusStealable: string;
            statusOccupied: string;
            plantSizeBadge: string;
            seasonBadge: string;
            itemName: string;
            itemCount: string;
            itemPrice: string;
            bagCatAll: string;
            bagCatFruit: string;
            bagCatSeed: string;
            bagCatTool: string;
            bagCatOther: string;
            use: string;
            useConfirm: string;
            useSuccess: string;
            useFailed: string;
            sell: string;
            sellConfirm: string;
            sellSuccess: string;
            sellFailed: string;
            sellNotFound: string;
            batchSell: string;
            batchSellConfirm: string;
            batchSellSuccess: string;
            batchSellEmpty: string;
            dailyGiftsTitle: string;
            growthTasksTitle: string;
            giftDone: string;
            giftWaiting: string;
            giftDisabled: string;
            giftNotOpened: string;
            giftNeedLogin: string;
            giftEmpty: string;
            growthEmpty: string;
            growthDone: string;
            growthProgress: string;
            growthInProgress: string;
            growthCompleted: string;
            nextRefresh: string;
            operateSuccess: string;
            operateFailed: string;
            op: {
              all: string;
              harvest: string;
              clear: string;
              plant: string;
              upgrade: string;
            };
            confirm: {
              all: string;
              harvest: string;
              clear: string;
              plant: string;
              upgrade: string;
            };
          };
          account: {
            title: string;
            keyword: string;
            code: string;
            codePlaceholder: string;
            codeRequired: string;
            urlHint: string;
            name: string;
            namePlaceholder: string;
            platform: string;
            uin: string;
            qq: string;
            remark: string;
            runStatus: string;
            status: string;
            lastOnlineAt: string;
            addAccount: string;
            editAccount: string;
            start: string;
            stop: string;
            startConfirm: string;
            stopConfirm: string;
            deleteConfirm: string;
          };
          friends: {
            title: string;
            tabFriends: string;
            tabBlacklist: string;
            tabVisitors: string;
            gid: string;
            nickname: string;
            level: string;
            gold: string;
            friendCount: string;
            stealNum: string;
            dryNum: string;
            weedNum: string;
            insectNum: string;
            syncedAt: string;
            sync: string;
            refreshList: string;
            syncSuccess: string;
            syncSuccessWithCount: string;
            syncFailed: string;
            steal: string;
            stealAll: string;
            help: string;
            bad: string;
            addBlacklist: string;
            removeBlacklist: string;
            noAction: string;
            normalGroup: string;
            blacklistGroup: string;
            blacklistHint: string;
            blacklistEmpty: string;
            noLands: string;
            searchPlaceholder: string;
            opConfirm: string;
            blacklistConfirm: string;
            unblacklistConfirm: string;
            opSuccess: string;
            opNothing: string;
            opNoStealable: string;
            stealAllDone: string;
            stealAllEmpty: string;
            opFailed: string;
            blacklistSuccess: string;
            blacklistFailed: string;
            filterAll: string;
            filterSteal: string;
            filterHelp: string;
            filterBad: string;
            visitorsEmpty: string;
            visitorsFailed: string;
            interact: string;
            justNow: string;
            minutesAgo: string;
            today: string;
          };
          activity: {
            title: string;
            tabTravel: string;
            tabConstellation: string;
            tabShop: string;
            tabSolar: string;
            tabGreenPlum: string;
            greenPlum: string;
            greenPlumUnknown: string;
            greenPlumHint: string;
            claimGreenPlum: string;
            greenPlumBalance: string;
            greenPlumBrew: string;
            greenPlumBrewStatus: string;
            greenPlumNotStarted: string;
            greenPlumRound: string;
            greenPlumBasePrice: string;
            greenPlumSelectIngredient: string;
            greenPlumIngredientName: string;
            greenPlumOwned: string;
            greenPlumInputCount: string;
            greenPlumStartBrew: string;
            greenPlumAll: string;
            greenPlumSelectAll: string;
            greenPlumDeselectAll: string;
            greenPlumNoIngredient: string;
            greenPlumUid: string;
            greenPlumSelectedSummary: string;
            greenPlumContinueBrew: string;
            greenPlumSettleBrew: string;
            greenPlumSelling: string;
            greenPlumBrewing: string;
            greenPlumQuoteRound: string;
            greenPlumQuotePending: string;
            greenPlumQuoteUnitPrice: string;
            greenPlumReady: string;
            greenPlumClaimSeed: string;
            greenPlumSeedClaimed: string;
            greenPlumQuoteTotalGold: string;
            greenPlumSettleSuccess: string;
            activityId: string;
            name: string;
            status: string;
            claimable: string;
            claim: string;
            claimAll: string;
            claiming: string;
            claimed: string;
            claimedToday: string;
            lit: string;
            lightable: string;
            locked: string;
            notStarted: string;
            claimUnavailable: string;
            claimFailed: string;
            claimPass: string;
            claimTask: string;
            claimGift: string;
            lightConstellation: string;
            exchangeShop: string;
            exchange: string;
            exchangeConfirm: string;
            exchanging: string;
            soldOut: string;
            alreadyExchanged: string;
            notExchangeable: string;
            exchangeUnavailable: string;
            claimSolar: string;
            seasonPass: string;
            constellation: string;
            starSandShop: string;
            solarTerms: string;
            terms: string;
            goods: string;
            goodsId: string;
            goodsIdRequired: string;
            count: string;
            termId: string;
            termIdRequired: string;
            syncedAt: string;
            claimSuccess: string;
            claimRewardsSuccess: string;
            exchangeableGoods: string;
            noClaimable: string;
            noRewards: string;
            dailyClaims: string;
            passNodes: string;
            claimableNodes: string;
            shopGoods: string;
            solarClaimable: string;
            rewards: string;
            level: string;
            travelScore: string;
            travelTip: string;
            starSand: string;
            shopHint: string;
            allCategories: string;
            ended: string;
            remainingDays: string;
            remainingHours: string;
          };
          analytics: {
            title: string;
            tabStrategy: string;
            tabBlacklist: string;
            currentStrategy: string;
            currentStrategyHint: string;
            currentTag: string;
            bagPriorityHint: string;
            strategyCompare: string;
            strategyCompareHint: string;
            refLevel: string;
            availableHint: string;
            noPlantable: string;
            strategyBest: string;
            rankingsTitle: string;
            plantName: string;
            level: string;
            growTime: string;
            strategyMaxExp: string;
            strategyMaxFertExp: string;
            strategyMaxProfit: string;
            strategyMaxFertProfit: string;
            strategyLevel: string;
            strategyBagPriority: string;
            metricExp: string;
            metricProfit: string;
            metricFertExp: string;
            metricFertProfit: string;
            metricLevel: string;
            metricExpDesc: string;
            metricProfitDesc: string;
            metricFertExpDesc: string;
            metricFertProfitDesc: string;
            metricLevelDesc: string;
            stealBlacklist: string;
            stealBlacklistHint: string;
            addBlacklist: string;
            addAllBlacklist: string;
            clearBlacklist: string;
            removeBlacklist: string;
            blacklistEmpty: string;
            blacklistUpdated: string;
            statDate: string;
            gold: string;
            exp: string;
            harvestCount: string;
            stealCount: string;
            helpCount: string;
            plantCount: string;
            days: string;
          };
          settings: {
            title: string;
            save: string;
            saveSuccess: string;
            saveStrategy: string;
            saveAutomation: string;
            saveStrategySuccess: string;
            saveAutomationSuccess: string;
            strategy: string;
            automation: string;
            plantingStrategy: string;
            preferredSeedId: string;
            preferredSeedAuto: string;
            strategyPreview: string;
            strategyFallbackPreview: string;
            strategyPreviewLoading: string;
            strategyPreviewEmpty: string;
            strategyPreviewNoMatch: string;
            strategyPreviewNoPreferred: string;
            plantDelaySection: string;
            plantOrderRandom: string;
            plantDelaySeconds: string;
            stealDelaySeconds: string;
            bagSeedOrder: string;
            bagSeedOrderHint: string;
            bagSeedOrderReset: string;
            bagSeedOrderLoading: string;
            bagSeedOrderEmpty: string;
            bagSeedOrderNeedRunning: string;
            bagSeedOrderError: string;
            bagSeedStock: string;
            plantBlacklist: string;
            friendBlacklist: string;
            blacklistPlaceholder: string;
            blacklistSelectPlaceholder: string;
            friendBlacklistSelectPlaceholder: string;
            intervals: string;
            farmMin: string;
            farmMax: string;
            helpMin: string;
            helpMax: string;
            stealMin: string;
            stealMax: string;
            quietHours: string;
            quietHoursEnable: string;
            quietStart: string;
            quietEnd: string;
            farm: string;
            farmPush: string;
            landUpgrade: string;
            friend: string;
            friendSteal: string;
            friendHelp: string;
            friendHelpExpLimit: string;
            friendBad: string;
            task: string;
            sell: string;
            fertilizer: string;
            fertilizerGift: string;
            fertilizerBuy: string;
            fertilizerBuyOrganic: string;
            fertilizerBuyNormal: string;
            fertilizerBuyOrganicTitle: string;
            fertilizerBuyNormalTitle: string;
            fertilizerBuyOrganicCount: string;
            fertilizerBuyOrganicThreshold: string;
            fertilizerBuyNormalCount: string;
            fertilizerBuyNormalThreshold: string;
            fertilizerBuyCheckInterval: string;
            fertilizerBuyHint: string;
            fertilizerMultiSeason: string;
            fertilizerLandTypes: string;
            fertilizerLandTypesHint: string;
            fertilizerSmartSeconds: string;
            fertilizerSmartSecondsHint: string;
            skipOwnWeedBug: string;
            landTypePurpleGold: string;
            landTypeGold: string;
            landTypeBlack: string;
            landTypeRed: string;
            landTypeNormal: string;
            bagSeedPriority: string;
            bagSeedPriorityPlaceholder: string;
            bagSeedFallback: string;
            strategyPreferred: string;
            strategyLevel: string;
            strategyMaxExp: string;
            strategyMaxFertExp: string;
            strategyMaxProfit: string;
            strategyMaxFertProfit: string;
            strategyBagPriority: string;
          };
          gameConfig: {
            title: string;
            tabSeeds: string;
            tabFruits: string;
            tabItems: string;
            keyword: string;
            seed: string;
            fruit: string;
            item: string;
            plant: string;
            seedId: string;
            fruitId: string;
            itemId: string;
            name: string;
            requiredLevel: string;
            seasons: string;
            growTime: string;
            growPhases: string;
            phaseTemplate: string;
            harvestCount: string;
            exp: string;
            price: string;
            priceId: string;
            size: string;
            rarity: string;
            itemType: string;
            maxCount: string;
            canUse: string;
            interactionType: string;
            assetName: string;
            desc: string;
            effectDesc: string;
            addSeed: string;
            editSeed: string;
            deleteSeed: string;
            addFruit: string;
            editFruit: string;
            deleteFruit: string;
            addItem: string;
            editItem: string;
            deleteItem: string;
          };
        };
      };
      form: {
        required: string;
        userName: FormMsg;
        phone: FormMsg;
        pwd: FormMsg;
        confirmPwd: FormMsg;
        code: FormMsg;
        email: FormMsg;
      };
      dropdown: Record<Global.DropdownKey, string>;
      icon: {
        themeConfig: string;
        themeSchema: string;
        lang: string;
        fullscreen: string;
        fullscreenExit: string;
        reload: string;
        collapse: string;
        expand: string;
        pin: string;
        unpin: string;
      };
      datatable: {
        itemCount: string;
        fixed: {
          left: string;
          right: string;
          unFixed: string;
        };
      };
    };

    type GetI18nKey<T extends Record<string, unknown>, K extends keyof T = keyof T> = K extends string
      ? T[K] extends Record<string, unknown>
        ? `${K}.${GetI18nKey<T[K]>}`
        : K
      : never;

    type I18nKey = GetI18nKey<Schema>;

    type TranslateOptions<Locales extends string> = import('vue-i18n').TranslateOptions<Locales>;

    interface $T {
      (key: I18nKey): string;
      (key: I18nKey, plural: number, options?: TranslateOptions<LangType>): string;
      (key: I18nKey, defaultMsg: string, options?: TranslateOptions<I18nKey>): string;
      (key: I18nKey, list: unknown[], options?: TranslateOptions<I18nKey>): string;
      (key: I18nKey, list: unknown[], plural: number): string;
      (key: I18nKey, list: unknown[], defaultMsg: string): string;
      (key: I18nKey, named: Record<string, unknown>, options?: TranslateOptions<LangType>): string;
      (key: I18nKey, named: Record<string, unknown>, plural: number): string;
      (key: I18nKey, named: Record<string, unknown>, defaultMsg: string): string;
    }
  }

  /** Service namespace */
  namespace Service {
    /** Other baseURL key */
    type OtherBaseURLKey = 'demo';

    interface ServiceConfigItem {
      /** The backend service base url */
      baseURL: string;
      /** The proxy pattern of the backend service base url */
      proxyPattern: string;
    }

    interface OtherServiceConfigItem extends ServiceConfigItem {
      key: OtherBaseURLKey;
    }

    /** The backend service config */
    interface ServiceConfig extends ServiceConfigItem {
      /** Other backend service config */
      other: OtherServiceConfigItem[];
    }

    interface SimpleServiceConfig extends Pick<ServiceConfigItem, 'baseURL'> {
      other: Record<OtherBaseURLKey, string>;
    }

    /** The backend service response data */
    type Response<T = unknown> = {
      /** The backend service response code */
      code: string;
      /** The backend service response message */
      msg: string;
      /** The backend service response data */
      data: T;
    };

    /** The demo backend service response data */
    type DemoResponse<T = unknown> = {
      /** The backend service response code */
      status: string;
      /** The backend service response message */
      message: string;
      /** The backend service response data */
      result: T;
    };
  }
}
