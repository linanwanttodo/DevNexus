let initialHash = (typeof window !== 'undefined' ? window.location.hash.slice(1) : "") || "/dashboard";
let route = $state(initialHash);
let searchQuery = $state("");

export function getRoute() {
  return route;
}

export function navigate(r) {
  route = r;
  if (typeof window !== 'undefined') {
    window.location.hash = r;
  }
}

if (typeof window !== 'undefined') {
  window.addEventListener('hashchange', () => {
    const hash = window.location.hash.slice(1) || "/dashboard";
    if (hash !== route) {
      route = hash;
    }
  });
}

export function getSearchQuery() {
  return searchQuery;
}

export function setSearchQuery(q) {
  searchQuery = q;
}
