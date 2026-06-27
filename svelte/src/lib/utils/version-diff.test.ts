import type { ManifestFile } from './zip-archive';

import { describe, expect, it } from 'vitest';

import { compareManifests, redirectTarget } from './version-diff';

function file(path: string, sha256 = path): ManifestFile {
  return {
    path,
    data_offset: 0,
    compressed_size: 0,
    uncompressed_size: 0,
    compression: 'deflate',
    sha256,
  };
}

describe('compareManifests()', () => {
  it('classifies unchanged, modified, added, and deleted files', () => {
    let diff = compareManifests(
      { files: [file('Cargo.toml', 'same'), file('src/lib.rs', 'old'), file('README.md', 'removed')] },
      { files: [file('Cargo.toml', 'same'), file('src/lib.rs', 'new'), file('src/main.rs', 'added')] },
    );

    expect(diff.changedFileCount).toBe(3);
    expect(diff.files.map(entry => [entry.path, entry.status])).toEqual([
      ['Cargo.toml', 'unchanged'],
      ['README.md', 'deleted'],
      ['src/lib.rs', 'modified'],
      ['src/main.rs', 'added'],
    ]);
  });

  it('detects exact-content renames by SHA-256', () => {
    let diff = compareManifests(
      { files: [file('src/old.rs', 'content'), file('src/changed_old.rs', 'before')] },
      { files: [file('src/new.rs', 'content'), file('src/changed_new.rs', 'after')] },
    );

    expect(diff.files.map(entry => [entry.path, entry.status, entry.oldPath, entry.newPath])).toEqual([
      ['src/changed_new.rs', 'added', undefined, undefined],
      ['src/changed_old.rs', 'deleted', undefined, undefined],
      ['src/new.rs', 'renamed', 'src/old.rs', 'src/new.rs'],
    ]);
  });

  it('keeps rename-plus-edit as deleted plus added', () => {
    let diff = compareManifests({ files: [file('src/old.rs', 'before')] }, { files: [file('src/new.rs', 'after')] });

    expect(diff.files.map(entry => [entry.path, entry.status])).toEqual([
      ['src/new.rs', 'added'],
      ['src/old.rs', 'deleted'],
    ]);
  });
});

describe('redirectTarget()', () => {
  it('returns undefined when the path is already a file', () => {
    let diff = compareManifests({ files: [file('Cargo.toml', 'a')] }, { files: [file('Cargo.toml', 'b')] });

    expect(redirectTarget(diff.files, 'Cargo.toml')).toBeUndefined();
  });

  it('prefers changed src/lib.rs, then src/main.rs, then Cargo.toml by default', () => {
    expect(
      redirectTarget(
        compareManifests(
          { files: [file('Cargo.toml', 'a'), file('src/main.rs', 'a'), file('src/lib.rs', 'a')] },
          { files: [file('Cargo.toml', 'b'), file('src/main.rs', 'b'), file('src/lib.rs', 'b')] },
        ).files,
        '',
      ),
    ).toBe('src/lib.rs');

    expect(
      redirectTarget(
        compareManifests(
          { files: [file('Cargo.toml', 'a'), file('src/main.rs', 'a')] },
          { files: [file('Cargo.toml', 'b'), file('src/main.rs', 'b')] },
        ).files,
        '',
      ),
    ).toBe('src/main.rs');

    expect(
      redirectTarget(
        compareManifests({ files: [file('Cargo.toml', 'a')] }, { files: [file('Cargo.toml', 'b')] }).files,
        '',
      ),
    ).toBe('Cargo.toml');
  });

  it('falls back to the first changed file by path', () => {
    let diff = compareManifests(
      { files: [file('README.md', 'a'), file('src/util.rs', 'same')] },
      { files: [file('README.md', 'b'), file('src/util.rs', 'same')] },
    );

    expect(redirectTarget(diff.files, '')).toBe('README.md');
  });

  it('returns undefined when there are no changed files', () => {
    let diff = compareManifests({ files: [file('Cargo.toml', 'same')] }, { files: [file('Cargo.toml', 'same')] });

    expect(redirectTarget(diff.files, '')).toBeUndefined();
  });

  it('redirects directories to the first changed file inside them', () => {
    let diff = compareManifests(
      { files: [file('src/a.rs', 'same'), file('src/b.rs', 'old'), file('src/c.rs', 'old')] },
      { files: [file('src/a.rs', 'same'), file('src/b.rs', 'new'), file('src/c.rs', 'new')] },
    );

    expect(redirectTarget(diff.files, 'src')).toBe('src/b.rs');
  });

  it('falls back to the first file inside a directory when it has no changed files', () => {
    let diff = compareManifests(
      { files: [file('docs/a.md', 'same'), file('docs/b.md', 'same'), file('src/lib.rs', 'old')] },
      { files: [file('docs/a.md', 'same'), file('docs/b.md', 'same'), file('src/lib.rs', 'new')] },
    );

    expect(redirectTarget(diff.files, 'docs')).toBe('docs/a.md');
  });

  it('returns undefined for a path that is neither a file nor a directory', () => {
    let diff = compareManifests({ files: [file('src/lib.rs', 'a')] }, { files: [file('src/lib.rs', 'b')] });

    expect(redirectTarget(diff.files, 'does/not/exist')).toBeUndefined();
  });
});
