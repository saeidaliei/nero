(() => {
  const docs = Array.isArray(window.NERO_DOCS) ? window.NERO_DOCS : [];
  const overlay = document.getElementById('search-overlay');
  const input = document.getElementById('search-input');
  const results = document.getElementById('search-results');
  const open = document.getElementById('search-open');
  const closeButton = document.getElementById('search-close-button');
  const closeBackdrop = document.getElementById('search-close');
  function score(doc, query) {
    const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
    if (!terms.length) return 0;

    const title = String(doc.title || '').toLowerCase();
    const summary = String(doc.summary || '').toLowerCase();
    const content = String(doc.content || '').toLowerCase();
    const haystack = `${title} ${summary} ${content}`;
    if (!terms.every(term => haystack.includes(term))) return -1;

    // Every query word must match, but words can occur in different parts of a page.
    // Prefer title matches, then summaries, then body text near the start of a page.
    return terms.reduce((value, term) => {
      if (title.includes(term)) value -= 1000;
      else if (summary.includes(term)) value -= 100;
      const index = haystack.indexOf(term);
      return value + Math.max(index, 0) / 1000;
    }, 0);
  }

  function renderResults(query) {
    if (!results) return;
    if (!query.trim()) {
      results.innerHTML = '<div class="search-empty">Start typing to search Nero documentation.</div>';
      return;
    }
    const matches = docs
      .map(doc => ({ doc, score: score(doc, query) }))
      .filter(item => item.score >= 0)
      .sort((a, b) => a.score - b.score)
      .slice(0, 8);

    if (!matches.length) {
      results.innerHTML = '<div class="search-empty">No documentation found.</div>';
      return;
    }
    results.innerHTML = matches.map(({ doc }) =>
      `<a class="search-result" href="${escapeHtml(doc.permalink)}"><strong>${escapeHtml(doc.title)}</strong><span>${escapeHtml(previewFor(doc, query))}</span></a>`
    ).join('');
  }

  function previewFor(doc, query) {
    const summary = String(doc.summary || '').replace(/\s+/g, ' ').trim();
    const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
    if (summary && terms.every(term => summary.toLowerCase().includes(term))) {
      return summary.slice(0, 200);
    }

    const content = String(doc.content || '').replace(/\s+/g, ' ').trim();
    const lowerContent = content.toLowerCase();
    const matchAt = terms
      .map(term => lowerContent.indexOf(term))
      .filter(index => index >= 0)
      .sort((a, b) => a - b)[0];
    if (matchAt === undefined) return summary.slice(0, 200);

    const start = Math.max(0, matchAt - 55);
    const end = Math.min(content.length, start + 200);
    return `${start > 0 ? '…' : ''}${content.slice(start, end)}${end < content.length ? '…' : ''}`;
  }

  function escapeHtml(value) {
    return String(value).replace(/[&<>'"]/g, ch => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;' })[ch]);
  }

  function showSearch() {
    if (!overlay) return;
    overlay.hidden = false;
    requestAnimationFrame(() => input?.focus());
    renderResults(input?.value || '');
  }

  function hideSearch() {
    if (!overlay) return;
    overlay.hidden = true;
    input?.blur();
  }

  open?.addEventListener('click', showSearch);
  closeButton?.addEventListener('click', hideSearch);
  closeBackdrop?.addEventListener('click', hideSearch);
  input?.addEventListener('input', () => renderResults(input.value));
  document.addEventListener('keydown', event => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
      event.preventDefault();
      showSearch();
    }
    if (event.key === 'Escape' && overlay && !overlay.hidden) hideSearch();
  });
})();
