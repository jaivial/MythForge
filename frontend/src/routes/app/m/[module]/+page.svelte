<script lang="ts">
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { workspace } from '$lib/api/endpoints';

  // Landing on a module: go to its first entity. A module with no entities (or a
  // module slug that does not exist) must not loop or silently fall back to
  // another module's entity: it explains itself and offers a way back.
  let error = $state('');
  let loading = $state(true);

  $effect(() => {
    const mod = page.params.module;
    if (!mod) return;
    loading = true;
    workspace
      .blueprint()
      .then((bp) => {
        const m = bp.modules.find((x) => x.slug === mod);
        const first = m?.entities[0]?.slug;
        if (m && first) {
          goto(`/app/m/${mod}/${first}`, { replaceState: true, noScroll: false });
        } else {
          error = m ? 'This module has no entities yet.' : 'Module not found.';
          loading = false;
        }
      })
      .catch(() => {
        error = 'Load failed';
        loading = false;
      });
  });
</script>

{#if error}
  <p data-testid="module-error" class="text-sm text-danger">{error}</p>
  <a href="/app" class="text-sm underline text-silver">Back to workspace</a>
{:else if loading}
  <p class="text-sm text-muted-foreground">Loading…</p>
{/if}
