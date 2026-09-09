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
  { id: "cookies", route: "/cookies", icon: "idcard", labelKey: "nav.cookies" },
  { id: "uninstall", route: "/uninstall", icon: "delete", labelKey: "nav.uninstall" },
  { id: "tuning", route: "/tuning/linux", icon: "tool", labelKey: "nav.system_tune",
    context: {
      titleKey: "nav.system_tune",
      items: [
        { route: "/tuning/linux", icon: "terminal", labelKey: "tuningLinux.nav" },
        { route: "/tuning/macos", icon: "apple", labelKey: "systemTune.mac" },
        { route: "/tuning/windows", icon: "monitor", labelKey: "systemTune.win" },
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
        { route: "/ssh/ai", icon: "sparkles", labelKey: "nav.sshAssistant" },
      ],
    },
  },
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