import assert from 'node:assert/strict';
import test from 'node:test';
import { isDuplicateDocumentTitle, renderMetadataChips, renderMetadataRows } from '../src/metadata.ts';

const metadata = [
  ['title', 'Release Plan'],
  ['date', '2026-10-13'],
  ['due', '2026-10-20'],
  ['tags', 'project, release'],
  ['status', 'in-progress'],
  ['type', 'task'],
  ['priority', 'high'],
  ['project', 'Nero'],
  ['author', 'Saeid'],
  ['source', 'https://example.com/plan'],
] as const;

test('metadata inspector renders all common fields and their values', () => {
  const html = renderMetadataRows(metadata);
  for (const [key, value] of metadata) {
    assert.ok(html.includes(`<span>${key}</span>`), `missing metadata key: ${key}`);
    assert.ok(html.includes(`<span>${value}</span>`), `missing metadata value: ${value}`);
  }
});

test('compact metadata chips show different fields but do not repeat the title', () => {
  const html = renderMetadataChips(metadata);
  assert.ok(!html.includes('metadata-chip-key">title'));
  for (const key of ['date', 'due', 'tags', 'status', 'type', 'priority', 'project', 'author', 'source']) {
    assert.ok(html.includes(`<span class="metadata-chip-key">${key}</span>`), `missing chip: ${key}`);
  }
  assert.ok(html.includes('2026-10-20'));
  assert.ok(html.includes('project, release'));
});

test('metadata keys and values are escaped before being inserted into the UI', () => {
  const hostile = [['<img src=x onerror=alert(1)>', '<script>nope</script>']] as const;
  const rows = renderMetadataRows(hostile);
  const chips = renderMetadataChips(hostile);
  for (const html of [rows, chips]) {
    assert.ok(html.includes('&lt;img src=x onerror=alert(1)&gt;'));
    assert.ok(html.includes('&lt;script&gt;nope&lt;/script&gt;'));
    assert.ok(!html.includes('<script>'));
    assert.ok(!html.includes('<img'));
  }
});


test('duplicate title detection removes only an exact first-level title heading', () => {
  assert.equal(isDuplicateDocumentTitle('H1', 'Release Plan', 'Release Plan'), true);
  assert.equal(isDuplicateDocumentTitle('h1', '  Release\nPlan  ', 'Release Plan'), true);
  assert.equal(isDuplicateDocumentTitle('H2', 'Release Plan', 'Release Plan'), false);
  assert.equal(isDuplicateDocumentTitle('H1', 'Overview', 'Release Plan'), false);
  assert.equal(isDuplicateDocumentTitle(undefined, '', 'Release Plan'), false);
});
