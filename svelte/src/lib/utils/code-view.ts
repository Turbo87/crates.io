import type { WorkerPoolManager } from '@pierre/diffs/worker';

import { getOrCreateWorkerPoolSingleton } from '@pierre/diffs/worker';
import WorkerUrl from '@pierre/diffs/worker/worker.js?worker&url';

export const CODE_VIEW_THEMES = { light: 'github-light', dark: 'github-dark' } as const;

export function getCodeViewHighlighterPool(): WorkerPoolManager {
  return getOrCreateWorkerPoolSingleton({
    poolOptions: {
      workerFactory: () => new Worker(WorkerUrl, { type: 'module' }),
      poolSize: 1,
    },
    highlighterOptions: {
      theme: CODE_VIEW_THEMES,
      langs: ['rust', 'toml'],
    },
  });
}
