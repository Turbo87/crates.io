import { createClient } from '@crates-io/api-client';
import { error } from '@sveltejs/kit';

import { cdnBase } from '$lib/utils/cdn';
import { loadSiteMetadata } from '$lib/utils/site-metadata';
import { loadManifest } from '$lib/utils/zip-archive';

export async function load({ fetch, params }) {
  let client = createClient({ fetch });
  let { data } = await loadSiteMetadata(client);
  if (!data) {
    throw new Error('Failed to load site metadata');
  }

  let base = cdnBase(data);
  let [baseVersion, targetVersion, baseManifest, targetManifest] = await Promise.all([
    loadVersion(fetch, params.crate_id, params.base_version),
    loadVersion(fetch, params.crate_id, params.target_version),
    loadManifest(fetch, base, params.crate_id, params.base_version),
    loadManifest(fetch, base, params.crate_id, params.target_version),
  ]);

  return { cdnBase: base, baseVersion, targetVersion, baseManifest, targetManifest };
}

async function loadVersion(fetch: typeof globalThis.fetch, name: string, version: string) {
  let client = createClient({ fetch });
  let response;
  try {
    response = await client.GET('/api/v1/crates/{name}/{version}', { params: { path: { name, version } } });
  } catch {
    loadVersionError(name, version, 504);
  }

  let status = response.response.status;
  if (response.error) {
    loadVersionError(name, version, status);
  }

  return response.data.version;
}

function loadVersionError(name: string, version: string, status: number): never {
  if (status === 404) {
    error(404, { message: `${name}: Version ${version} not found` });
  } else {
    error(status, { message: `${name}: Failed to load version data`, tryAgain: true });
  }
}
