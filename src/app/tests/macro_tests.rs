//! Tests for config-defined `@` macros.
use std::path::{Path, PathBuf};

use crate::app::*;
use crate::config::{MacroConfig, MacroStep};
use crate::forge::traits::{ForgeRepository, PrSessionKey};
use crate::handler::run_colon_command;
use crate::model::diff_types::{DiffHunk, DiffLine, FileStatus, LineOrigin};
use crate::vcs::traits::{VcsChangeStatus, VcsType};

struct DummyVcs {
    info: VcsInfo,
}

impl VcsBackend for DummyVcs {
    fn info(&self) -> &VcsInfo {
        &self.info
    }
    fn get_working_tree_diff(&self, _h: &SyntaxHighlighter) -> Result<Vec<DiffFile>> {
        Err(TuicrError::NoChanges)
    }
    fn fetch_context_lines(
        &self,
        _p: &Path,
        _s: FileStatus,
        _ref_commit: Option<&str>,
        _start: u32,
        _end: u32,
    ) -> Result<Vec<DiffLine>> {
        Ok(Vec::new())
    }
    fn get_change_status(&self) -> Result<VcsChangeStatus> {
        Ok(VcsChangeStatus {
            staged: false,
            unstaged: false,
        })
    }
    fn file_line_count(&self, _p: &Path, _s: FileStatus, _ref_commit: Option<&str>) -> Result<u32> {
        Ok(0)
    }
}

fn make_pr_app() -> App {
    let vcs_info = VcsInfo {
        root_path: PathBuf::from("/tmp/repo"),
        head_commit: "abcdef0123".to_string(),
        branch_name: Some("feat".to_string()),
        vcs_type: VcsType::File,
    };
    let session = ReviewSession::new(
        vcs_info.root_path.clone(),
        vcs_info.head_commit.clone(),
        vcs_info.branch_name.clone(),
        SessionDiffSource::PullRequest,
    );
    let diff_file = DiffFile {
        old_path: Some(PathBuf::from("src/lib.rs")),
        new_path: Some(PathBuf::from("src/lib.rs")),
        status: FileStatus::Modified,
        hunks: vec![DiffHunk {
            header: "@@".to_string(),
            old_start: 1,
            old_count: 0,
            new_start: 1,
            new_count: 1,
            lines: vec![DiffLine {
                origin: LineOrigin::Addition,
                content: "b".to_string(),
                old_lineno: None,
                new_lineno: Some(1),
                highlighted_spans: None,
            }],
        }],
        is_binary: false,
        is_too_large: false,
        is_commit_message: false,
        content_hash: 0,
    };
    let pr_source = PullRequestDiffSource {
        key: PrSessionKey::new(
            ForgeRepository::github("github.com", "owner", "repo"),
            1,
            "abcdef0123".to_string(),
        ),
        base_sha: "0000".to_string(),
        title: "test".to_string(),
        url: "https://github.com/owner/repo/pull/1".to_string(),
        head_ref_name: "feat".to_string(),
        base_ref_name: "main".to_string(),
        state: "OPEN".to_string(),
        closed: false,
        merged: false,
    };
    let mut app = App::build(
        Box::new(DummyVcs {
            info: vcs_info.clone(),
        }),
        vcs_info,
        Theme::dark(),
        None,
        false,
        vec![diff_file],
        session,
        DiffSource::PullRequest(Box::new(pr_source)),
        InputMode::Normal,
        Vec::new(),
        None,
        None,
    )
    .expect("build app");
    app.current_pr_head = Some("abcdef0123".to_string());
    app
}

#[test]
fn should_run_colon_command_from_macro_helper() {
    let mut app = make_pr_app();
    assert!(run_colon_command(&mut app, "submit approve"));
    assert_eq!(app.input_mode, InputMode::SubmitConfirm);
}

#[test]
fn should_run_submit_approve_macro() {
    let mut app = make_pr_app();
    app.load_macros(&[MacroConfig {
        key: 'c',
        steps: vec![MacroStep::command("submit approve")],
    }]);

    app.run_macro('c');

    assert_eq!(app.last_macro_register, Some('c'));
    assert_eq!(app.input_mode, InputMode::SubmitConfirm);
}

#[test]
fn should_abort_remaining_steps_when_command_fails() {
    let mut app = make_pr_app();
    app.load_macros(&[MacroConfig {
        key: 'c',
        steps: vec![
            MacroStep::command("not-a-real-command"),
            MacroStep::command("submit approve"),
        ],
    }]);

    app.run_macro('c');

    assert_eq!(app.input_mode, InputMode::Normal);
    assert_eq!(app.last_macro_register, Some('c'));
}

#[test]
fn should_warn_on_unknown_macro_register() {
    let mut app = make_pr_app();
    app.run_macro('z');
    assert!(app.message.is_some());
    assert_eq!(app.last_macro_register, None);
}

#[test]
fn should_replay_last_macro_with_run_last_macro() {
    let mut app = make_pr_app();
    app.load_macros(&[MacroConfig {
        key: 'a',
        steps: vec![MacroStep::command("help")],
    }]);
    app.run_macro('a');
    assert_eq!(app.input_mode, InputMode::Help);
    assert_eq!(app.last_macro_register, Some('a'));

    app.input_mode = InputMode::Normal;
    app.run_last_macro();
    assert_eq!(app.input_mode, InputMode::Help);
}

#[test]
fn should_stop_macro_after_mode_changing_command() {
    let mut app = make_pr_app();
    app.load_macros(&[MacroConfig {
        key: 'c',
        steps: vec![
            MacroStep::command("submit approve"),
            MacroStep::command("help"),
        ],
    }]);

    app.run_macro('c');

    assert_eq!(app.input_mode, InputMode::SubmitConfirm);
}
