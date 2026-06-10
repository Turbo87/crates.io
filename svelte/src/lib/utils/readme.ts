/**
 * Maps the known crates.io deployment hosts to the base URL of their static
 * asset CDN. Returns `null` for any other host (local development, PR preview
 * deployments, …), where the README is instead loaded via the API endpoint
 * that redirects to the active storage backend.
 */
function staticCdnBase(host: string): string | null {
  if (host === 'crates.io') return 'https://static.crates.io';
  if (host === 'staging.crates.io') return 'https://static.staging.crates.io';
  return null;
}

/**
 * Builds the URL the README HTML is fetched from.
 *
 * On the production and staging deployments we can address the rendered README
 * directly on the static CDN, skipping the `/api/v1/crates/{name}/{version}/readme`
 * endpoint and the extra HTTP redirect it issues. The path mirrors `readme_path`
 * in the backend's `storage.rs`. Everywhere else we fall back to the API
 * endpoint, which resolves the storage backend server-side.
 */
export function readmeUrl(crateName: string, versionNum: string): string {
  let base = staticCdnBase(globalThis.location.host);
  if (base === null) {
    return `/api/v1/crates/${crateName}/${versionNum}/readme`;
  }

  // The backend percent-encodes `+` (which can appear in semver build metadata)
  // when uploading, so mirror that here to match the stored object key.
  let path = `readmes/${crateName}/${crateName}-${versionNum}.html`.replaceAll('+', '%2B');
  return `${base}/${path}`;
}

/**
 * Loads the README HTML for a crate version.
 *
 * @returns The README HTML string, or `null` if no README exists.
 * @throws Error If the request fails with a non-404/403 status.
 */
export async function loadReadme(
  fetch: typeof globalThis.fetch,
  crateName: string,
  versionNum: string,
): Promise<string | null> {
  let response = await fetch(readmeUrl(crateName, versionNum));

  // 404/403 means no README (not an error)
  if (response.status === 404 || response.status === 403) {
    return null;
  }

  if (!response.ok) {
    throw new Error('Failed to load README');
  }

  return await response.text();
}
