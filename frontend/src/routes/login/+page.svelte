<script lang="ts">
  import { goto } from '$app/navigation';
  import { Button, Card, Input, Separator, Spinner } from '$lib/components/ui';
  import { auth } from '$lib/api/endpoints';
  import { setSession } from '$lib/stores/session';
  import type { Session } from '$lib/api/types';

  let mode = $state<'login' | 'signup'>('signup');
  let email = $state('');
  let password = $state('');
  let name = $state('');
  let company = $state('');
  let template = $state('crm');
  let busy = $state(false);
  let error = $state('');

  const templates = [
    { id: 'blank', label: 'Blank' },
    { id: 'crm', label: 'CRM' },
    { id: 'erp', label: 'ERP' },
    { id: 'services', label: 'Services' },
    { id: 'retail', label: 'Retail' }
  ];

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    error = '';
    busy = true;
    try {
      let s: Session;
      if (mode === 'signup') {
        s = await auth.signup(email, password, name, company, template);
      } else {
        const r = await auth.login(email, password);
        s = { token: r.token, user: r.user, company_id: r.company_id };
      }
      setSession(s);
      goto('/app');
    } catch (err) {
      error = err instanceof Error ? err.message : 'Unexpected error';
    } finally {
      busy = false;
    }
  }
</script>

<main class="flex min-h-screen items-center justify-center p-6">
  <div class="flex w-full max-w-md flex-col gap-6">
    <header class="flex flex-col gap-1">
      <div class="flex items-center gap-3">
        <img src="/favicon.svg" alt="" class="size-8" />
        <h1 class="text-2xl font-semibold tracking-tight">MythForge</h1>
      </div>
      <p class="text-sm text-muted-foreground">
        Build your ERP &amp; CRM by prompting. No code, no restarts.
      </p>
    </header>

    <Card class="p-6">
      <form onsubmit={submit} class="flex flex-col gap-4" data-testid="login-form">
        <div class="flex gap-2" role="tablist" aria-label="Mode">
          <Button type="button" size="sm" variant={mode === 'signup' ? 'default' : 'ghost'}
            onclick={() => (mode = 'signup')}>Create workspace</Button>
          <Button type="button" size="sm" variant={mode === 'login' ? 'default' : 'ghost'}
            onclick={() => (mode = 'login')}>Sign in</Button>
        </div>
        <Separator />

        {#if mode === 'signup'}
          <label class="flex flex-col gap-1 text-sm">
            <span class="text-muted-foreground">Your name</span>
            <Input data-testid="name" bind:value={name} placeholder="Ada Lovelace" required />
          </label>
          <label class="flex flex-col gap-1 text-sm">
            <span class="text-muted-foreground">Company</span>
            <Input data-testid="company" bind:value={company} placeholder="Acme Trading" required />
          </label>
          <div class="flex flex-col gap-1 text-sm">
            <span class="text-muted-foreground">Starting template</span>
            <div class="flex flex-wrap gap-2" data-testid="templates">
              {#each templates as t}
                <Button type="button" size="sm" variant={template === t.id ? 'default' : 'outline'}
                  onclick={() => (template = t.id)}>{t.label}</Button>
              {/each}
            </div>
          </div>
        {/if}

        <label class="flex flex-col gap-1 text-sm">
          <span class="text-muted-foreground">Email</span>
          <Input data-testid="email" type="email" bind:value={email} placeholder="you@company.com" required />
        </label>
        <label class="flex flex-col gap-1 text-sm">
          <span class="text-muted-foreground">Password</span>
          <Input data-testid="password" type="password" bind:value={password}
            placeholder="at least 8 characters" required minlength={8} />
        </label>

        {#if error}
          <p data-testid="login-error" class="text-sm text-danger">{error}</p>
        {/if}

        <Button type="submit" disabled={busy} data-testid="submit">
          {#if busy}<Spinner size={14} />{/if}
          {mode === 'signup' ? 'Create workspace' : 'Sign in'}
        </Button>
      </form>
    </Card>
  </div>
</main>
