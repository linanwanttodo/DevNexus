// src/lib/nav-config.js — 导航配置单一事实来源
export const navItems = [
  { id: "dashboard", route: "/dashboard", icon: "dashboard", labelKey: "nav.dashboard" },
  { id: "environments", route: "/environments", icon: "code", labelKey: "nav.environments" },
  { id: "migration", route: "/migration", icon: "swap", labelKey: "nav.migration" },
  { id: "software", route: "/software", icon: "apps", labelKey: "nav.software" },
  { id: "containers", route: "/containers", icon: "command", labelKey: "nav.containers" },
  { id: "mirrors", route: "/mirrors", icon: "sync", labelKey: "nav.mirrors",
    context: {
      titleKey: "nav.mirrors",
      items: [
        { route: "/mirrors", icon: "apps", labelKey: "mirrors.all" },
        { route: "/mirrors/npm", icon: "code", labelKey: "mirrors.npm" },
        { route: "/mirrors/pypi", icon: "code-block", labelKey: "mirrors.pypi" },
        { route: "/mirrors/docker", icon: "command", labelKey: "mirrors.docker" },
        { route: "/mirrors/cargo", icon: "tool", labelKey: "mirrors.cargo" },
      ],
    },
  },
  { id: "processes", route: "/processes", icon: "thunderbolt", labelKey: "nav.processes" },
  { id: "passwords", route: "/passwords", icon: "lock", labelKey: "nav.passwords" },
  { id: "cookies", route: "/cookies", icon: "idcard", labelKey: "nav.cookies" },
  { id: "uninstall", route: "/uninstall", icon: "delete", labelKey: "nav.uninstall" },
  { id: "system-tune", route: "/system-tune", icon: "tool", labelKey: "nav.system_tune",
    context: {
      titleKey: "nav.system_tune",
      items: [
        { route: "/system-tune", icon: "database", labelKey: "systemTune.disk" },
        { route: "/system-tune/mac", icon: "apple", labelKey: "systemTune.mac" },
        { route: "/system-tune/win", icon: "monitor", labelKey: "systemTune.win" },
      ],
    },
  },
  { id: "api-hub", route: "/api-hub", icon: "branch", labelKey: "nav.api_hub",
    context: {
      titleKey: "nav.api_hub",
      items: [
        { route: "/api-hub", icon: "bar-chart", labelKey: "apiHub.tabs.stats" },
        { route: "/api-hub/providers", icon: "database", labelKey: "apiHub.tabs.providers" },
        { route: "/api-hub/endpoints", icon: "link", labelKey: "apiHub.tabs.endpoints" },
        { route: "/api-hub/logs", icon: "history", labelKey: "apiHub.tabs.logs" },
      ],
    },
  },
  { id: "ssh", route: "/ssh", icon: "server", labelKey: "nav.ssh",
    context: {
      titleKey: "nav.ssh",
      items: [
        { route: "/ssh", icon: "list", labelKey: "ssh.connections" },
        { route: "/ssh/sessions", icon: "terminal", labelKey: "ssh.sessions" },
        { route: "/ssh/sftp", icon: "folder", labelKey: "ssh.sftp" },
      ],
    },
  },
  { id: "island", route: "/island", icon: "island", labelKey: "nav.island" },
  { id: "settings", route: "/settings", icon: "settings", labelKey: "nav.settings" },
];

/** 由当前路径推导激活的主导航与上下文子项（精确匹配，避免 /ssh* 之类前缀误命中） */
export function navForPath(path) {
  for (const nav of navItems) {
    const hit =
      path === nav.route ||
      (nav.context && nav.context.items.some((i) => i.route === path));
    if (hit) {
      const sub = nav.context
        ? nav.context.items.find((i) => i.route === path) || null
        : null;
      return { nav, sub };
    }
  }
  return { nav: null, sub: null };
}