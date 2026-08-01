<script>
  // 卡片网格骨架：模拟软件中心等卡片网格布局
  import Skeleton from "./Skeleton.svelte";
  let { count = 8 } = $props();
</script>

<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
  {#each Array(count) as _, i}
    <div class="nx-card p-4 space-y-3">
      <div class="flex items-center gap-3">
        <Skeleton class="h-10 w-10 rounded-lg shrink-0" />
        <div class="flex-1 space-y-2">
          <Skeleton class="h-3.5 w-2/3" />
          <Skeleton class="h-3 w-1/2" />
        </div>
      </div>
      <Skeleton class="h-3 w-4/5" />
      <Skeleton class="h-3 w-3/5" />
      <div class="flex gap-2 pt-1">
        <Skeleton class="h-7 w-16 rounded-md" />
        <Skeleton class="h-7 w-16 rounded-md" />
      </div>
    </div>
  {/each}
</div>
