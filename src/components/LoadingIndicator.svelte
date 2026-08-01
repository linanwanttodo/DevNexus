<script>
  // 统一加载指示器：居中旋转 spinner
  // padding 控制垂直留白（对应原各页面的 py-8/py-12/py-16/py-20）
  let { padding = "py-12" } = $props();
</script>

<div class="flex items-center justify-center {padding}">
  <span class="material-symbols-outlined nx-animate-spin text-nx-text-muted text-3xl">progress_activity</span>
</div>
