use std::process::Command;

pub(crate) struct CommandFailure {
    pub(crate) command: String,
    pub(crate) output: String,
}

pub(crate) fn execute_scancel(job_id: &str, signal: Option<&str>) -> Result<(), CommandFailure> {
    let mut command = Command::new("scancel");
    let mut command_display = String::from("scancel");

    if let Some(signal) = signal {
        command.arg("--signal").arg(signal);
        command_display.push_str(&format!(" --signal {signal}"));
    }
    command.arg(job_id);
    command_display.push_str(&format!(" {job_id}"));

    execute_command(command, command_display)
}

pub(crate) fn execute_scontrol_update_timelimit(
    job_id: &str,
    time_limit: &str,
) -> Result<(), CommandFailure> {
    let mut command = Command::new("scontrol");
    command
        .arg("update")
        .arg(format!("JobId={job_id}"))
        .arg(format!("TimeLimit={time_limit}"));

    execute_command(
        command,
        format!("scontrol update JobId={job_id} TimeLimit={time_limit}"),
    )
}

fn execute_command(mut command: Command, command_label: String) -> Result<(), CommandFailure> {
    let output = command.output().map_err(|error| CommandFailure {
        command: command_label.clone(),
        output: error.to_string(),
    })?;

    if output.status.success() {
        return Ok(());
    }

    let mut details = vec![match output.status.code() {
        Some(code) => format!("Exit code: {code}"),
        None => "Exit code: N/A".to_string(),
    }];

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout.trim_end();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim_end();
    let has_stdout = !stdout.is_empty();
    let has_stderr = !stderr.is_empty();
    match (has_stdout, has_stderr) {
        (true, true) => {
            details.push(format!("stdout:\n{stdout}"));
            details.push(format!("stderr:\n{stderr}"));
        }
        (true, false) => {
            details.push(stdout.to_string());
        }
        (false, true) => {
            details.push(stderr.to_string());
        }
        (false, false) => {}
    }

    if details.len() == 1 {
        details.push("No output.".to_string());
    }

    Err(CommandFailure {
        command: command_label,
        output: details.join("\n\n"),
    })
}
