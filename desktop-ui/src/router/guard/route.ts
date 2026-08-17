import type { LocationQueryRaw, RouteLocationNormalized, RouteLocationRaw, Router } from 'vue-router';
import type { RouteKey, RoutePath } from '@elegant-router/types';
import { useRouteStore } from '@/store/modules/route';
import { getRouteName } from '@/router/elegant/transform';

/**
 * Personal free desktop: no login gate — always allow farm routes.
 */
export function createRouteGuard(router: Router) {
  router.beforeEach(async to => {
    const location = await initRoute(to);
    if (location) {
      return location;
    }
    return true;
  });
}

async function initRoute(to: RouteLocationNormalized): Promise<RouteLocationRaw | null> {
  const routeStore = useRouteStore();
  const notFoundRoute: RouteKey = 'not-found';
  const isNotFoundRoute = to.name === notFoundRoute;

  if (!routeStore.isInitConstantRoute) {
    await routeStore.initConstantRoute();
    return {
      path: to.fullPath,
      replace: true,
      query: to.query,
      hash: to.hash
    };
  }

  if (!routeStore.isInitAuthRoute) {
    await routeStore.initAuthRoute();
    if (isNotFoundRoute) {
      const rootRoute: RouteKey = 'root';
      const path = to.redirectedFrom?.name === rootRoute ? '/' : to.fullPath;
      return { path, replace: true, query: to.query, hash: to.hash };
    }
    return {
      path: to.fullPath,
      replace: true,
      query: to.query,
      hash: to.hash
    };
  }

  routeStore.onRouteSwitchWhenLoggedIn();

  if (to.name === ('login' as RouteKey)) {
    return { name: 'root' as RouteKey };
  }

  return null;
}

function getRouteQueryOfLoginRoute(_to: RouteLocationNormalized, _routeHome: RouteKey) {
  const query: LocationQueryRaw = {};
  return query;
}

// keep helper referenced for elegant-router compatibility
void getRouteName;
void getRouteQueryOfLoginRoute;
export type { RoutePath };
