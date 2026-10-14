export type MetadataEntry = readonly [key: string, value: string];

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

/** Prevent the first rendered H1 from repeating the title shown above the note. */
export function isDuplicateDocumentTitle(
  firstTagName: string | undefined,
  headingText: string,
  title: string,
): boolean {
  const normalize = (value: string) => value.replace(/\s+/g, ' ').trim();
  return firstTagName?.toUpperCase() === 'H1' && normalize(headingText) === normalize(title);
}

/** Render all frontmatter fields in the detailed Context inspector. */
export function renderMetadataRows(frontmatter: readonly MetadataEntry[]): string {
  return frontmatter.map(([key, value]) => `
    <div class="meta-row"><span>${escapeHtml(key)}</span><span>${escapeHtml(value)}</span></div>
  `).join('');
}

/** Render compact metadata chips for narrow windows where the inspector is hidden. */
export function renderMetadataChips(frontmatter: readonly MetadataEntry[]): string {
  return frontmatter
    .filter(([key]) => key.trim().toLowerCase() !== 'title')
    .map(([key, value]) => `
      <span class="metadata-chip"><span class="metadata-chip-key">${escapeHtml(key)}</span><span class="metadata-chip-value">${escapeHtml(value)}</span></span>
    `).join('');
}
