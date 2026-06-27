<script lang="ts">
  import type { CodeViewOptions, FileDiffMetadata } from '@pierre/diffs';

  import { onMount } from 'svelte';
  import { CodeView } from '@pierre/diffs';

  import { CODE_VIEW_THEMES, getCodeViewHighlighterPool } from '$lib/utils/code-view';
  import { registerCustomExtensions } from '$lib/utils/syntax-language';

  interface Props {
    diff: { path: string; metadata: FileDiffMetadata; cacheKey: string } | null;
    colorScheme: 'light' | 'dark';
  }

  let { diff, colorScheme }: Props = $props();

  let container = $state.raw<HTMLElement>();
  let view = $state.raw<CodeView>();

  function options(): CodeViewOptions<undefined> {
    return {
      theme: CODE_VIEW_THEMES,
      themeType: colorScheme,
      diffStyle: 'unified',
      overflow: 'scroll',
      layout: {
        paddingTop: 0,
        paddingBottom: 0,
        gap: 0,
      },
    };
  }

  onMount(() => {
    registerCustomExtensions();
    view = new CodeView(options(), getCodeViewHighlighterPool());
    view.setup(container!);
    return () => view?.cleanUp();
  });

  $effect(() => view?.setOptions(options()));

  $effect(() => {
    let items = [];
    if (diff) {
      items.push({ id: diff.path, type: 'diff' as const, fileDiff: { ...diff.metadata, cacheKey: diff.cacheKey } });
    }
    view?.setItems(items);
    view?.render(true);
    view?.scrollTo({ type: 'position', position: 0, behavior: 'instant' });
  });
</script>

<div class="diff" class:hidden={diff === null} bind:this={container} data-test-diff-viewer></div>

<style>
  .hidden {
    display: none;
  }

  .diff {
    flex: 1;
    min-height: 0;
    overflow: auto;
    font-size: calc(0.85 * var(--space-s));
    background-color: light-dark(white, #141413);
  }

  .diff :global(diffs-container) {
    --diffs-font-family: var(--font-monospace);
    --diffs-header-font-family: var(--font-body);
    --diffs-light-bg: white;
    --diffs-dark-bg: #141413;
  }
</style>
