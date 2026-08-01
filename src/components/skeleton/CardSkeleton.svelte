<script>
  // 单卡片骨架：模拟 Dashboard 等单卡片信息区
  import Skeleton from "./Skeleton.svelte";
  let { lines = 4 } = $props();
  const patterns = ["w-1/3", "w-1/2", "w-2/3", "w-1/4", "w-3/5"];
</script>

<div class="nx-card p-4 space-y-3">
  <div class="flex items-center gap-2">
    <Skeleton class="h-4 w-4 rounded" />
    <Skeleton class="h-3.5 w-24" />
  </div>
  {#each Array(lines) as _, i}
    <Skeleton class="h-3.5 {patterns[i % patterns.length]}" />
  {/each}
  <div class="flex items-center gap-4 pt-1">
    <Skeleton class="h-8 w-20 rounded-md" />
    <Skeleton class="h-8 w-20 rounded-md" />
  </div>
</div>
