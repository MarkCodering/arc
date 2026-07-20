use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::model::{
    command::CommandSpec,
    operation::{OperationPlan, PlanStep},
};

pub fn normalize_for_current_user(plan: &mut OperationPlan) {
    if !running_as_root() {
        return;
    }
    for step in &mut plan.steps {
        if step.command.program == "sudo" && !step.command.args.is_empty() {
            step.command.program = step.command.args.remove(0);
        }
    }
}

pub fn ensure_execution_privileges(plan: &OperationPlan) -> Result<()> {
    ensure_execution_privileges_with(
        plan,
        running_as_root(),
        Command::new("sudo").arg("--version").output().is_ok(),
    )
}

fn ensure_execution_privileges_with(
    plan: &OperationPlan,
    running_as_root: bool,
    sudo_available: bool,
) -> Result<()> {
    if running_as_root || !plan.steps.iter().any(|step| step.command.program == "sudo") {
        return Ok(());
    }
    if !sudo_available {
        bail!(
            "This plan requires administrative privileges, but arc is not running as root and sudo is unavailable. Run arc as your normal user after installing sudo, or use a root shell."
        );
    }
    Ok(())
}

fn running_as_root() -> bool {
    Command::new("id").arg("-u").output().is_ok_and(|output| {
        output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "0"
    })
}

pub trait CommandRunner {
    fn run(&self, command: &CommandSpec) -> Result<()>;
}

pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, command: &CommandSpec) -> Result<()> {
        let status = Command::new(&command.program)
            .args(&command.args)
            .status()
            .with_context(|| format!("could not start {}", command.program))?;
        if !status.success() {
            bail!(
                "command failed (exit status {status}): {}",
                command.display()
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ExecutionEvent<'a> {
    Started {
        index: usize,
        total: usize,
        step: &'a PlanStep,
    },
    Completed {
        index: usize,
        total: usize,
        step: &'a PlanStep,
    },
    Failed {
        index: usize,
        total: usize,
        step: &'a PlanStep,
    },
}

pub fn execute_plan_with_reporter<'a>(
    runner: &impl CommandRunner,
    plan: &'a OperationPlan,
    mut report: impl FnMut(ExecutionEvent<'a>),
) -> Result<()> {
    let total = plan.steps.len();
    for (index, step) in plan.steps.iter().enumerate() {
        report(ExecutionEvent::Started { index, total, step });
        if let Err(error) = runner.run(&step.command) {
            report(ExecutionEvent::Failed { index, total, step });
            return Err(error).with_context(|| step.description.clone());
        }
        report(ExecutionEvent::Completed { index, total, step });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use crate::model::operation::{OperationPlan, PlanStep};

    use super::*;

    #[derive(Default)]
    struct RecordingRunner {
        commands: RefCell<Vec<CommandSpec>>,
    }

    impl CommandRunner for RecordingRunner {
        fn run(&self, command: &CommandSpec) -> Result<()> {
            self.commands.borrow_mut().push(command.clone());
            Ok(())
        }
    }

    #[test]
    fn executes_the_exact_commands_stored_in_the_plan() {
        let expected = CommandSpec::new("gpu-check", ["--version"]);
        let plan = OperationPlan {
            title: "Test".into(),
            details: vec![],
            devices: vec![],
            steps: vec![PlanStep::new("check GPU", expected.clone())],
            confirmation_warning: String::new(),
            completion_message: String::new(),
            reboot_message: None,
        };
        let runner = RecordingRunner::default();

        execute_plan_with_reporter(&runner, &plan, |_| {}).unwrap();

        assert_eq!(*runner.commands.borrow(), vec![expected]);
    }

    #[test]
    fn root_normalization_preserves_the_exact_typed_command() {
        let mut plan = OperationPlan {
            title: "Test".into(),
            details: vec![],
            devices: vec![],
            steps: vec![PlanStep::new(
                "privileged",
                CommandSpec::sudo("apt-get", ["update"]),
            )],
            confirmation_warning: String::new(),
            completion_message: String::new(),
            reboot_message: None,
        };
        if running_as_root() {
            normalize_for_current_user(&mut plan);
            assert_eq!(plan.steps[0].command.display(), "apt-get update");
        }
    }

    #[test]
    fn missing_sudo_fails_before_any_command_runs() {
        let plan = OperationPlan {
            title: "Test".into(),
            details: vec![],
            devices: vec![],
            steps: vec![PlanStep::new(
                "privileged",
                CommandSpec::sudo("apt-get", ["update"]),
            )],
            confirmation_warning: String::new(),
            completion_message: String::new(),
            reboot_message: None,
        };
        assert!(
            ensure_execution_privileges_with(&plan, false, false)
                .unwrap_err()
                .to_string()
                .contains("sudo is unavailable")
        );
        assert!(ensure_execution_privileges_with(&plan, true, false).is_ok());
    }
}
