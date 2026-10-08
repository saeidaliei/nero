(() => {
  const docs = Array.isArray(window.NERO_DOCS) ? window.NERO_DOCS : [];
  const overlay = document.getElementById('search-overlay');
  const input = document.getElementById('search-input');
  const results = document.getElementById('search-results');
  const open = document.getElementById('search-open');
  const closeButton = document.getElementById('search-close-button');
  const closeBackdrop = document.getElementById('search-close');
  const themeToggle = document.getElementById('theme-toggle');

  function setTheme(next) {
    document.documentElement.dataset.theme = next;
    localStorage.setItem('nero-theme', next);
  }

  function effectiveTheme() {
    const saved = document.documentElement.dataset.theme;
    if (saved === 'light' || saved === 'dark') return saved;
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }

  themeToggle?.addEventListener('click', () => {
    setTheme(effectiveTheme() === 'dark' ? 'light' : 'dark');
  });

  function score(doc, query) {
    const q = query.trim().toLowerCase();
    if (!q) return 0;
    const haystack = `${doc.title} ${doc.content || ''}`.toLowerCase();
    let index = haystack.indexOf(q);
    if (index === -1) return -1;
    let value = index;
    if (doc.title.toLowerCase().includes(q)) value -= 1000;
    return value;
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
      `<a class="search-result" href="${doc.permalink}"><strong>${escapeHtml(doc.title)}</strong><span>${escapeHtml(doc.summary || '')}</span></a>`
    ).join('');
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
