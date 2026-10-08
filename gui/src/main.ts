import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import katex from 'katex';
import 'katex/dist/katex.min.css';
import './style.css';

type Note = { path: string; title: string };
type SearchResult = { note: Note; score: number; preview: string };
type Document = {
  note: Note;
  source: string;
  html: string;
  frontmatter: [string, string][];
  wikiLinks: { target: string; label?: string }[];
  tasks: { open: number; done: number };
  mathCount: number;
};

type ViewMode = 'read' | 'edit' | 'split';

const app = document.querySelector<HTMLDivElement>('#app')!;

let notes: Note[] = [];
let current: Document | null = null;
let draft = '';
let dirty = false;
let viewMode: ViewMode = 'read';
let activePanel: 'backlinks' | 'context' | null = 'context';
let paletteOpen = false;
let searchQuery = '';
let searchResults: SearchResult[] = [];
let searchTimer: number | undefined;
let saveTimer: number | undefined;
let editorSelection: { start: number; end: number } | null = null;

const esc = (value: string) =>
  value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&#039;');

function shell() {
  app.innerHTML = `
    <div class="app-shell">
      <header class="topbar">
        <div class="brand"><span class="brand-mark">N</span><span>Nero</span></div>
        <button id="search-button" class="search-button" title="Search (Ctrl/Cmd+K)">
          <span class="search-glyph">⌕</span><span id="search-label">Search</span><kbd>⌘K</kbd>
        </button>
        <div class="top-actions">
          <button id="edit-button" class="quiet-button">Edit</button>
          <button id="new-button" class="quiet-button">New</button>
          <button id="today-button" class="quiet-button">Today</button>
        </div>
      </header>
      <div class="workspace-grid" id="workspace-grid">
        <aside class="sidebar">
          <div class="sidebar-heading"><span>Notes</span><span id="note-count" class="muted"></span></div>
          <div id="note-list" class="note-list"></div>
        </aside>
        <main class="reader-wrap">
          <div id="reader" class="reader"></div>
        </main>
        <aside id="inspector" class="inspector"></aside>
      </div>
      <footer class="statusbar">
        <span id="status">Ready</span>
        <span id="status-right">Ctrl/Cmd K · search&nbsp;&nbsp; Ctrl/Cmd P · commands&nbsp;&nbsp; Ctrl/Cmd E · edit</span>
      </footer>
      <div id="overlay-root"></div>
    </div>
  `;

  document.querySelector('#search-button')?.addEventListener('click', openPalette);
  document.querySelector('#edit-button')?.addEventListener('click', toggleEditor);
  document.querySelector('#new-button')?.addEventListener('click', createNotePrompt);
  document.querySelector('#today-button')?.addEventListener('click', openToday);
}

function setStatus(message: string) {
  const el = document.querySelector('#status');
  if (el) el.textContent = message;
}

function setEditorState() {
  const button = document.querySelector<HTMLButtonElement>('#edit-button');
  if (!button) return;
  button.textContent = viewMode === 'read' ? 'Edit' : viewMode === 'edit' ? 'Preview' : 'Read';
  button.title = viewMode === 'read' ? 'Edit note (Ctrl/Cmd+E)' : 'Return to reading view';
}

function markDirty(value: boolean) {
  dirty = value;
  const path = current?.note.path ?? '';
  setStatus(value ? `Unsaved · ${path}` : path || 'Ready');
  const statusRight = document.querySelector('#status-right');
  if (statusRight) {
    statusRight.textContent = value
      ? 'Ctrl/Cmd S · save   Esc · read'
      : viewMode === 'read'
        ? 'Ctrl/Cmd K · search   Ctrl/Cmd P · commands   Ctrl/Cmd E · edit'
        : 'Ctrl/Cmd S · save   Ctrl/Cmd E · read';
  }
}

async function loadNotes() {
  notes = await invoke<Note[]>('list_notes');
  const list = document.querySelector('#note-list')!;
  document.querySelector('#note-count')!.textContent = String(notes.length);
  list.innerHTML = notes.map((note) => `
    <button class="note-row ${current?.note.path === note.path ? 'selected' : ''}" data-path="${esc(note.path)}">
      <span class="note-title">${esc(note.title)}</span>
      <span class="note-path">${esc(note.path)}</span>
    </button>
  `).join('');
  list.querySelectorAll<HTMLButtonElement>('[data-path]').forEach((button) => {
    button.addEventListener('click', () => openNote(button.dataset.path!));
  });
}

