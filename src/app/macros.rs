use crate::config::{MacroAction, MacroConfig, MacroStep};
use crate::handler::run_colon_command;

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
