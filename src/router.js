// src/router.js — hash 模式路由（懒加载路由组件，优化首屏与分包）
import { createRouter, createWebHashHistory } from "vue-router";

/** @type {import('vue-router').RouteRecordRaw[]} */
const routes = [
  { path: "/", redirect: "/dashboard" },
  {
    path: "/dashboard",
    component: () => import("./views/Dashboard.vue"),
  },
  {
    path: "/environments",
    component: () => import("./views/EnvironmentManager.vue"),
  },
  {
    path: "/software",
    component: () => import("./views/SoftwareCenter.vue"),
  },
  {
    path: "/mirrors",
    component: () => import("./views/MirrorSettings.vue"),
  },
  // 镜像源子导航：按包管理器过滤（同一页面，路由驱动）
  {
    path: "/mirrors/npm",
    component: () => import("./views/MirrorSettings.vue"),
  },
  {
    path: "/mirrors/pypi",
    component: () => import("./views/MirrorSettings.vue"),
  },
  {
    path: "/mirrors/docker",
    component: () => import("./views/MirrorSettings.vue"),
  },
  {
    path: "/mirrors/cargo",
    component: () => import("./views/MirrorSettings.vue"),
  },
  {
    path: "/processes",
    component: () => import("./views/ProcessManager.vue"),
  },
  { path: "/ports", redirect: "/processes" },
  {
    path: "/cookies",
    component: () => import("./views/CookieExtractor.vue"),
  },
  {
    path: "/uninstall",
    component: () => import("./views/AppUninstaller.vue"),
  },
  {
    path: "/tuning/linux",
    component: () => import("./views/SystemTuning.vue"),
  },
  {
    path: "/tuning/macos",
    component: () => import("./views/SystemTuning.vue"),
  },
  {
    path: "/tuning/windows",
    component: () => import("./views/SystemTuning.vue"),
  },
  {
    path: "/containers",
    component: () => import("./views/ContainerManager.vue"),
  },
  {
    path: "/settings",
    component: () => import("./views/Settings.vue"),
  },
  {
    path: "/migration",
    component: () => import("./views/Migration.vue"),
  },
  {
    path: "/ssh",
    component: () => import("./views/SSHConnections.vue"),
  },
  {
    path: "/ssh/sessions",
    component: () => import("./views/SSHTerminal.vue"),
  },
  {
    path: "/ssh/sftp",
    component: () => import("./views/SSHSftp.vue"),
  },
  {
    path: "/ssh/ai",
    component: () => import("./views/SSHAssistant.vue"),
  },
  { path: "/:pathMatch(.*)*", redirect: "/dashboard" },
];

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
});
