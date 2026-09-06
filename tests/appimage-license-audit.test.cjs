const test = require('node:test');
const assert = require('node:assert/strict');

test('license audit parses package owners and architecture suffixes', async () => {
  const { parseDpkgOwners } = await import('../scripts/audit-appimage-licenses.mjs');
  assert.deepEqual(parseDpkgOwners(
    'libgtk-3-0:amd64: /usr/lib/x86_64-linux-gnu/libgtk-3.so.0\ninvalid\n'
  ), [{ packageName: 'libgtk-3-0', path: '/usr/lib/x86_64-linux-gnu/libgtk-3.so.0' }]);
});

test('license audit extracts an ELF build ID', async () => {
  const { extractBuildId } = await import('../scripts/audit-appimage-licenses.mjs');
  assert.equal(extractBuildId('Build ID: A1b2C3\n'), 'a1b2c3');
  assert.equal(extractBuildId('no build id'), null);
});

test('license audit reports libraries without copyright notices', async () => {
  const { missingNoticePlan } = await import('../scripts/audit-appimage-licenses.mjs');
  const entries = [
    { relativeLibrary: 'usr/lib/libok.so', packages: ['libok'], buildId: 'abc' },
    { relativeLibrary: 'usr/lib/libmissing.so', packages: [], buildId: null },
  ];
  const plan = missingNoticePlan(entries, path => path.includes('/libok/'));
  assert.deepEqual(plan.packages, ['libok']);
  assert.deepEqual(plan.unresolved, ['usr/lib/libmissing.so\tno matching installed package\tno build ID']);
});