async function openNote(query: string) {
  if (dirty && current) {
    const ok = window.confirm('This note has unsaved changes. Discard them and open another note?');
    if (!ok) return;
  }
  try {
    const loaded = await invoke<Document>('read_note', { query });
    current = loaded;
    draft = loaded.source;
    editorSelection = null;
    dirty = false;
    viewMode = 'read';
    activePanel = 'context';
    renderReader();
    await loadNotes();
    markDirty(false);
  } catch (error) {
    setStatus(String(error));
  }
}

function renderReader() {
  const reader = document.querySelector('#reader')!;
  setEditorState();
  if (!current) {
    reader.innerHTML = `
      <div class="empty-state">
        <div class="empty-mark">N</div>
        <h1>A quiet place for your thoughts.</h1>
        <p>Choose a note or create one with <code>New</code>.</p>
      </div>
    `;
    renderInspector();
    return;
  }

  const header = `
    <div class="document-header ${viewMode !== 'read' ? 'compact' : ''}">
      <div class="eyebrow">${esc(current.note.path)}</div>
      <h1>${esc(current.note.title)}${dirty ? '<span class="dirty-dot" title="Unsaved changes"></span>' : ''}</h1>
    </div>
  `;

  if (viewMode === 'read') {
    reader.innerHTML = `${header}<div class="document-body">${current.html}</div>`;
    renderMath(reader);
    wireWikiLinks(reader);
    void hydrateImages(reader, current.note.path);
  } else if (viewMode === 'edit') {
    reader.innerHTML = `${header}${editorMarkup(false)}`;
    wireEditor();
  } else {
    reader.innerHTML = `${header}${editorMarkup(true)}`;
    wireEditor();
  }

  renderInspector();
}

function editorMarkup(split: boolean) {
  return `
    <div class="editor-shell ${split ? 'split-editor' : ''}">
      <div class="editor-pane">
        <div class="editor-toolbar">
          <span class="editor-label">Markdown</span>
          <div class="editor-actions">
            <span class="editor-hints">Tab indent · Enter continues lists · Ctrl+B bold</span>
            <button id="asset-button" class="editor-action" title="Insert image">Image</button>
          </div>
        </div>
        <div class="editor-frame">
          <div id="line-numbers" class="line-numbers" aria-hidden="true"></div>
          <div class="editor-surface">
            <pre id="syntax-layer" class="syntax-layer" aria-hidden="true"><code id="syntax-code"></code></pre>
            <textarea id="source-editor" spellcheck="false" autocomplete="off" autocorrect="off" autocapitalize="off">${esc(draft)}</textarea>
          </div>
        </div>
      </div>
      ${split ? `
        <div class="preview-pane">
          <div class="editor-toolbar"><span class="editor-label">Preview</span><span class="editor-hints">Live · scroll follows source</span></div>
          <article id="live-preview" class="document-body preview-body"></article>
        </div>
      ` : ''}
    </div>
  `;
}

function wireEditor() {
  const editor = document.querySelector<HTMLTextAreaElement>('#source-editor');
  if (!editor) return;

  syncLineNumbers(editor);
  syncSyntaxLayer(editor);
  if (viewMode === 'split') updateLivePreview();

  if (editorSelection) {
    editor.selectionStart = Math.min(editorSelection.start, editor.value.length);
    editor.selectionEnd = Math.min(editorSelection.end, editor.value.length);
  } else {
    editor.selectionStart = editor.value.length;
    editor.selectionEnd = editor.value.length;
  }
  editor.focus();

  editor.addEventListener('input', () => {
    draft = editor.value;
    rememberEditorSelection(editor);
    markDirty(true);
    syncLineNumbers(editor);
    syncSyntaxLayer(editor);
    if (viewMode === 'split') updateLivePreview();
    scheduleAutosaveIndicator();
  });

  editor.addEventListener('scroll', () => {
    rememberEditorSelection(editor);
    syncLineNumbers(editor);
    syncSyntaxLayer(editor);
    syncPreviewScroll(editor);
  });
  editor.addEventListener('select', () => rememberEditorSelection(editor));
  editor.addEventListener('keyup', () => rememberEditorSelection(editor));

  editor.addEventListener('keydown', (event) => {
    if (event.key === 'Tab') {
      event.preventDefault();
      if (event.shiftKey) indentSelection(editor, false);
      else indentSelection(editor, true);
      return;
    }

    if (event.key === 'Enter' && handleListEnter(editor, event)) return;

    const mod = event.metaKey || event.ctrlKey;
    if (mod && event.key.toLowerCase() === 's') {
      event.preventDefault();
      rememberEditorSelection(editor);
      void saveCurrentNote();
    } else if (mod && event.key.toLowerCase() === 'b') {
      event.preventDefault();
      wrapSelection(editor, '**');
    } else if (mod && event.key.toLowerCase() === 'i') {
      event.preventDefault();
      wrapSelection(editor, '*');
    } else if (mod && event.shiftKey && event.key === 'Enter') {
      event.preventDefault();
      rememberEditorSelection(editor);
      void saveCurrentNote();
    }
  });

  document.querySelector('#asset-button')?.addEventListener('click', () => void importImage(editor));
}

