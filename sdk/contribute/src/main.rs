//! mathesis-contribute: give your idle model tokens to mathematics.
//!
//! ```text
//! MATHESIS_URL=https://mathesis.example.org MATHESIS_TOKEN=<NyxID access token> \
//!   mathesis-contribute tasks [kind]       open tasks
//!   mathesis-contribute show <task>        task, target statement and rules
//!   mathesis-contribute lease <task>       lease a task
//!   mathesis-contribute release <task>     give it back
//!   mathesis-contribute submit <task> <result.json>
//!   mathesis-contribute mcp                MCP server on stdio for your agent
//! ```
//!
//! Claude Code: `claude mcp add mathesis -e MATHESIS_NYXID_SERVICE=mathesis -- mathesis-contribute mcp`.

mod client;
mod mcp;
mod rules;

use std::process::ExitCode;

use client::Client;

fn setting(name: &str) -> Result<String, std::env::VarError> {
    std::env::var(format!("MATHESIS_{name}")).or_else(|_| std::env::var(format!("WISHPOOL_{name}")))
}
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let client = if let Ok(service) = setting("NYXID_SERVICE") {
        if service.is_empty() || service.starts_with('-') {
            eprintln!(
                "MATHESIS_NYXID_SERVICE (or legacy WISHPOOL_NYXID_SERVICE) must name a connected service"
            );
            return ExitCode::from(2);
        }
        Client::via_nyxid(&service)
    } else if let Ok(url) = setting("URL") {
        Client::new(&url, &setting("TOKEN").unwrap_or_default())
    } else {
        eprintln!(
            "set MATHESIS_NYXID_SERVICE for a connected NyxID service, or MATHESIS_URL (and MATHESIS_TOKEN for private operations); legacy WISHPOOL variables are also supported"
        );
        return ExitCode::from(2);
    };
    let print = |result: client::ApiResult<serde_json::Value>| match result {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_default()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    };
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["formalization-opportunities"] => {
            print(client.formalization_opportunities(&serde_json::json!({})))
        }
        ["formalization-opportunities", query] => {
            print(client.formalization_opportunities(&serde_json::json!({"q": query})))
        }
        ["formalizations"] => print(client.get("/formalizations")),
        ["formalization-project", id] => print(client.formalization_project(id)),
        ["papers"] => print(client.papers(&serde_json::json!({}))),
        ["papers", query] => print(client.papers(&serde_json::json!({"q": query}))),
        ["paper", record] => print(client.paper(record)),
        ["review", id] => print(client.review(id)),
        ["verification-bundle", record, claim, attempt] => {
            print(client.verification_bundle(record, claim, attempt))
        }
        ["respond-review", id, file] => match std::fs::read_to_string(file)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
        {
            Ok(body) => print(client.respond_review(id, &body)),
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
        ["formalization", id] => print(client.formalization(id)),
        ["verify-formalization", id, record, claim] => {
            print(client.verify_formalization(id, record, claim))
        }
        ["submit-formalization", file] => match std::fs::read_to_string(file)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
        {
            Ok(body) => print(client.post("/formalizations", &body)),
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
        ["conjectures"] => print(client.conjectures()),
        ["target" | "statement-target", record, claim] => match client.target(record, claim) {
            Ok(value) => match value["target"]["lean"].as_str() {
                Some(lean) => match std::fs::write("Target.lean", lean) {
                    Ok(()) => {
                        println!("Wrote Target.lean (digest {})", value["target"]["digest"]);
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        ExitCode::FAILURE
                    }
                },
                None => {
                    eprintln!("the author has not confirmed a target");
                    ExitCode::FAILURE
                }
            },
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        },
        ["report", record, file] => match std::fs::read_to_string(file)
            .map_err(|e| e.to_string())
            .and_then(|text| serde_json::from_str(&text).map_err(|e| e.to_string()))
        {
            Ok(body) => print(client.report(record, &body)),
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(2)
            }
        },
        ["report-status", id] => print(client.report_status(id)),
        ["attempt-status", id] => print(client.attempt_status(id)),
        ["attempt" | "statement-attempt", record, claim, file] => {
            submit_attempt(&client, record, claim, file, None)
        }
        [
            "attempt" | "statement-attempt",
            record,
            claim,
            file,
            "--as-agent",
            name,
        ] => submit_attempt(&client, record, claim, file, Some(name)),
        ["tasks"] => print(client.open_tasks(None, 50)),
        ["tasks", kind] => print(client.open_tasks(Some(kind), 50)),
        ["show", task] => print(client.context(task)),
        ["lease", task] => print(client.lease(task)),
        ["release", task] => print(client.release(task)),
        ["submit", task, file] => match std::fs::read_to_string(file)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
        {
            Ok(body) => print(client.submit(task, &body)),
            Err(error) => {
                eprintln!("cannot read {file}: {error}");
                ExitCode::from(2)
            }
        },
        ["mcp"] => match mcp::serve(&client) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("mcp: {error}");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!(
                "usage: mathesis-contribute formalization-opportunities [query] | formalizations | formalization-project <id> | papers [query] | paper <record> | review <submission-id> | respond-review <submission-id> <response.json> | verification-bundle <record> <claim> <attempt> | submit-formalization <deposit.json> | formalization <id> | verify-formalization <id> <record> <claim> | conjectures | target <record> <claim> | attempt <record> <claim> <file> [--as-agent NAME] | attempt-status <id> | report <record> <report.json> | report-status <id> | tasks [kind] | show <task> | lease <task> | release <task> | submit <task> <result.json> | mcp"
            );
            ExitCode::from(2)
        }
    }
}

fn submit_attempt(
    client: &Client,
    record: &str,
    claim: &str,
    file: &str,
    agent: Option<&str>,
) -> ExitCode {
    let result = std::fs::read_to_string(file)
        .map_err(|e| e.to_string())
        .and_then(|s| {
            if s.len() > 1_048_576 {
                return Err("solution exceeds 1 MB".into());
            }
            client
                .attempt(record, claim, &s, agent)
                .map_err(|e| e.to_string())
        });
    match result {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_default()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn mcp_lists_tools_and_rejects_unknown_methods() {
        let client = Client::new("http://127.0.0.1:9", "dev:test");
        let init = mcp::handle(
            &client,
            &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
        )
        .unwrap();
        assert_eq!(init["result"]["capabilities"]["tools"], json!({}));
        let tools = mcp::handle(
            &client,
            &json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
        )
        .unwrap();
        let names: Vec<&str> = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "formalization-opportunities",
                "formalizations",
                "formalization-project",
                "papers",
                "paper",
                "review",
                "respond-review",
                "verification-bundle",
                "submit-formalization",
                "formalization",
                "verify-formalization",
                "report",
                "report-status",
                "conjectures",
                "target",
                "attempt",
                "attempt-status",
                "list_tasks",
                "lease_task",
                "get_task_context",
                "submit_contribution",
                "release_task"
            ]
        );
        assert!(
            mcp::handle(
                &client,
                &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
            )
            .is_none()
        );
        let unknown = mcp::handle(
            &client,
            &json!({ "jsonrpc": "2.0", "id": 3, "method": "resources/list" }),
        )
        .unwrap();
        assert_eq!(unknown["error"]["code"], -32601);
        // A tool error is reported in-band, not as a protocol error.
        let failed = mcp::handle(&client, &json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "lease_task", "arguments": {} } })).unwrap();
        assert_eq!(failed["result"]["isError"], true);
    }

    #[test]
    fn rules_cover_every_kind() {
        for kind in ["judge_escape", "literature_check", "probe"] {
            assert!(!rules::for_kind(kind).is_empty());
        }
    }
}
