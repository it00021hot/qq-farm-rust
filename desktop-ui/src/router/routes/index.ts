import type { ElegantConstRoute, ElegantRoute } from '@elegant-router/types';
import { generatedRoutes } from '../elegant/routes';
import { layouts, views } from '../elegant/imports';
import { transformElegantRoutesToVueRoutes } from '../elegant/transform';

/**
 * Farm-only static routes for personal free desktop (no login / no user admin).
 * Parents use hideInMenu so sidebar shows a flat farm list.
 */
const flatAuthRoutes: ElegantRoute[] = [
  {
    name: 'home',
    path: '/home',
    component: 'layout.base$view.home',
    meta: {
      title: 'home',
      i18nKey: 'route.home',
      icon: 'mdi:monitor-dashboard',
      order: 1
    }
  },
  {
    name: 'farm',
    path: '/farm',
    component: 'layout.base',
    meta: {
      title: 'farm',
      i18nKey: 'route.farm',
      hideInMenu: true,
      order: 2
    },
    children: [
      {
        name: 'farm_personal',
        path: '/farm/personal',
        component: 'view.farm_personal',
        meta: {
          title: 'farm_personal',
          i18nKey: 'route.farm_personal',
          icon: 'mdi:sprout',
          order: 2
        }
      },
      {
        name: 'farm_friends',
        path: '/farm/friends',
        component: 'view.farm_friends',
        meta: {
          title: 'farm_friends',
          i18nKey: 'route.farm_friends',
          icon: 'mdi:account-group',
          order: 3
        }
      },
      {
        name: 'farm_activity',
        path: '/farm/activity',
        component: 'view.farm_activity',
        meta: {
          title: 'farm_activity',
          i18nKey: 'route.farm_activity',
          icon: 'mdi:calendar-star',
          order: 4
        }
      },
      {
        name: 'farm_analytics',
        path: '/farm/analytics',
        component: 'view.farm_analytics',
        meta: {
          title: 'farm_analytics',
          i18nKey: 'route.farm_analytics',
          icon: 'mdi:chart-line',
          order: 7
        }
      },
      {
        name: 'farm_solar-calendar',
        path: '/farm/solar-calendar',
        component: 'view.farm_solar-calendar',
        meta: {
          title: 'farm_solar-calendar',
          i18nKey: 'route.farm_solar-calendar',
          icon: 'mdi:calendar-month',
          order: 5,
          hideInMenu: true,
          activeMenu: 'farm_activity'
        }
      },
      {
        name: 'farm_milk-tea',
        path: '/farm/milk-tea',
        component: 'view.farm_milk-tea',
        meta: {
          title: 'farm_milk-tea',
          i18nKey: 'route.farm_milk-tea',
          icon: 'mdi:cup-outline',
          order: 6,
          hideInMenu: true,
          activeMenu: 'farm_activity'
        }
      },
      {
        name: 'farm_share-reward',
        path: '/farm/share-reward',
        component: 'view.farm_share-reward',
        meta: {
          title: 'farm_share-reward',
          i18nKey: 'route.farm_share-reward',
          icon: 'mdi:gift-outline',
          order: 7,
          hideInMenu: true,
          activeMenu: 'farm_activity'
        }
      },
      {
        name: 'farm_game-mall',
        path: '/farm/game-mall',
        component: 'view.farm_game-mall',
        meta: {
          title: 'farm_game-mall',
          i18nKey: 'route.farm_game-mall',
          icon: 'mdi:storefront',
          order: 8
        }
      },
      {
        // 神秘商人已并入游戏商城 tab（对齐 bot d5a50cc）；旧路径重定向，不再进菜单
        name: 'farm_mystery-shop',
        path: '/farm/mystery-shop',
        redirect: '/farm/game-mall?tab=mystery',
        meta: {
          title: 'farm_mystery-shop',
          i18nKey: 'route.farm_mystery-shop',
          hideInMenu: true
        }
      },
      {
        name: 'farm_settings',
        path: '/farm/settings',
        component: 'view.farm_settings',
        meta: {
          title: 'farm_settings',
          i18nKey: 'route.farm_settings',
          icon: 'mdi:cog',
          order: 10
        }
      },
      {
        name: 'farm_game-config',
        path: '/farm/game-config',
        component: 'view.farm_game-config',
        meta: {
          title: 'farm_game-config',
          i18nKey: 'route.farm_game-config',
          icon: 'mdi:leaf',
          order: 11
        }
      },
      {
        name: 'farm_account',
        path: '/farm/account',
        component: 'view.farm_account',
        meta: {
          title: 'farm_account',
          i18nKey: 'route.farm_account',
          icon: 'mdi:account-cowboy-hat',
          order: 12
        }
      }
    ]
  }
] as ElegantRoute[];

export function createStaticRoutes() {
  const constantRoutes: ElegantRoute[] = [];

  [...generatedRoutes].forEach(item => {
    if (item.meta?.constant) {
      if (item.name === 'home') return;
      constantRoutes.push(item);
    }
  });

  return {
    constantRoutes,
    authRoutes: flatAuthRoutes
  };
}

export function getAuthVueRoutes(routes: ElegantConstRoute[]) {
  return transformElegantRoutesToVueRoutes(routes, layouts, views);
}
