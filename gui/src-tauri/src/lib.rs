//! Tauri application bridge for Nero's desktop frontend.
//!
//! Commands in this module translate WebView requests into `nero-core` calls. The
//! Rust core remains authoritative for filesystem access, parsing, search, and links;
//! the TypeScript side should not duplicate those rules.

use std::{env, path::PathBuf, sync::Mutex, thread};

use nero_core::{Document, NoteSummary, TaskCounts, WikiLink, Workspace, WorkspaceWatcher};
use serde::Serialize;
use tauri::{Emitter, State};

struct AppState {
    workspace: Workspace,
    _watcher: Mutex<Option<WorkspaceWatcher>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NoteDto {
    path: String,
    title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskDto {
    open: usize,
    done: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WikiLinkDto {
    target: String,
    label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentDto {
    note: NoteDto,
    source: String,
    html: String,
    frontmatter: Vec<(String, String)>,
    wiki_links: Vec<WikiLinkDto>,
    tasks: TaskDto,
    math_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchDto {
    note: NoteDto,
    score: f64,
    preview: String,
}

#[derive(Debug, Clone, Serialize)]
struct WorkspaceDto {
    root: String,
}

fn error_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn to_note(note: NoteSummary) -> NoteDto {
    NoteDto {
        path: note.path.to_string_lossy().replace('\\', "/"),
        title: note.title,
    }
}

fn to_document(note: nero_core::Note) -> DocumentDto {
    let NeroCoreNote { summary, document } = NeroCoreNote::from(note);
    let frontmatter = document.frontmatter.into_iter().collect();
    DocumentDto {
        note: to_note(summary),
        source: document.source,
        html: document.html,
        frontmatter,
        wiki_links: document.wiki_links.into_iter().map(to_wiki_link).collect(),
        tasks: to_tasks(document.tasks),
        math_count: document.math_count,
    }
}

struct NeroCoreNote {
    summary: NoteSummary,
    document: Document,
}

impl From<nero_core::Note> for NeroCoreNote {
    fn from(note: nero_core::Note) -> Self {
        Self { summary: note.summary, document: note.document }
    }
}

fn to_tasks(tasks: TaskCounts) -> TaskDto {
    TaskDto { open: tasks.open, done: tasks.done }
}

fn to_wiki_link(link: WikiLink) -> WikiLinkDto {
    WikiLinkDto { target: link.target, label: link.label }
}

fn workspace_from_env() -> Result<Workspace, String> {
    let root = env::var_os("NERO_WORKSPACE")
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok())
        .ok_or_else(|| "could not determine a workspace directory".to_owned())?;

    Workspace::discover(&root)
        .or_else(|_| Workspace::open(&root))
        .map_err(error_message)
}

#[tauri::command]
fn workspace_info(state: State<'_, AppState>) -> Result<WorkspaceDto, String> {
    Ok(WorkspaceDto { root: state.workspace.root().to_string_lossy().to_string() })
}

#[tauri::command]
fn list_notes(state: State<'_, AppState>) -> Result<Vec<NoteDto>, String> {
    state.workspace.list_notes()
        .map(|notes| notes.into_iter().map(to_note).collect())
        .map_err(error_message)
}

#[tauri::command]
fn read_note(query: String, state: State<'_, AppState>) -> Result<DocumentDto, String> {
    state.workspace.read_note(&query)
        .map(to_document)
        .map_err(error_message)
}

#[tauri::command]
fn render_markdown(source: String) -> Result<String, String> {
    Ok(nero_core::render_html(&source))
}

#[tauri::command]
fn search_notes(query: String, state: State<'_, AppState>) -> Result<Vec<SearchDto>, String> {
    state.workspace.search(&query)
        .map(|results| results.into_iter().map(|result| SearchDto {
            note: to_note(result.note),
            score: result.score,
            preview: result.preview,
        }).collect())
        .map_err(error_message)
}

#[tauri::command]
fn backlinks(query: String, state: State<'_, AppState>) -> Result<Vec<NoteDto>, String> {
    state.workspace.backlinks(&query)
        .map(|notes| notes.into_iter().map(to_note).collect())
        .map_err(error_message)
}

#[tauri::command]
fn resolve_link(source_path: String, target: String, state: State<'_, AppState>) -> Result<Option<NoteDto>, String> {
    state.workspace.resolve_link(PathBuf::from(source_path).as_path(), &target)
        .map(|note| note.map(to_note))
        .map_err(error_message)
}

#[tauri::command]
fn create_note(title: String, state: State<'_, AppState>) -> Result<NoteDto, String> {
    state.workspace.create_note(&title)
        .map(to_note)
        .map_err(error_message)
}


#[tauri::command]
fn import_asset(source_path: String, state: State<'_, AppState>) -> Result<String, String> {
    state.workspace.import_asset(PathBuf::from(source_path)).map_err(error_message)
}

#[tauri::command]
fn read_asset_data_url(source_path: String, reference: String, state: State<'_, AppState>) -> Result<String, String> {
    state.workspace.read_asset_data_url(PathBuf::from(source_path).as_path(), &reference).map_err(error_message)
}

#[tauri::command]
// Save through the core so GUI edits get the same atomic-write and workspace-boundary
// guarantees as CLI/TUI edits. The frontend never writes note files directly.
fn save_note(query: String, source: String, state: State<'_, AppState>) -> Result<DocumentDto, String> {
    state.workspace.save_note(&query, &source)
        .map(to_document)
        .map_err(error_message)
}

#[tauri::command]
fn today(state: State<'_, AppState>) -> Result<NoteDto, String> {
    state.workspace.today()
        .map(to_note)
        .map_err(error_message)
}

#[tauri::command]
fn reindex(state: State<'_, AppState>) -> Result<usize, String> {
    state.workspace.reindex()
        .map(|stats| stats.indexed)
        .map_err(error_message)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let workspace = match workspace_from_env() {
        Ok(workspace) => workspace,
        Err(error) => {
            eprintln!("nero-gui: {error}");
            #[cfg(not(mobile))]
            std::process::exit(1);
            return;
        }
    };

    let (watcher, receiver) = match workspace.watch() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("nero-gui: watcher unavailable: {error}");
            // The UI remains usable without live reload.
            let builder = tauri::Builder::default()
                .plugin(tauri_plugin_dialog::init())
                .manage(AppState {
                workspace,
                _watcher: Mutex::new(None),
            });
            run_builder(builder);
            return;
        }
    };

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            workspace: workspace.clone(),
            _watcher: Mutex::new(Some(watcher)),
        })
        .setup(move |app| {
            let handle = app.handle().clone();
            let watcher_workspace = workspace.clone();
            thread::spawn(move || {
                for paths in receiver {
                    let _ = watcher_workspace.reindex();
                    let paths = paths
                        .into_iter()
                        .map(|path| path.to_string_lossy().replace('\\', "/"))
                        .collect::<Vec<_>>();
                    let _ = handle.emit("workspace-changed", paths);
                }
            });
            Ok(())
        });

    run_builder(builder);
}

fn run_builder(builder: tauri::Builder<tauri::Wry>) {
    let result = builder
        .invoke_handler(tauri::generate_handler![
            workspace_info,
            list_notes,
            read_note,
            render_markdown,
            search_notes,
            backlinks,
            resolve_link,
            create_note,
            save_note,
            import_asset,
            read_asset_data_url,
            today,
            reindex,
        ])
        .run(tauri::generate_context!());

    match result {
        Ok(()) => {}
        Err(error) => {
            eprintln!("nero-gui: {error}");
            #[cfg(not(mobile))]
            std::process::exit(1);
        }
    }
}
