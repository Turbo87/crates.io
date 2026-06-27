import { expect, test } from '@/e2e/helper';

const BASE_FILES = {
  'Cargo.toml': '[package]\nname = "serde"\nversion = "1.0.0"\n',
  'src/lib.rs': '// serde crate root\npub fn answer() -> u32 { 41 }\n',
  'src/removed.rs': '// removed file\npub fn removed() {}\n',
  'src/old-name.rs': '// moved file\npub fn moved() {}\n',
  'src/unchanged.rs': '// unchanged file\npub fn stable() {}\n',
  'docs/changed.md': '# Guide\n\nOld text.\n',
  'docs-stable/a.md': '# Stable\n',
  'examples/icon.bin': `BIN${String.fromCodePoint(0)}old`,
};

const TARGET_FILES = {
  'Cargo.toml': '[package]\nname = "serde"\nversion = "1.0.1"\n',
  'src/lib.rs': '// serde crate root\npub fn answer() -> u32 { 42 }\n',
  'src/added.rs': '// added file\npub fn added() {}\n',
  'src/new-name.rs': '// moved file\npub fn moved() {}\n',
  'src/unchanged.rs': '// unchanged file\npub fn stable() {}\n',
  'docs/changed.md': '# Guide\n\nNew text.\n',
  'docs-stable/a.md': '# Stable\n',
  'examples/icon.bin': `BIN${String.fromCodePoint(0)}new`,
};

async function publishComparedVersions(msw) {
  let crate = await msw.db.crate.create({ name: 'serde' });
  await msw.db.version.create({ crate, num: '1.0.0', source_files: BASE_FILES });
  await msw.db.version.create({ crate, num: '1.0.1', source_files: TARGET_FILES });
}

test.describe('Acceptance | crate version compare', { tag: '@acceptance' }, () => {
  test('redirects to the default changed file and renders it', async ({ page, msw, percy }) => {
    await publishComparedVersions(msw);

    await page.goto('/crates/serde/compare/1.0.0/1.0.1');

    await expect(page).toHaveURL('/crates/serde/compare/1.0.0/1.0.1/src/lib.rs');
    await expect(page.getByRole('heading', { name: 'serde v1.0.1' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'serde 1.0.0 → 1.0.1' })).toBeVisible();
    await expect(page.getByText('7 changed files')).toBeVisible();
    await expect(page.getByRole('region', { name: 'File diff' })).toContainText('41');
    await expect(page.getByRole('region', { name: 'File diff' })).toContainText('42');
    await page.waitForFunction(() =>
      document
        .querySelector('[data-test-diff-viewer] diffs-container')
        ?.shadowRoot?.querySelector('span[style*="--diffs-token"]'),
    );
    await expect(page.getByRole('treeitem', { name: /lib\.rs/, selected: true })).toBeVisible();

    await percy.snapshot();
    await expect(page).toMatchAriaSnapshot({ name: 'aria.yml' });
  });

  test('redirects directories to a changed file, then falls back to the first file', async ({ page, msw }) => {
    await publishComparedVersions(msw);

    await page.goto('/crates/serde/compare/1.0.0/1.0.1/docs');
    await expect(page).toHaveURL('/crates/serde/compare/1.0.0/1.0.1/docs/changed.md');
    await expect(page.getByRole('region', { name: 'File diff' })).toContainText('Old text.');
    await expect(page.getByRole('region', { name: 'File diff' })).toContainText('New text.');

    await page.goto('/crates/serde/compare/1.0.0/1.0.1/docs-stable');
    await expect(page).toHaveURL('/crates/serde/compare/1.0.0/1.0.1/docs-stable/a.md');
    await expect(page.locator('[data-test-compare-message]')).toContainText('No changes in this file.');
  });

  test('navigates between files via the tree', async ({ page, msw }) => {
    await publishComparedVersions(msw);

    await page.goto('/crates/serde/compare/1.0.0/1.0.1/src/lib.rs');
    await page.getByRole('treeitem', { name: 'added.rs' }).click();

    await expect(page).toHaveURL('/crates/serde/compare/1.0.0/1.0.1/src/added.rs');
    await expect(page.getByRole('region', { name: 'File diff' })).toContainText('// added file');
  });

  test('renders added and deleted files as one-sided diffs', async ({ page, msw }) => {
    await publishComparedVersions(msw);

    await page.goto('/crates/serde/compare/1.0.0/1.0.1/src/added.rs');
    await expect(page.getByRole('region', { name: 'File diff' })).toContainText('// added file');
    await expect(page.getByRole('region', { name: 'File diff' })).toContainText('pub fn added()');

    await page.goto('/crates/serde/compare/1.0.0/1.0.1/src/removed.rs');
    await expect(page.getByRole('region', { name: 'File diff' })).toContainText('// removed file');
    await expect(page.getByRole('region', { name: 'File diff' })).toContainText('pub fn removed()');
  });

  test('renders renamed, unchanged, and binary states', async ({ page, msw }) => {
    await publishComparedVersions(msw);

    await page.goto('/crates/serde/compare/1.0.0/1.0.1/src/new-name.rs');
    await expect(page.locator('[data-test-compare-message]')).toContainText('File renamed without content changes.');
    await expect(page.locator('[data-test-compare-message]')).toContainText('src/old-name.rs → src/new-name.rs');

    await page.goto('/crates/serde/compare/1.0.0/1.0.1/src/unchanged.rs');
    await expect(page.locator('[data-test-compare-message]')).toContainText('No changes in this file.');

    await page.goto('/crates/serde/compare/1.0.0/1.0.1/examples/icon.bin');
    await expect(page.locator('[data-test-compare-message]')).toContainText('Binary file changed.');
  });

  test('ignores leading and trailing whitespace changes', async ({ page, msw }) => {
    let crate = await msw.db.crate.create({ name: 'serde' });
    await msw.db.version.create({
      crate,
      num: '1.0.0',
      source_files: { 'src/lib.rs': 'pub fn answer() -> u32 { 42 }\n' },
    });
    await msw.db.version.create({
      crate,
      num: '1.0.1',
      source_files: { 'src/lib.rs': '  pub fn answer() -> u32 { 42 }  \n' },
    });

    await page.goto('/crates/serde/compare/1.0.0/1.0.1/src/lib.rs');

    await expect(page.locator('[data-test-compare-message]')).toContainText('No changes in this file.');
  });

  test('shows compare-level and path errors', async ({ page, msw }) => {
    await publishComparedVersions(msw);

    await page.goto('/crates/serde/compare/1.0.0/1.0.0');
    await expect(page.locator('[data-test-compare-message]')).toContainText(
      'Choose two different versions to compare.',
    );

    await page.goto('/crates/serde/compare/1.0.0/1.0.1/src/missing.rs');
    await expect(page.locator('[data-test-load-error]')).toBeVisible();
  });

  test('shows a not-available message when either archive is missing', async ({ page, msw }) => {
    let crate = await msw.db.crate.create({ name: 'serde' });
    await msw.db.version.create({ crate, num: '1.0.0', source_files: BASE_FILES });
    await msw.db.version.create({ crate, num: '1.0.1' });

    await page.goto('/crates/serde/compare/1.0.0/1.0.1');
    await expect(page.locator('[data-test-compare-unavailable]')).toBeVisible();
  });
});
