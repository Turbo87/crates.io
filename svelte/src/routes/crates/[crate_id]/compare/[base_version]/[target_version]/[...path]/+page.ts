import { resolve } from '$app/paths';
import { redirect } from '@sveltejs/kit';

import { compareManifests, redirectTarget } from '$lib/utils/version-diff';

export async function load({ params, parent }) {
  let { baseManifest, targetManifest } = await parent();
  let selectedPath = params.path || null;

  if (params.base_version === params.target_version || !baseManifest || !targetManifest) {
    return { selectedPath, diff: null };
  }

  let diff = compareManifests(baseManifest, targetManifest);
  let target = redirectTarget(diff.files, params.path);
  if (target) {
    redirect(
      307,
      resolve('/crates/[crate_id]/compare/[base_version]/[target_version]/[...path]', {
        crate_id: params.crate_id,
        base_version: params.base_version,
        target_version: params.target_version,
        path: target,
      }),
    );
  }

  return { selectedPath, diff };
}
