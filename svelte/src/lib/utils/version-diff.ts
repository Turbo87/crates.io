import type { Manifest, ManifestFile } from './zip-archive';

export interface CrateVersionRef {
  crateName: string;
  version: string;
}

export type VersionDiffStatus = 'unchanged' | 'modified' | 'added' | 'deleted' | 'renamed';

export interface VersionDiffEntry {
  path: string;
  status: VersionDiffStatus;
  baseFile?: ManifestFile;
  targetFile?: ManifestFile;
  oldPath?: string;
  newPath?: string;
}

export interface VersionDiff {
  files: VersionDiffEntry[];
  changedFileCount: number;
}

const DEFAULT_FILE_PRIORITY = ['src/lib.rs', 'src/main.rs', 'Cargo.toml'];

export function compareManifests(base: Manifest, target: Manifest): VersionDiff {
  let baseByPath = new Map(base.files.map(file => [file.path, file]));
  let targetByPath = new Map(target.files.map(file => [file.path, file]));

  let files: VersionDiffEntry[] = [];
  let deleted: ManifestFile[] = [];
  let added: ManifestFile[] = [];

  for (let baseFile of base.files) {
    let targetFile = targetByPath.get(baseFile.path);
    if (targetFile) {
      files.push({
        path: baseFile.path,
        status: baseFile.sha256 === targetFile.sha256 ? 'unchanged' : 'modified',
        baseFile,
        targetFile,
      });
    } else {
      deleted.push(baseFile);
    }
  }

  for (let targetFile of target.files) {
    if (!baseByPath.has(targetFile.path)) {
      added.push(targetFile);
    }
  }

  let unmatchedAdded = [...added].sort((a, b) => comparePaths(a.path, b.path));
  for (let baseFile of deleted.sort((a, b) => comparePaths(a.path, b.path))) {
    let renameIndex = unmatchedAdded.findIndex(targetFile => targetFile.sha256 === baseFile.sha256);
    if (renameIndex === -1) {
      files.push({ path: baseFile.path, status: 'deleted', baseFile });
      continue;
    }

    let targetFile = unmatchedAdded.splice(renameIndex, 1)[0]!;
    files.push({
      path: targetFile.path,
      status: 'renamed',
      baseFile,
      targetFile,
      oldPath: baseFile.path,
      newPath: targetFile.path,
    });
  }

  for (let targetFile of unmatchedAdded) {
    files.push({ path: targetFile.path, status: 'added', targetFile });
  }

  files.sort((a, b) => comparePaths(a.path, b.path));

  return {
    files,
    changedFileCount: files.filter(file => file.status !== 'unchanged').length,
  };
}

export function redirectTarget(files: VersionDiffEntry[], path: string | undefined): string | undefined {
  if (!path) {
    return defaultFile(files)?.path;
  }

  if (files.some(file => file.path === path)) {
    return undefined;
  }

  return (
    firstFileInDirectory(files, path, file => file.status !== 'unchanged')?.path ??
    firstFileInDirectory(files, path)?.path
  );
}

function defaultFile(files: VersionDiffEntry[]): VersionDiffEntry | undefined {
  for (let path of DEFAULT_FILE_PRIORITY) {
    let match = files.find(file => file.path === path && file.status !== 'unchanged');
    if (match) {
      return match;
    }
  }

  return files.find(file => file.status !== 'unchanged');
}

function firstFileInDirectory(
  files: VersionDiffEntry[],
  dirPath: string,
  predicate: (file: VersionDiffEntry) => boolean = () => true,
): VersionDiffEntry | undefined {
  let prefix = dirPath.endsWith('/') ? dirPath : `${dirPath}/`;
  return files.find(file => file.path.startsWith(prefix) && predicate(file));
}

function comparePaths(a: string, b: string) {
  return a.localeCompare(b, undefined, { sensitivity: 'base' }) || a.localeCompare(b);
}
