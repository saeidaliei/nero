use std::{
    path::Path,
    process::{Command, Output},
};

use crate::{NeroError, Result, Workspace};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStatus {
    pub branch: String,
    pub porcelain: String,
}

impl Workspace {
    pub fn git_init(&self) -> Result<String> {
        let output = run_git(self.root(), &["init"])?;
        git_stdout(output)
    }

    pub fn git_status(&self) -> Result<GitStatus> {
        let branch = git_stdout(run_git(self.root(), &["branch", "--show-current"])?)?
            .trim()
            .to_owned();
        let porcelain = git_stdout(run_git(
            self.root(),
            &["status", "--porcelain=v1", "--branch"],
        )?)?;
        Ok(GitStatus { branch, porcelain })
    }

    pub fn git_snapshot(&self, message: &str) -> Result<String> {
        let message = message.trim();
        if message.is_empty() {
            return Err(NeroError::Message(
                "git snapshot message cannot be empty".into(),
            ));
        }
        run_git(self.root(), &["add", "--all"])?;
        let output = run_git(self.root(), &["commit", "-m", message])?;
        git_stdout(output)
    }

    pub fn git_add_remote(&self, name: &str, url: &str) -> Result<String> {
        validate_git_name(name)?;
        let remotes = run_git(self.root(), &["remote"])?;
        let existing = git_stdout(remotes)?;
        let args: Vec<String> = if existing.lines().any(|line| line.trim() == name) {
            vec!["remote".into(), "set-url".into(), name.into(), url.into()]
        } else {
            vec!["remote".into(), "add".into(), name.into(), url.into()]
        };
        let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        git_stdout(run_git(self.root(), &refs)?)
    }

    pub fn git_remotes(&self) -> Result<String> {
        git_stdout(run_git(self.root(), &["remote", "-v"])?)
    }

    pub fn git_push(&self, remote: &str, branch: Option<&str>) -> Result<String> {
        validate_git_name(remote)?;
        let branch = branch.map(str::trim).filter(|value| !value.is_empty());
        if let Some(branch) = branch {
            validate_git_branch(branch)?;
        }
        match branch {
            Some(branch) => git_stdout(run_git(self.root(), &["push", remote, branch])?),
            None => git_stdout(run_git(self.root(), &["push", remote])?),
        }
    }

    pub fn git_pull(&self, remote: &str, branch: Option<&str>) -> Result<String> {
        validate_git_name(remote)?;
        let branch = branch.map(str::trim).filter(|value| !value.is_empty());
        if let Some(branch) = branch {
            validate_git_branch(branch)?;
        }
        match branch {
            Some(branch) => git_stdout(run_git(self.root(), &["pull", remote, branch])?),
            None => git_stdout(run_git(self.root(), &["pull", remote])?),
        }
    }
}

fn run_git(root: &Path, args: &[&str]) -> Result<Output> {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| NeroError::Message(format!("could not run git: {error}")))
        .and_then(|output| {
            if output.status.success() {
                Ok(output)
            } else {
                Err(NeroError::Message(format!(
                    "git {} failed: {}",
                    args.join(" "),
                    String::from_utf8_lossy(&output.stderr).trim()
                )))
            }
        })
}

fn git_stdout(output: Output) -> Result<String> {
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn validate_git_name(name: &str) -> Result<()> {
    if name.trim().is_empty()
        || name.starts_with('-')
        || name.chars().any(char::is_whitespace)
        || name.contains('/')
    {
        return Err(NeroError::Message(format!(
            "invalid git remote name: {name}"
        )));
    }
    Ok(())
}

fn validate_git_branch(branch: &str) -> Result<()> {
    if branch.is_empty() || branch.starts_with('-') || branch.chars().any(char::is_whitespace) {
        return Err(NeroError::Message(format!("invalid git branch: {branch}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_remote_names() {
        assert!(validate_git_name("").is_err());
        assert!(validate_git_name("origin/main").is_err());
        assert!(validate_git_name(" origin").is_err());
        assert!(validate_git_name("origin").is_ok());
        assert!(validate_git_branch("main").is_ok());
        assert!(validate_git_branch("--force").is_err());
        assert!(validate_git_branch("main feature").is_err());
    }
}
