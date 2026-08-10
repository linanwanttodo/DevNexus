// src/island/main.js — 灵动岛悬浮窗入口（tauri.conf.json 中 url: island.html）
import { createApp } from "vue";
import IslandApp from "./IslandApp.vue";
import "./island.css";

createApp(IslandApp).mount("#app");