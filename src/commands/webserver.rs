use std::process::Command as ProcessCommand;

use anyhow::{Context, Result};
use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::{app::Cli, commands::Command};

#[derive(Debug, Args)]
pub struct WebserverArgs {
    /// Host interface to bind to
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    /// TCP port to listen on
    #[arg(long, default_value_t = 8765)]
    pub port: u16,
}

#[derive(Debug, Deserialize)]
struct CommandRequest {
    args: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum CommandResponse {
    Success {
        data: Value,
    },
    Error {
        error: String,
        #[serde(flatten)]
        details: ErrorDetails,
    },
}

#[derive(Debug, Serialize, Default)]
struct ErrorDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    returncode: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stdout: Option<String>,
}

#[derive(Debug)]
struct RequestError {
    status: StatusCode,
    message: String,
}

impl RequestError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl Command for WebserverArgs {
    fn run_with_ctx(
        &self,
        _cli: &Cli,
        _out: &mut dyn std::io::Write,
        _ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        let addr = format!("{}:{}", self.host, self.port);
        let server =
            Server::http(&addr).map_err(|err| anyhow::anyhow!("failed to bind {addr}: {err}"))?;
        eprintln!("things3 webserver listening on http://{addr}");

        for request in server.incoming_requests() {
            if let Err(err) = handle_request(request) {
                eprintln!("webserver request error: {err}");
            }
        }

        Ok(())
    }
}

fn handle_request(mut request: Request) -> Result<()> {
    let (status, payload) = match process_request(&mut request) {
        Ok(response) => (StatusCode(200), response),
        Err(err) => (
            err.status,
            CommandResponse::Error {
                error: err.message,
                details: Default::default(),
            },
        ),
    };

    send_json(request, status, &payload)
}

fn process_request(request: &mut Request) -> std::result::Result<CommandResponse, RequestError> {
    if request.method() != &Method::Post {
        return Err(RequestError::new(StatusCode(405), "method not allowed"));
    }

    if request.url() != "/" {
        return Err(RequestError::new(StatusCode(404), "not found"));
    }

    let mut body = String::new();
    request
        .as_reader()
        .read_to_string(&mut body)
        .map_err(|_| RequestError::new(StatusCode(400), "failed reading request body"))?;

    let parsed = if body.trim().is_empty() {
        CommandRequest { args: None }
    } else {
        serde_json::from_str::<CommandRequest>(&body).map_err(|err| {
            RequestError::new(StatusCode(400), format!("invalid JSON payload: {err}"))
        })?
    };

    let args = normalize_args(parsed.args);

    execute_command(&args).map_err(|err| {
        RequestError::new(StatusCode(500), format!("failed to execute command: {err}"))
    })
}

fn normalize_args(args: Option<Vec<String>>) -> Vec<String> {
    let mut out = args
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    if out.is_empty() {
        out.push("today".to_string());
    }

    out
}

fn execute_command(args: &[String]) -> Result<CommandResponse> {
    let exe = std::env::current_exe().context("failed to locate current executable")?;

    let mut argv = args.to_vec();
    if !argv.iter().any(|v| v == "--json") {
        argv.insert(0, "--json".to_string());
    }

    let output = ProcessCommand::new(exe)
        .args(&argv)
        .output()
        .context("failed to execute command")?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        return Ok(command_error_response(
            "command failed",
            output.status.code(),
            stdout,
            stderr,
        ));
    }

    match serde_json::from_str::<Value>(&stdout) {
        Ok(data) => Ok(CommandResponse::Success { data }),
        Err(_) => Ok(command_error_response(
            "command did not return JSON output",
            output.status.code(),
            stdout,
            stderr,
        )),
    }
}

fn command_error_response(
    message: &str,
    returncode: Option<i32>,
    stdout: String,
    stderr: String,
) -> CommandResponse {
    CommandResponse::Error {
        error: message.to_string(),
        details: ErrorDetails {
            returncode,
            stderr: (!stderr.is_empty()).then_some(stderr),
            stdout: (!stdout.is_empty()).then_some(stdout),
        },
    }
}

fn send_json<T: Serialize>(request: Request, status: StatusCode, payload: &T) -> Result<()> {
    let body = serde_json::to_string(payload)?;
    let content_type = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
        .expect("static content-type header should always be valid");

    let response = Response::from_string(body)
        .with_status_code(status)
        .with_header(content_type);
    request.respond(response)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_args_defaults_to_today() {
        assert_eq!(normalize_args(None), vec!["today"]);
        assert_eq!(
            normalize_args(Some(vec!["".to_string(), " ".to_string()])),
            vec!["today"]
        );
    }

    #[test]
    fn normalize_args_trims_entries() {
        assert_eq!(
            normalize_args(Some(vec![" today ".to_string(), "  ".to_string()])),
            vec!["today"]
        );
    }
}
