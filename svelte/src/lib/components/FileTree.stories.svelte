<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';

  import FileTree from './FileTree.svelte';

  const PATHS = [
    '.cargo_vcs_info.json',
    'Cargo.toml',
    'Cargo.toml.orig',
    'Cargo.lock',
    'README.md',
    'src/lib.rs',
    'src/main.rs',
    'src/core/mod.rs',
    'src/core/de.rs',
    'src/core/ser.rs',
    'src/utils/mod.rs',
    'tests/integration.rs',
  ];

  const GIT_STATUS = [
    { path: 'Cargo.lock', status: 'modified' },
    { path: 'README.md', status: 'deleted' },
    { path: 'src/main.rs', status: 'added' },
    { path: 'src/core/de.rs', status: 'renamed' },
  ] as const;

  const { Story } = defineMeta({
    title: 'FileTree',
    component: FileTree,
    tags: ['autodocs'],
    argTypes: {
      colorScheme: { control: 'inline-radio', options: ['light', 'dark'] },
    },
    args: {
      paths: PATHS,
      colorScheme: 'light',
      gitStatus: GIT_STATUS,
      onselect: fn(),
    },
  });
</script>

<script lang="ts">
  let selectedPath = $state<string | null>('src/lib.rs');
</script>

<Story name="Default">
  {#snippet template(args)}
    <div class="container">
      <FileTree
        paths={args.paths}
        {selectedPath}
        onselect={path => {
          args.onselect(path);
          selectedPath = path;
        }}
        colorScheme={args.colorScheme}
        gitStatus={args.gitStatus}
      />
    </div>
  {/snippet}
</Story>

<style>
  .container {
    height: 500px;
    width: 300px;
  }
</style>