function rememberEditorSelection(editor: HTMLTextAreaElement) {
  editorSelection = { start: editor.selectionStart, end: editor.selectionEnd };
}

function syncSyntaxLayer(editor: HTMLTextAreaElement) {
  const code = document.querySelector<HTMLElement>('#syntax-code');
  const layer = document.querySelector<HTMLElement>('#syntax-layer');
  if (!code || !layer) return;
  code.innerHTML = highlightMarkdown(editor.value);
  layer.scrollTop = editor.scrollTop;
  layer.scrollLeft = editor.scrollLeft;
}

function highlightMarkdown(source: string): string {
  let inFence = false;
  return source.split('\n').map((line) => {
    if (/^\s*```/.test(line)) {
      inFence = !inFence;
      return `<span class="md-fence">${esc(line)}</span>`;
    }
    if (inFence) return `<span class="md-code-line">${esc(line)}</span>`;

    const heading = /^(\s{0,3})(#{1,6})(\s+)(.*)$/.exec(line);
    if (heading) {
      return `<span class="md-heading"><span class="md-syntax">${esc(heading[1] + heading[2])}</span>${esc(heading[3])}${highlightInlineSyntax(heading[4])}</span>`;
    }

    const task = /^(\s*)([-*+])(\s+)(\[[ xX]\])(\s+)(.*)$/.exec(line);
    if (task) {
      return `${esc(task[1])}<span class="md-list-marker">${esc(task[2])}</span>${esc(task[3])}<span class="md-task">${esc(task[4])}</span>${esc(task[5])}${highlightInlineSyntax(task[6])}`;
    }

    const bullet = /^(\s*)([-*+])(\s+)(.*)$/.exec(line);
    if (bullet) {
      return `${esc(bullet[1])}<span class="md-list-marker">${esc(bullet[2])}</span>${esc(bullet[3])}${highlightInlineSyntax(bullet[4])}`;
    }

    const ordered = /^(\s*)(\d+\.)(\s+)(.*)$/.exec(line);
    if (ordered) {
      return `${esc(ordered[1])}<span class="md-list-marker">${esc(ordered[2])}</span>${esc(ordered[3])}${highlightInlineSyntax(ordered[4])}`;
    }

    if (/^\s*>/.test(line)) {
      return `<span class="md-quote">${highlightInlineSyntax(line)}</span>`;
    }

    return highlightInlineSyntax(line);
  }).join('\n');
}

function highlightInlineSyntax(value: string): string {
  let text = esc(value);
  text = text.replace(/(\$\$?[\s\S]*?\$\$?)/g, '<span class="md-math">$1</span>');
  text = text.replace(/(\[\[[^\]]+\]\])/g, '<span class="md-wiki">$1</span>');
  text = text.replace(/(\[[^\]]+\]\([^\)]+\))/g, '<span class="md-link">$1</span>');
  text = text.replace(/(`[^`]+`)/g, '<span class="md-code-inline">$1</span>');
  text = text.replace(/(\*\*[^*]+\*\*)/g, '<span class="md-strong">$1</span>');
  return text;
}

function syncPreviewScroll(editor: HTMLTextAreaElement) {
  const preview = document.querySelector<HTMLElement>('#live-preview');
  if (!preview || viewMode !== 'split') return;
  const sourceMax = Math.max(1, editor.scrollHeight - editor.clientHeight);
  const previewMax = Math.max(1, preview.scrollHeight - preview.clientHeight);
  preview.scrollTop = (editor.scrollTop / sourceMax) * previewMax;
}

function handleListEnter(editor: HTMLTextAreaElement, event: KeyboardEvent): boolean {
  if (editor.selectionStart !== editor.selectionEnd) return false;
  const position = editor.selectionStart;
  const lineStart = editor.value.lastIndexOf('\n', position - 1) + 1;
  const lineEndRaw = editor.value.indexOf('\n', position);
  const lineEnd = lineEndRaw === -1 ? editor.value.length : lineEndRaw;
  const line = editor.value.slice(lineStart, lineEnd);
  const prefix = /^(\s*)([-*+]|\d+\.|>)(\s+)(.*)$/.exec(line);
  if (!prefix) return false;

  const [, indent, marker, , content] = prefix;
  const isQuote = marker === '>';
  const contentTrimmed = content.trim();
  let continuation = `${indent}${marker} `;
  if (!isQuote && /^\[([ xX])\]\s+/.test(content)) continuation = `${indent}${marker} [ ] `;
  if (!isQuote && /^\d+\.$/.test(marker)) {
    continuation = `${indent}${Number(marker.slice(0, -1)) + 1}. `;
  }

  event.preventDefault();
  let replacement: string;
  let caret: number;
  if (!contentTrimmed) {
    replacement = '\n';
    caret = lineStart + 1;
  } else {
    replacement = `\n${continuation}`;
    caret = position + replacement.length;
  }

  const next = editor.value.slice(0, position) + replacement + editor.value.slice(position);
  editor.value = next;
  editor.selectionStart = caret;
  editor.selectionEnd = caret;
  draft = next;
  rememberEditorSelection(editor);
  markDirty(true);
  syncLineNumbers(editor);
  syncSyntaxLayer(editor);
  if (viewMode === 'split') updateLivePreview();
  return true;
}

function syncLineNumbers(editor: HTMLTextAreaElement) {
  const gutter = document.querySelector<HTMLDivElement>('#line-numbers');
  if (!gutter) return;
  const count = Math.max(1, editor.value.split('\n').length);
  gutter.innerHTML = Array.from({ length: count }, (_, index) => `<span>${index + 1}</span>`).join('');
  gutter.scrollTop = editor.scrollTop;
}

function indentSelection(editor: HTMLTextAreaElement, add: boolean) {
  const start = editor.selectionStart;
  const end = editor.selectionEnd;
  const source = editor.value;
  const lineStart = source.lastIndexOf('\n', start - 1) + 1;
  const selectedEnd = source.indexOf('\n', end);
  const lineEnd = selectedEnd === -1 ? source.length : selectedEnd;
  const block = source.slice(lineStart, lineEnd);
  const lines = block.split('\n');
  const transformed = add
    ? lines.map((line) => `  ${line}`).join('\n')
    : lines.map((line) => line.startsWith('  ') ? line.slice(2) : line.startsWith(' ') ? line.slice(1) : line).join('\n');

  editor.value = source.slice(0, lineStart) + transformed + source.slice(lineEnd);
  const delta = transformed.length - block.length;
  editor.selectionStart = start + (add ? 2 : Math.min(0, delta));
  editor.selectionEnd = Math.max(editor.selectionStart, end + delta);
  draft = editor.value;
  markDirty(true);
  syncLineNumbers(editor);
  if (viewMode === 'split') updateLivePreview();
}

function wrapSelection(editor: HTMLTextAreaElement, marker: string) {
  const start = editor.selectionStart;
  const end = editor.selectionEnd;
  const selected = editor.value.slice(start, end) || 'text';
  const replacement = `${marker}${selected}${marker}`;
  editor.setRangeText(replacement, start, end, 'select');
  draft = editor.value;
  markDirty(true);
  if (viewMode === 'split') updateLivePreview();
}

function updateLivePreview() {
  const preview = document.querySelector<HTMLElement>('#live-preview');
  if (!preview || !current) return;
  preview.innerHTML = basicLivePreview(draft);
  renderMath(preview);
  void hydrateImages(preview, current.note.path);
  syncPreviewScroll(document.querySelector<HTMLTextAreaElement>('#source-editor')!);
}

function basicLivePreview(source: string) {
  const lines = source.split('\n');
  let html = '';
  let paragraph: string[] = [];
  let inCode = false;
  let codeLines: string[] = [];
  let listType: 'ul' | 'ol' | null = null;
  let listItems: string[] = [];

  const flushParagraph = () => {
    if (!paragraph.length) return;
    html += `<p>${inlinePreview(paragraph.join(' '))}</p>`;
    paragraph = [];
  };

  const flushList = () => {
    if (!listItems.length || !listType) return;
    html += `<${listType}>${listItems.map((item) => `<li>${inlinePreview(item)}</li>`).join('')}</${listType}>`;
    listItems = [];
    listType = null;
  };

  for (const line of lines) {
    if (/^\s*```/.test(line)) {
      if (inCode) {
        html += `<pre><code>${esc(codeLines.join('\n'))}</code></pre>`;
        inCode = false;
        codeLines = [];
      } else {
        flushParagraph();
        flushList();
        inCode = true;
      }
      continue;
    }
    if (inCode) {
      codeLines.push(line);
      continue;
    }
    if (!line.trim()) {
      flushParagraph();
      flushList();
      continue;
    }

    const heading = /^(#{1,6})\s+(.*)$/.exec(line);
    if (heading) {
      flushParagraph();
      flushList();
      const level = heading[1].length;
      html += `<h${level}>${inlinePreview(heading[2])}</h${level}>`;
      continue;
    }

    if (/^\s*(---+|\*\*\*+|___+)\s*$/.test(line)) {
      flushParagraph();
      flushList();
      html += '<hr />';
      continue;
    }

    const bullet = /^\s*[-*+]\s+(.*)$/.exec(line);
    if (bullet) {
      flushParagraph();
      if (listType && listType !== 'ul') flushList();
      listType = 'ul';
      listItems.push(bullet[1]);
      continue;
    }

    const ordered = /^\s*\d+[.)]\s+(.*)$/.exec(line);
    if (ordered) {
      flushParagraph();
      if (listType && listType !== 'ol') flushList();
      listType = 'ol';
      listItems.push(ordered[1]);
      continue;
    }

    const quote = /^\s*>\s?(.*)$/.exec(line);
    if (quote) {
      flushParagraph();
      flushList();
      html += `<blockquote>${inlinePreview(quote[1])}</blockquote>`;
      continue;
    }

    paragraph.push(line);
  }

  if (inCode) html += `<pre><code>${esc(codeLines.join('\n'))}</code></pre>`;
  flushParagraph();
  flushList();
  return html;
}

function inlinePreview(value: string) {
  const mathPattern = /\$\$([\s\S]*?)\$\$|\$([^$\n]+)\$/g;
  let html = '';
  let last = 0;
  for (const match of value.matchAll(mathPattern)) {
    const index = match.index ?? 0;
    html += formatPlainInline(value.slice(last, index));
    if (match[1] !== undefined) {
      html += `<span data-math-style="display">${esc(match[1].trim())}</span>`;
    } else {
      html += `<span data-math-style="inline">${esc(match[2].trim())}</span>`;
    }
    last = index + match[0].length;
  }
  html += formatPlainInline(value.slice(last));
  return html;
}

function formatPlainInline(value: string) {
  let text = esc(value);
  text = text.replace(/!\[([^\]]*)\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g, (_m, alt, src) =>
    `<img class="live-asset" data-nero-asset-src="${esc(src)}" alt="${esc(alt)}" />`,
  );
  text = text.replace(/\[\[([^\]|#]+)(?:#([^\]|]+))?(?:\|([^\]]+))?\]\]/g, (_m, target, heading, label) =>
    `<span class="wiki-live">${esc(label ?? target)}${heading ? `#${esc(heading)}` : ''}</span>`,
  );
  text = text.replace(/\[([^\]]+)\]\((https?:\/\/[^)]+)\)/g, (_m, label, href) =>
    `<a href="${esc(href)}" target="_blank" rel="noreferrer">${esc(label)}</a>`,
  );
  text = text.replace(/`([^`]+)`/g, '<code>$1</code>');
  text = text.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
  text = text.replace(/\*([^*]+)\*/g, '<em>$1</em>');
  return text;
}

async function hydrateImages(root: Element, sourcePath: string) {
  const images = Array.from(root.querySelectorAll<HTMLImageElement>('img'));
  await Promise.all(images.map(async (image) => {
    const reference = image.dataset.neroAssetSrc ?? image.getAttribute('src') ?? '';
    if (!reference || reference.startsWith('http://') || reference.startsWith('https://') || reference.startsWith('data:')) {
      return;
    }
    image.dataset.neroAssetSrc = reference;
    try {
      const dataUrl = await invoke<string>('read_asset_data_url', {
        sourcePath,
        reference,
      });
      image.src = dataUrl;
      image.dataset.neroHydrated = 'true';
    } catch {
      image.classList.add('asset-missing');
      image.alt = `${image.alt || 'image'} · missing`;
    }
  }));
}

function relativeMarkdownPath(fromNote: string, assetPath: string) {
  const from = fromNote.replace(/\\/g, '/').split('/');
  from.pop();
  const target = assetPath.replace(/\\/g, '/').split('/').filter(Boolean);
  let common = 0;
  while (common < from.length && common < target.length && from[common] === target[common]) common += 1;
  const parts = [...from.slice(common).map(() => '..'), ...target.slice(common)];
  return parts.join('/') || target[target.length - 1] || 'asset';
}

async function importImage(editor: HTMLTextAreaElement) {
  try {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp', 'avif', 'svg', 'bmp', 'ico'] }],
    });
    if (typeof selected !== 'string') return;
    const assetPath = await invoke<string>('import_asset', { sourcePath: selected });
    const markdownPath = relativeMarkdownPath(current?.note.path ?? '', assetPath);
    const source = editor.value;
    const start = editor.selectionStart;
    const end = editor.selectionEnd;
    const filename = markdownPath.split('/').pop() ?? 'image';
    const alt = filename.replace(/\.[^.]+$/, '').replace(/[-_]+/g, ' ');
    const replacement = `![${alt}](${markdownPath})`;
    editor.setRangeText(replacement, start, end, 'end');
    draft = editor.value;
    rememberEditorSelection(editor);
    markDirty(true);
    syncSyntaxLayer(editor);
    syncLineNumbers(editor);
    if (viewMode === 'split') updateLivePreview();
    setStatus(`Added · ${assetPath}`);
  } catch (error) {
    setStatus(`Image import failed · ${String(error)}`);
  }
}

function scheduleAutosaveIndicator() {
  if (saveTimer) window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => {
    if (dirty) setStatus(`Unsaved · ${current?.note.path ?? ''}`);
  }, 180);
}

async function saveCurrentNote() {
  const editor = document.querySelector<HTMLTextAreaElement>('#source-editor');
  if (editor) rememberEditorSelection(editor);
  if (!current || !dirty) return;
  try {
    const saved = await invoke<Document>('save_note', { query: current.note.path, source: draft });
    current = saved;
    draft = saved.source;
    dirty = false;
    await loadNotes();
    renderReader();
    markDirty(false);
    setStatus(`Saved · ${saved.note.path}`);
  } catch (error) {
    setStatus(`Save failed · ${String(error)}`);
  }
}

async function toggleEditor() {
  const editor = document.querySelector<HTMLTextAreaElement>('#source-editor');
  if (editor) rememberEditorSelection(editor);
  if (!current) return;
  if (viewMode === 'read') {
    draft = current.source;
    viewMode = 'split';
    renderReader();
    return;
  }
  if (dirty) await saveCurrentNote();
  viewMode = 'read';
  renderReader();
  markDirty(false);
}

function renderMath(root: Element) {
  root.querySelectorAll<HTMLElement>('[data-math-style]').forEach((element) => {
    const source = element.textContent?.trim() ?? '';
    const displayMode = element.dataset.mathStyle === 'display';
    try {
      katex.render(source, element, { displayMode, throwOnError: false, trust: false });
    } catch {
      element.classList.add('math-error');
    }
  });
}

function wireWikiLinks(root: Element) {
  root.querySelectorAll<HTMLAnchorElement>('a[data-wikilink="true"]').forEach((anchor) => {
    anchor.addEventListener('click', async (event) => {
      event.preventDefault();
      const target = anchor.getAttribute('href') ?? '';
      if (!current) return;
      try {
        const resolved = await invoke<Note | null>('resolve_link', {
          sourcePath: current.note.path,
          target,
        });
        if (resolved) await openNote(resolved.path);
        else setStatus(`Broken link: ${target}`);
      } catch (error) {
        setStatus(String(error));
      }
    });
  });
}

async function renderInspector() {
  const inspector = document.querySelector('#inspector')!;
  if (!current || activePanel === null) {
    inspector.innerHTML = '';
    return;
  }

  if (activePanel === 'backlinks') {
    try {
      const items = await invoke<Note[]>('backlinks', { query: current.note.path });
      inspector.innerHTML = panelMarkup('Backlinks', items.map((item) => `
        <button class="inspector-row" data-path="${esc(item.path)}"><strong>${esc(item.title)}</strong><small>${esc(item.path)}</small></button>
      `).join('') || '<div class="panel-empty">Nothing points here yet.</div>');
      wireInspectorLinks();
    } catch (error) {
      inspector.innerHTML = panelMarkup('Backlinks', `<div class="panel-empty">${esc(String(error))}</div>`);
    }
    return;
  }

  const links = current.wikiLinks.map((link) => `
    <div class="context-link"><span>→</span><span>${esc(link.label ?? link.target)}</span></div>
  `).join('');
  const meta = current.frontmatter.map(([key, value]) => `
    <div class="meta-row"><span>${esc(key)}</span><span>${esc(value)}</span></div>
  `).join('');

  inspector.innerHTML = panelMarkup('Context', `
    <div class="stat-grid">
      <div><strong>${current.tasks.open}</strong><span>open tasks</span></div>
      <div><strong>${current.tasks.done}</strong><span>done</span></div>
      <div><strong>${current.mathCount}</strong><span>math blocks</span></div>
    </div>
    ${links ? `<div class="inspector-section"><div class="section-label">Links</div>${links}</div>` : ''}
    ${meta ? `<div class="inspector-section"><div class="section-label">Metadata</div>${meta}</div>` : ''}
    <button id="show-backlinks" class="panel-link">Show backlinks →</button>
  `);
  document.querySelector('#show-backlinks')?.addEventListener('click', () => {
    activePanel = 'backlinks';
    renderInspector();
  });
  document.querySelector('#close-inspector')?.addEventListener('click', () => {
    activePanel = null;
    renderInspector();
  });
}

function panelMarkup(title: string, body: string) {
  return `<div class="inspector-header"><span>${esc(title)}</span><button id="close-inspector" aria-label="Close">×</button></div>${body}`;
}

function wireInspectorLinks() {
  document.querySelector('#close-inspector')?.addEventListener('click', () => {
    activePanel = null;
    renderInspector();
  });
  document.querySelectorAll<HTMLButtonElement>('.inspector-row[data-path]').forEach((button) => {
    button.addEventListener('click', () => openNote(button.dataset.path!));
  });
}

function openPalette() {
  if (viewMode !== 'read' && document.activeElement?.matches('#source-editor')) return;
  paletteOpen = true;
  searchResults = [];
  searchQuery = '';
  renderOverlay();
}

function closePalette() {
  paletteOpen = false;
  document.querySelector('#overlay-root')!.innerHTML = '';
}

function renderOverlay() {
  const root = document.querySelector('#overlay-root')!;
  if (!paletteOpen) {
    root.innerHTML = '';
    return;
  }

  root.innerHTML = `
    <div class="scrim" id="scrim">
      <div class="palette" role="dialog" aria-modal="true">
        <div class="palette-input-wrap"><span>⌕</span><input id="palette-input" autofocus placeholder="Search notes or type a command…" /></div>
        <div id="palette-results"></div>
      </div>
    </div>
  `;

  const input = document.querySelector<HTMLInputElement>('#palette-input')!;
  input.addEventListener('input', () => {
    searchQuery = input.value;
    scheduleSearch();
    renderPaletteResults();
  });
  input.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') closePalette();
    if (event.key === 'Enter') void activatePaletteInput();
  });
  document.querySelector('#scrim')?.addEventListener('click', (event) => {
    if (event.target === event.currentTarget) closePalette();
  });
  renderPaletteResults();
  input.focus();
}

