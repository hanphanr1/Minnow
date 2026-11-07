<script lang="ts">
  import { onDestroy, type Snippet } from 'svelte';

  let { label, side = 'bottom', children }: {
    label: string;
    side?: 'top' | 'bottom' | 'left' | 'right';
    children: Snippet;
  } = $props();

  const id = $props.id();
  let anchor: HTMLElement;
  let visible = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let disposed = false;

  function hide() {
    clearTimeout(timer);
    // Removing a focused ancestor can fire focusout during Svelte's DOM teardown.
    queueMicrotask(() => { if (!disposed) visible = false; });
    anchor?.querySelector('button')?.removeAttribute('aria-describedby');
  }

  function trigger(node: HTMLElement) {
    anchor = node;
    const show = () => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        if (disposed) return;
        visible = true;
        node.querySelector('button')?.setAttribute('aria-describedby',id);
      },400);
    };
    node.addEventListener('pointerenter',show);
    node.addEventListener('pointerleave',hide);
    node.addEventListener('focusin',show);
    node.addEventListener('focusout',hide);
    window.addEventListener('resize',hide);
    window.addEventListener('scroll',hide,true);
    return { destroy() {
      clearTimeout(timer);
      node.querySelector('button')?.removeAttribute('aria-describedby');
      node.removeEventListener('pointerenter',show);
      node.removeEventListener('pointerleave',hide);
      node.removeEventListener('focusin',show);
      node.removeEventListener('focusout',hide);
      window.removeEventListener('resize',hide);
      window.removeEventListener('scroll',hide,true);
    } };
  }

  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    const rect = anchor.getBoundingClientRect();
    const box = node.getBoundingClientRect();
    let x = rect.left+(rect.width-box.width)/2;
    let y = side === 'top' ? rect.top-box.height-7 : rect.bottom+7;
    if (side === 'left' || side === 'right') {
      x = side === 'left' ? rect.left-box.width-7 : rect.right+7;
      y = rect.top+(rect.height-box.height)/2;
    }
    node.style.left = `${Math.max(6,Math.min(innerWidth-box.width-6,x))}px`;
    node.style.top = `${Math.max(6,Math.min(innerHeight-box.height-6,y))}px`;
    return { destroy() { node.remove(); } };
  }

  onDestroy(() => { disposed = true; clearTimeout(timer); });
</script>

<span class="inline-flex" use:trigger>{@render children()}</span>
{#if visible}
  <span {id} role="tooltip" use:portal class="sb-tip-bubble">{label}</span>
{/if}

<style>
  .sb-tip-bubble {
    position: fixed;
    z-index: 10000;
    max-width: min(240px,calc(100vw - 12px));
    padding: 6px 12px;
    border-radius: var(--radius-md,8px);
    background: var(--foreground);
    color: var(--background);
    font-size: 12px;
    line-height: 1.35;
    text-align: center;
    pointer-events: none;
  }
</style>
