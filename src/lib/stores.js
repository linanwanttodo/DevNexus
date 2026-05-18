let _route = "/dashboard";
let _routeListeners = [];

export function getRoute() {
  return _route;
}

export function navigate(route) {
  console.log('[navigate]', route);
  _route = route;
  for (const fn of _routeListeners) {
    try { fn(route); } catch (e) { console.error(e); }
  }
}

export function onRouteChange(fn) {
  _routeListeners.push(fn);
  try { fn(_route); } catch (e) { console.error(e); }
  return () => {
    _routeListeners = _routeListeners.filter(f => f !== fn);
  };
}

// ===== 全局搜索 =====
let _searchQuery = "";
let _searchListeners = [];

export function getSearchQuery() {
  return _searchQuery;
}

export function setSearchQuery(query) {
  _searchQuery = query;
  for (const fn of _searchListeners) {
    try { fn(query); } catch (e) { console.error(e); }
  }
}

export function onSearchChange(fn) {
  _searchListeners.push(fn);
  return () => {
    _searchListeners = _searchListeners.filter(f => f !== fn);
  };
}