function scheduleSearch() {
  if (searchTimer) window.clearTimeout(searchTimer);
  searchTimer = window.setTimeout(async () => {
    const query = searchQuery.trim();
    if (!query || query.startsWith(':')) return;
    try {
      searchResults = await invoke<SearchResult[]>('search_notes', { query });
      renderPaletteResults();
    } catch (error) {
      setStatus(String(error));
    }
  }, 90);
}

function commandList() {
  return [
    ['new', 'Create a Markdown note'],
    ['today', "Open today's note"],
    ['edit', 'Edit the current note'],
    ['save', 'Save the current note'],
    ['split', 'Toggle editor and preview'],
    ['image', 'Insert an image into the current note'],
    ['backlinks', 'Show notes linking here'],
    ['context', 'Show note context'],
    ['reindex', 'Refresh the search index'],
    ['close', 'Close the palette'],
  ];
}

function renderPaletteResults() {
  const results = document.querySelector('#palette-results');
  if (!results) return;
  const query = searchQuery.trim();

  if (query.startsWith(':')) {
    const needle = query.slice(1).toLowerCase();
    results.innerHTML = commandList().filter(([name]) => name.includes(needle)).map(([name, description]) => `
      <button class="palette-row command" data-command="${name}"><span>:</span><strong>${name}</strong><small>${description}</small></button>
    `).join('') || '<div class="panel-empty">No matching command.</div>';
    results.querySelectorAll<HTMLButtonElement>('[data-command]').forEach((button) => {
      button.addEventListener('click', () => void runCommand(button.dataset.command!));
    });
    return;
  }

  if (!query) {
    results.innerHTML = `
      <div class="palette-hint">Search your notes</div>
      <div class="palette-shortcuts"><span>⌘K</span> Search <span>⌘P</span> Commands <span>Esc</span> Close</div>
    `;
    return;
  }

  results.innerHTML = searchResults.map((result) => `
    <button class="palette-row" data-search-path="${esc(result.note.path)}">
      <strong>${esc(result.note.title)}</strong>
      <small>${esc(result.preview)}</small>
    </button>
  `).join('') || '<div class="panel-empty">No matches.</div>';
  results.querySelectorAll<HTMLButtonElement>('[data-search-path]').forEach((button) => {
    button.addEventListener('click', async () => {
      const path = button.dataset.searchPath!;
      closePalette();
      await openNote(path);
    });
  });
}

