<script lang="ts">
  import { page } from '$app/state';
  import { goto } from '$app/navigation';

  // A module with several entities: land on its first entity's table.
  $effect(() => {
    const mod = page.params.module;
    goto(`/app/m/${mod}`, { replaceState: true });
  });
</script>
