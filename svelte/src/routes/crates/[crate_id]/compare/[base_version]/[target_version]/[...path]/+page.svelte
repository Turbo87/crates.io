<script lang="ts">
  import type { FileContents, FileDiffMetadata } from '@pierre/diffs';
  import type { GitStatusEntry } from '@pierre/trees';
  import type { ManifestFile, LoadedFile } from '$lib/utils/zip-archive';
  import type { VersionDiffEntry } from '$lib/utils/version-diff';

  import { goto } from '$app/navigation';
  import { resolve } from '$app/paths';
  import { parseDiffFromFile } from '@pierre/diffs';

  import { getColorScheme } from '$lib/color-scheme.svelte';
  import CrateHeader from '$lib/components/CrateHeader.svelte';
  import DiffViewer from '$lib/components/DiffViewer.svelte';
  import FileTree from '$lib/components/FileTree.svelte';
  import { loadFile } from '$lib/utils/zip-archive';

  type FileState =
    | { kind: 'loading' }
    | { kind: 'diff'; path: string; metadata: FileDiffMetadata; cacheKey: string }
    | { kind: 'message'; message: string; detail?: string }
    | { kind: 'unavailable' }
    | { kind: 'error'; message: string };

  let { data } = $props();

  let crate = $derived(data.crate);
  let baseVersion = $derived(data.baseVersion);
  let targetVersion = $derived(data.targetVersion);
  let selectedPath = $derived(data.selectedPath);
  let diff = $derived(data.diff);
  let colorScheme = getColorScheme();

  let files = $derived(diff?.files ?? []);
  let selectedEntry = $derived(selectedPath ? files.find(file => file.path === selectedPath) : undefined);
  let gitStatus = $derived.by<readonly GitStatusEntry[]>(() => {
    return files.flatMap(file => {
      if (file.status === 'unchanged') return [];
      return [{ path: file.path, status: file.status }];
    });
  });

  let fileState = $state<FileState>({ kind: 'loading' });
  let renderedDiff = $derived(
    fileState.kind === 'diff'
      ? { path: fileState.path, metadata: fileState.metadata, cacheKey: fileState.cacheKey }
      : null,
  );

  $effect(() => {
    if (!diff) return;

    let entry = selectedEntry;
    if (!entry) {
      fileState = selectedPath
        ? { kind: 'error', message: `File "${selectedPath}" was not found in this comparison.` }
        : { kind: 'message', message: 'No files changed.' };
      return;
    }

    void showFile(entry);
  });

  async function showFile(entry: VersionDiffEntry) {
    if (entry.status === 'unchanged') {
      fileState = { kind: 'message', message: 'No changes in this file.' };
      return;
    }

    if (entry.status === 'renamed') {
      fileState = {
        kind: 'message',
        message: 'File renamed without content changes.',
        detail: `${entry.oldPath} → ${entry.newPath}`,
      };
      return;
    }

    fileState = { kind: 'loading' };
    let path = entry.path;

    try {
      let metadata;
      if (entry.status === 'modified') {
        metadata = await loadModifiedDiff(entry);
      } else if (entry.status === 'added') {
        metadata = await loadAddedDiff(entry);
      } else {
        metadata = await loadDeletedDiff(entry);
      }

      if (selectedPath !== path) return;

      if (metadata === null) {
        fileState = { kind: 'unavailable' };
      } else if (metadata === 'binary') {
        fileState = { kind: 'message', message: `Binary file ${binaryVerb(entry.status)}.` };
      } else {
        fileState = { kind: 'diff', path, metadata, cacheKey: metadata.cacheKey ?? path };
      }
    } catch (error) {
      if (selectedPath !== path) return;

      let message = error instanceof Error ? error.message : String(error);
      fileState = { kind: 'error', message };
    }
  }

  async function loadModifiedDiff(entry: VersionDiffEntry): Promise<FileDiffMetadata | 'binary' | null> {
    let baseFile = entry.baseFile!;
    let targetFile = entry.targetFile!;
    let [base, target] = await Promise.all([
      loadTextFile(baseVersion.num, baseFile),
      loadTextFile(targetVersion.num, targetFile),
    ]);
    if (base === null || target === null) return null;
    if (base.kind === 'binary' || target.kind === 'binary') return 'binary';

    return parseDiffFromFile(fileContents(baseFile, base.text), fileContents(targetFile, target.text));
  }

  async function loadAddedDiff(entry: VersionDiffEntry): Promise<FileDiffMetadata | 'binary' | null> {
    let targetFile = entry.targetFile!;
    let target = await loadTextFile(targetVersion.num, targetFile);
    if (target === null) return null;
    if (target.kind === 'binary') return 'binary';

    return parseDiffFromFile(emptyFile(targetFile), fileContents(targetFile, target.text));
  }

  async function loadDeletedDiff(entry: VersionDiffEntry): Promise<FileDiffMetadata | 'binary' | null> {
    let baseFile = entry.baseFile!;
    let base = await loadTextFile(baseVersion.num, baseFile);
    if (base === null) return null;
    if (base.kind === 'binary') return 'binary';

    return parseDiffFromFile(fileContents(baseFile, base.text), emptyFile(baseFile));
  }

  function loadTextFile(version: string, file: ManifestFile): Promise<LoadedFile | null> {
    return loadFile(fetch, data.cdnBase, crate.name, version, file);
  }

  function fileContents(file: ManifestFile, text: string): FileContents {
    return { name: file.path, contents: text, cacheKey: file.sha256 };
  }

  function emptyFile(file: ManifestFile): FileContents {
    return { name: file.path, contents: '', cacheKey: `empty:${file.sha256}` };
  }

  function binaryVerb(status: VersionDiffEntry['status']) {
    if (status === 'added') return 'added';
    if (status === 'deleted') return 'deleted';
    return 'changed';
  }

  function navigateTo(path: string) {
    let href = resolve('/crates/[crate_id]/compare/[base_version]/[target_version]/[...path]', {
      crate_id: crate.id,
      base_version: baseVersion.num,
      target_version: targetVersion.num,
      path,
    });

    void goto(href, { keepFocus: true, noScroll: true });
  }
