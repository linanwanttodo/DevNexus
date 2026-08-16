// 校验 Arco 图标名 → @lucide/vue 组件名映射是否存在，输出缺失项。
// 直接引用 src/lib/icon-map.js（单一数据源），避免手抄快照漂移。
import * as Icons from "@lucide/vue";
import { iconMap, FALLBACK_ICON } from "../src/lib/icon-map.js";

const missing = [];
for (const [arco, lucide] of Object.entries(iconMap)) {
  if (!Icons[lucide]) missing.push(`${arco} -> ${lucide}`);
}
if (!Icons[FALLBACK_ICON]) missing.push(`(fallback) -> ${FALLBACK_ICON}`);

console.log(`total mappings: ${Object.keys(iconMap).length}`);
console.log(
  missing.length === 0 ? "ALL EXIST" : "MISSING:\n" + missing.join("\n")
);