async function activatePaletteInput() {
  const query = searchQuery.trim();
  if (!query) return;
  if (query.startsWith(':')) {
    await runCommand(query.slice(1).trim().split(/\s+/)[0] ?? '');
    return;
  }
  const first = searchResults[0];
  if (first) {
    closePalette();
    await openNote(first.note.path);
  }
}

async function runCommand(command: string) {
  closePalette();
  switch (command) {
    case 'new': await createNotePrompt(); break;
    case 'today': await openToday(); break;
    case 'edit': if (viewMode === 'read') await toggleEditor(); break;
    case 'save': await saveCurrentNote(); break;
    case 'split':
      if (current) {
        viewMode = viewMode === 'split' ? 'edit' : 'split';
        renderReader();
      }
      break;
    case 'image':
      if (current && viewMode === 'read') {
        await toggleEditor();
      }
      {
        const editor = document.querySelector<HTMLTextAreaElement>('#source-editor');
        if (editor) await importImage(editor);
      }
      break;
    case 'backlinks': activePanel = 'backlinks'; await renderInspector(); break;
    case 'context': activePanel = 'context'; await renderInspector(); break;
    case 'reindex':
      await invoke('reindex');
      await loadNotes();
      setStatus('Index refreshed');
      break;
  }
}

async function createNotePrompt() {
  const title = window.prompt('Note title');
  if (!title?.trim()) return;
  try {
    const note = await invoke<Note>('create_note', { title: title.trim() });
    await loadNotes();
    await openNote(note.path);
    viewMode = 'split';
    renderReader();
  } catch (error) {
    setStatus(String(error));
  }
}