</script>

<CrateHeader
  {crate}
  version={targetVersion}
  versionNum={targetVersion.num}
  keywords={data.keywords}
  ownersPromise={data.ownersPromise}
/>

{#if baseVersion.num === targetVersion.num}
  <div class="message" data-test-compare-message>Choose two different versions to compare.</div>
{:else if data.baseManifest === null || data.targetManifest === null}
  <div class="message" data-test-compare-unavailable>Source archive is not available for one or both versions.</div>
{:else if diff}
  <div class="compare-header">
    <h2>{crate.name} {baseVersion.num} → {targetVersion.num}</h2>
    <p>{diff.changedFileCount} {diff.changedFileCount === 1 ? 'changed file' : 'changed files'}</p>
  </div>

  <div class="viewer">
    <aside class="tree-panel" aria-label="File tree">
      <FileTree
        paths={files.map(file => file.path)}
        {selectedPath}
        onselect={navigateTo}
        colorScheme={colorScheme.resolvedScheme}
        {gitStatus}
      />
    </aside>

    <section class="diff-panel" aria-label="File diff">
      {#if fileState.kind === 'loading'}
        <div class="message" data-test-diff-loading>Loading diff…</div>
      {:else if fileState.kind === 'message'}
        <div class="message" data-test-compare-message>
          <p>{fileState.message}</p>
          {#if fileState.detail}
            <p>{fileState.detail}</p>
          {/if}
        </div>
      {:else if fileState.kind === 'unavailable'}
        <div class="message" data-test-compare-unavailable>Source archive is not available for one or both versions.</div>
      {:else if fileState.kind === 'error'}
        <div class="error" data-test-load-error>Failed to load file diff: {fileState.message}</div>
      {/if}

      <DiffViewer diff={renderedDiff} colorScheme={colorScheme.resolvedScheme} />
    </section>
  </div>
{/if}

<style>
  .compare-header {
    padding: 0 var(--space-m) var(--space-s);

    h2 {
      margin: 0;
      font-size: var(--space-m);
    }

    p {
      margin: var(--space-3xs) 0 0;
      color: var(--main-color-light);
    }
  }

  .message {
    padding: var(--space-m);
    color: var(--main-color-light);
    line-height: 1.4;

    p {
      margin: 0 0 var(--space-2xs);
    }
  }

  .error {
    padding: var(--space-s);
    color: light-dark(oklch(0.5 0.15 24), oklch(0.8 0.07 24));
  }

  .viewer {
    display: grid;
    grid-template-columns: minmax(200px, 280px) 1fr;
    gap: var(--space-s);
    height: 70vh;
    min-height: 400px;
  }

  .tree-panel,
  .diff-panel {
    background-color: light-dark(white, #141413);
    border-radius: var(--space-3xs);
    box-shadow: 0 2px 3px light-dark(hsla(51, 50%, 44%, 0.35), #232321);
    overflow: hidden;
  }

  .diff-panel {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  @media only screen and (max-width: 750px) {
    .viewer {
      grid-template-columns: 1fr;
      height: auto;
    }

    .tree-panel {
      height: 240px;
    }

    .diff-panel {
      height: 60vh;
    }
  }
</style>
