<script>
  // 表格骨架：表头 + 多行骨架（行内条形宽度错落，接近真实表格观感）
  import Skeleton from "./Skeleton.svelte";
  let { rows = 6, cols = 4 } = $props();
  // 行内条宽模式：交替变化模拟真实内容长度
  const barPatterns = ["w-1/3", "w-1/2", "w-2/3", "w-1/4", "w-3/5", "w-1/2", "w-2/5", "w-3/4"];
  function barWidth(row, col) {
    return barPatterns[(row * cols + col) % barPatterns.length];
  }
</script>

<div class="w-full">
  <!-- 表头 -->
  <div class="flex items-center gap-4 border-b border-nx-border pb-3">
    {#each Array(cols) as _, c}
      <Skeleton class="h-3.5 {c === 0 ? 'w-1/5' : 'w-16'}" />
    {/each}
  </div>
  <!-- 行 -->
  {#each Array(rows) as _, r}
    <div class="flex items-center gap-4 border-b border-nx-border/50 py-3">
      {#each Array(cols) as _, c}
        <Skeleton class="h-3.5 {c === 0 ? 'w-1/5' : barWidth(r, c)}" />
      {/each}
    </div>
  {/each}
</div>