async function openToday() {
  try {
    const note = await invoke<Note>('today');
    await loadNotes();
    await openNote(note.path);
  } catch (error) {
    setStatus(String(error));
  }
}

async function boot() {
  shell();
  try {
    const info = await invoke<{ root: string }>('workspace_info');
    setStatus(info.root);
    await loadNotes();
    if (notes.length) await openNote(notes[0].path);
    else {
      renderReader();
      renderInspector();
    }
  } catch (error) {
    setStatus(String(error));
  }

  await listen<string[]>('workspace-changed', async () => {
    const activePath = current?.note.path;
    await loadNotes();
    if (activePath && notes.some((note) => note.path === activePath)) {
      if (!dirty) await openNote(activePath);
      else setStatus('File changed on disk · your unsaved draft is safe');
    } else if (activePath) {
      current = null;
      dirty = false;
      renderReader();
      renderInspector();
    }
    if (!dirty) setStatus('Workspace updated');
  });
}

document.addEventListener('keydown', (event) => {
  const mod = event.metaKey || event.ctrlKey;
  const editorFocused = document.activeElement?.matches('#source-editor');

  if (mod && event.key.toLowerCase() === 's' && editorFocused) {
    event.preventDefault();
    void saveCurrentNote();
    return;
  }

  if (mod && event.key.toLowerCase() === 'e' && !paletteOpen) {
    event.preventDefault();
    void toggleEditor();
    return;
  }

  if (event.key === 'Escape' && paletteOpen) {
    closePalette();
    return;
  }

  if (event.key === 'Escape' && editorFocused) {
    event.preventDefault();
    if (dirty) void saveCurrentNote().then(() => {
      viewMode = 'read';
      renderReader();
    });
    else {
      viewMode = 'read';
      renderReader();
    }
    return;
  }

  if (mod && event.key.toLowerCase() === 'k') {
    event.preventDefault();
    if (paletteOpen) closePalette(); else openPalette();
  } else if (mod && event.key.toLowerCase() === 'p') {
    event.preventDefault();
    openPalette();
  }
});

void boot();
