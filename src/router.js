// src/router.js — hash 模式路由（懒加载路由组件，优化首屏与分包）
import { createRouter, createWebHashHistory } from "vue-router";

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
  {
    path: "/processes",
    component: () => import("./views/ProcessManager.vue"),
  },
  { path: "/ports", redirect: "/processes" },
  {
    path: "/passwords",
    component: () => import("./views/PasswordManager.vue"),
  },
  {
    path: "/cookies",
    component: () => import("./views/CookieExtractor.vue"),
  },
  {
    path: "/uninstall",
    component: () => import("./views/AppUninstaller.vue"),
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
    path: "/api-hub",
    component: () => import("./views/ApiHub.vue"),
  },
  {
    path: "/migration",
    component: () => import("./views/Migration.vue"),
  },
  { path: "/:pathMatch(.*)*", redirect: "/dashboard" },
];

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
});
