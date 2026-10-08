use std::{path::{Path, PathBuf}, sync::mpsc::{self, Receiver}};

use notify::{recommended_watcher, RecursiveMode, Watcher};

use crate::{Result, Workspace};

pub struct WorkspaceWatcher {
    _watcher: notify::RecommendedWatcher,
}

impl Workspace {
    pub fn watch(&self) -> Result<(WorkspaceWatcher, Receiver<Vec<PathBuf>>)> {
        let (sender, receiver) = mpsc::channel();
        let root = self.root().to_path_buf();
        let watcher = recommended_watcher(move |result: notify::Result<notify::Event>| {
            let Ok(event) = result else {
                return;
            };
            let paths: Vec<PathBuf> = event
                .paths
                .into_iter()
                .filter(|path| is_user_markdown(path, &root))
                .filter_map(|path| path.strip_prefix(&root).ok().map(Path::to_path_buf))
                .collect();
            if !paths.is_empty() {
                let _ = sender.send(paths);
            }
        })?;

        let mut watcher = watcher;
        watcher.watch(self.root(), RecursiveMode::Recursive)?;
        Ok((WorkspaceWatcher { _watcher: watcher }, receiver))
    }
}

fn is_user_markdown(path: &Path, root: &Path) -> bool {
    if !path.starts_with(root) {
        return false;
    }
    if path.components().any(|part| part.as_os_str() == ".nero" || part.as_os_str() == ".note") {
        return false;
    }
    path.extension().and_then(|ext| ext.to_str()) == Some("md")
}
