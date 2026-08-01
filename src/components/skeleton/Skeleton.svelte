<script>
  // 基础骨架块：shimmer 动画的占位矩形
  // 用法：<Skeleton class="h-4 w-full" /> 或传 width/height
  let { class: cls = "", width = "", height = "" } = $props();
</script>

<div
  class="nx-skeleton {cls}"
  style={[width ? `width:${width}` : "", height ? `height:${height}` : ""]
    .filter(Boolean)
    .join(";")}
></div>
