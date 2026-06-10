import { afterEach, describe, expect, it, vi } from 'vitest';

import { readmeUrl } from './readme';

function stubHost(host: string) {
  vi.stubGlobal('location', { host });
}

describe('readmeUrl', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('addresses the static CDN directly on the production host', () => {
    stubHost('crates.io');

    expect(readmeUrl('serde', '1.0.0')).toBe('https://static.crates.io/readmes/serde/serde-1.0.0.html');
  });

  it('addresses the static CDN directly on the staging host', () => {
    stubHost('staging.crates.io');

    expect(readmeUrl('serde', '1.0.0')).toBe(
      'https://static.staging.crates.io/readmes/serde/serde-1.0.0.html',
    );
  });

  it('percent-encodes `+` in build metadata to match the stored object key', () => {
    stubHost('crates.io');

    expect(readmeUrl('foo', '1.0.0+build.1')).toBe(
      'https://static.crates.io/readmes/foo/foo-1.0.0%2Bbuild.1.html',
    );
  });

  it('falls back to the API endpoint on other hosts', () => {
    stubHost('localhost:5173');

    expect(readmeUrl('serde', '1.0.0')).toBe('/api/v1/crates/serde/1.0.0/readme');
  });
});
