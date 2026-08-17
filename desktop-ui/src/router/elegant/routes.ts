/* eslint-disable */
/* prettier-ignore */
// Desktop static routes aligned with qq-farm-web (no system_admin)

import type { GeneratedRoute } from '@elegant-router/types';

export const generatedRoutes: GeneratedRoute[] = [
  {
    name: '403',
    path: '/403',
    component: 'layout.blank$view.403',
    meta: {
      title: '403',
      i18nKey: 'route.403',
      constant: true,
      hideInMenu: true
    }
  },
  {
    name: '404',
    path: '/404',
    component: 'layout.blank$view.404',
    meta: {
      title: '404',
      i18nKey: 'route.404',
      constant: true,
      hideInMenu: true
    }
  },
  {
    name: '500',
    path: '/500',
    component: 'layout.blank$view.500',
    meta: {
      title: '500',
      i18nKey: 'route.500',
      constant: true,
      hideInMenu: true
    }
  },
  {
    name: 'farm',
    path: '/farm',
    component: 'layout.base',
    meta: {
      title: 'farm',
      i18nKey: 'route.farm'
    },
    children: [
      {
        name: 'farm_account',
        path: '/farm/account',
        component: 'view.farm_account',
        meta: {
          title: 'farm_account',
          i18nKey: 'route.farm_account'
        }
      },
      {
        name: 'farm_activity',
        path: '/farm/activity',
        component: 'view.farm_activity',
        meta: {
          title: 'farm_activity',
          i18nKey: 'route.farm_activity'
        }
      },
      {
        name: 'farm_analytics',
        path: '/farm/analytics',
        component: 'view.farm_analytics',
        meta: {
          title: 'farm_analytics',
          i18nKey: 'route.farm_analytics'
        }
      },
      {
        name: 'farm_dashboard',
        path: '/farm/dashboard',
        component: 'view.farm_dashboard',
        meta: {
          title: 'farm_dashboard',
          i18nKey: 'route.farm_dashboard'
        }
      },
      {
        name: 'farm_friends',
        path: '/farm/friends',
        component: 'view.farm_friends',
        meta: {
          title: 'farm_friends',
          i18nKey: 'route.farm_friends'
        }
      },
      {
        name: 'farm_game-config',
        path: '/farm/game-config',
        component: 'view.farm_game-config',
        meta: {
          title: 'farm_game-config',
          i18nKey: 'route.farm_game-config'
        }
      },
      {
        name: 'farm_game-mall',
        path: '/farm/game-mall',
        component: 'view.farm_game-mall',
        meta: {
          title: 'farm_game-mall',
          i18nKey: 'route.farm_game-mall'
        }
      },
      {
        name: 'farm_mystery-shop',
        path: '/farm/mystery-shop',
        component: 'view.farm_mystery-shop',
        meta: {
          title: 'farm_mystery-shop',
          i18nKey: 'route.farm_mystery-shop'
        }
      },
      {
        name: 'farm_personal',
        path: '/farm/personal',
        component: 'view.farm_personal',
        meta: {
          title: 'farm_personal',
          i18nKey: 'route.farm_personal'
        }
      },
      {
        name: 'farm_settings',
        path: '/farm/settings',
        component: 'view.farm_settings',
        meta: {
          title: 'farm_settings',
          i18nKey: 'route.farm_settings'
        }
      }
    ]
  },
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
    name: 'iframe-page',
    path: '/iframe-page/:url',
    component: 'layout.base$view.iframe-page',
    props: true,
    meta: {
      title: 'iframe-page',
      i18nKey: 'route.iframe-page',
      constant: true,
      hideInMenu: true,
      keepAlive: true
    }
  },
  {
    name: 'login',
    path: '/login/:module(pwd-login|code-login|register|reset-pwd|bind-wechat)?',
    component: 'layout.blank$view.login',
    props: true,
    meta: {
      title: 'login',
      i18nKey: 'route.login',
      constant: true,
      hideInMenu: true
    }
  }
];
