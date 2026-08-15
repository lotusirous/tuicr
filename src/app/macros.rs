use crate::config::{MacroAction, MacroConfig, MacroStep};
use crate::handler::run_colon_command;
use crate::model::CommentType;
use crate::review_store::{AddCommentRequest, CommentLevel, CommentTarget, add_comment_to_session};

use super::{App, InputMode};

impl App {
    /// Install macros from config (replaces any previously loaded set).
    pub fn load_macros(&mut self, macros: &[MacroConfig]) {
        self.macros.clear();
        for m in macros {
            self.macros.insert(m.key, m.steps.clone());
        }
    }

    pub fn begin_pending_at(&mut self) {
        self.pending_at = true;
    }

    pub fn clear_pending_at(&mut self) {
        self.pending_at = false;
    }

    /// Run the macro bound to `register`, if any.
    /// Sets `last_macro_register` when the macro starts (before steps run).
    pub fn run_macro(&mut self, register: char) {
        let Some(steps) = self.macros.get(&register).cloned() else {
            self.set_warning(format!("No macro defined for @{register}"));
            return;
        };

        self.last_macro_register = Some(register);
        self.clear_pending_at();

        for step in steps {
            if !self.run_macro_step(&step) {
                return;
            }
            // Mode-changing commands (e.g. :submit) leave Normal; stop the chain.
            if self.input_mode != InputMode::Normal {
                return;
            }
        }
    }

    /// Replay the last macro register (`@@`).
    pub fn run_last_macro(&mut self) {
        self.clear_pending_at();
        match self.last_macro_register {
            Some(register) => self.run_macro(register),
            None => self.set_warning("No previous macro to replay"),
        }
    }

    fn run_macro_step(&mut self, step: &MacroStep) -> bool {
        match &step.action {
            MacroAction::Command { command } => run_colon_command(self, command),
        }
    }

    /// Silently append an untyped comment at `level` using cursor context for
    /// file/line, then best-effort autosave.
    ///
    /// Used by `:comment [level] <text>` (and therefore by macros that run that
    /// command). Returns `false` on empty text, missing cursor context, or
    /// insert failure.
    pub fn add_comment_at_level(&mut self, level: CommentLevel, text: &str) -> bool {
        let content = text.trim();
        if content.is_empty() {
            self.set_warning("Comment cannot be empty");
            return false;
        }

        let (target, success_message) = match level {
            CommentLevel::Review => (
                CommentTarget::Review,
                "Review comment added".to_string(),
            ),
            CommentLevel::File => {
                let Some(path) = self.current_file_path().cloned() else {
                    self.set_warning("No file under cursor for :comment file");
                    return false;
                };
                (CommentTarget::File { path }, "File comment added".to_string())
            }
            CommentLevel::Line => {
                let Some(path) = self.current_file_path().cloned() else {
                    self.set_warning("No file under cursor for :comment line");
                    return false;
                };
                let Some((line, side)) = self.get_line_at_cursor() else {
                    self.set_warning("Move cursor to a diff line for :comment line");
                    return false;
                };
                (
                    CommentTarget::Line { path, line, side },
                    format!("Comment added to line {line}"),
                )
            }
        };

        let commit_id = match level {
            CommentLevel::Review => None,
            CommentLevel::File | CommentLevel::Line => self.commit_id_for_new_comment(),
        };

        let request = AddCommentRequest {
            target,
            content: content.to_string(),
            comment_type: CommentType::None,
            author: self.username.clone(),
            commit_id,
        };

        match add_comment_to_session(&mut self.session, request) {
            Ok(_) => {
                self.dirty = true;
                if let Err(e) = self.save_current_session_merging_external() {
                    // Comment is in-memory; keep going so later macro steps
                    // (e.g. submit) still see it.
                    self.set_error(format!("{success_message}; autosave failed: {e}"));
                } else {
                    self.set_message(success_message);
                }
                self.rebuild_annotations();
                true
            }
            Err(e) => {
                self.set_error(format!("Could not save comment: {e}"));
                false
            }
        }
    }

    /// Short summaries for help / status (register → description).
    pub fn macro_help_rows(&self) -> Vec<(char, String)> {
        let mut keys: Vec<char> = self.macros.keys().copied().collect();
        keys.sort_unstable();
        keys.into_iter()
            .filter_map(|key| {
                let steps = self.macros.get(&key)?;
                Some((key, summarize_macro_steps(steps)))
            })
            .collect()
    }
}

fn summarize_macro_steps(steps: &[MacroStep]) -> String {
    steps
        .iter()
        .map(|step| match &step.action {
            MacroAction::Command { command } => format!(":{command}"),
        })
        .collect::<Vec<_>>()
        .join(" → ")
}
