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
          icon: 'mdi:star-circle',
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
          order: 5
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
          order: 6
        }
      },
      {
        name: 'farm_mystery-shop',
        path: '/farm/mystery-shop',
        component: 'view.farm_mystery-shop',
        meta: {
          title: 'farm_mystery-shop',
          i18nKey: 'route.farm_mystery-shop',
          icon: 'mdi:account-question',
          order: 7
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
          order: 8
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
          order: 9
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
          order: 10
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
