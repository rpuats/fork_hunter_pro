use chrono::{DateTime, Utc};
use parsers::diagnostics::{
    run_runtime_diagnostics, RuntimeDiagnosticsRun, DEFAULT_RUNTIME_DIAGNOSTIC_SLUGS,
    LIVE_THRESHOLD, PREMATCH_THRESHOLD,
};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Default)]
struct CliOptions {
    requested_slugs: Vec<String>,
    json_stdout: bool,
    json_outs: Vec<PathBuf>,
    json_out_dir: Option<PathBuf>,
    summary_only: bool,
    strict_exit: bool,
    help: bool,
}

impl CliOptions {
    fn parse() -> Self {
        let mut args = std::env::args().skip(1);
        let mut options = Self::default();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" => options.help = true,
                "--json-stdout" => options.json_stdout = true,
                "--json-out" => {
                    let path = args
                        .next()
                        .unwrap_or_else(|| panic!("--json-out requires a file path argument"));
                    options.json_outs.push(PathBuf::from(path));
                }
                "--json-out-dir" => {
                    let path = args.next().unwrap_or_else(|| {
                        panic!("--json-out-dir requires a directory path argument")
                    });
                    options.json_out_dir = Some(PathBuf::from(path));
                }
                "--summary-only" => options.summary_only = true,
                "--strict-exit" => options.strict_exit = true,
                _ => options.requested_slugs.push(arg),
            }
        }

        options
    }
}

#[tokio::main]
async fn main() {
    let options = CliOptions::parse();
    if options.help {
        print_help();
        return;
    }

    let effective = if options.requested_slugs.is_empty() {
        DEFAULT_RUNTIME_DIAGNOSTIC_SLUGS
            .iter()
            .map(|slug| (*slug).to_string())
            .collect::<Vec<_>>()
    } else {
        options.requested_slugs.clone()
    };

    let client = Arc::new(
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .expect("client"),
    );

    let run = run_runtime_diagnostics(client, &options.requested_slugs).await;

    if options.summary_only {
        print_summary_line(&run);
    } else {
        println!("runtime-only parser diagnostics");
        println!(
            "thresholds: live >= {}, prematch >= {}",
            LIVE_THRESHOLD, PREMATCH_THRESHOLD
        );
        println!("slugs: {}", effective.join(", "));
        println!();
        println!("slug,total,live,prematch,live_missing,prematch_missing,live_ok,prematch_ok,pass,runtime_only,duration_ms,error");

        for report in &run.reports {
            println!(
                "{},{},{},{},{},{},{},{},{},{},{},{}",
                report.bookmaker_slug,
                report.total_events,
                report.live_events,
                report.prematch_events,
                report.live_kpi.missing,
                report.prematch_kpi.missing,
                report.live_threshold_met,
                report.prematch_threshold_met,
                report.passed,
                report.runtime_only,
                report.duration_ms,
                report
                    .error
                    .as_deref()
                    .unwrap_or_default()
                    .replace(',', ";")
            );
        }

        println!();
        print_summary_line(&run);
    }

    maybe_write_json_output(
        &run,
        options.json_stdout,
        &options.json_outs,
        options.json_out_dir.as_ref(),
    );

    std::process::exit(exit_code_for_run(&run, options.strict_exit));
}

fn print_summary_line(run: &RuntimeDiagnosticsRun) {
    println!(
        "nightly_summary,status={},exit_code={},pass_reports={},fail_reports={},live_gap_total={},prematch_gap_total={},failing_slugs={}",
        run.nightly.status,
        run.nightly.exit_code,
        run.aggregate.passed_reports,
        run.aggregate.failed_reports,
        run.nightly.live_gap_total,
        run.nightly.prematch_gap_total,
        join_failing_slugs(&run.nightly.failing_slugs),
    );
}

fn print_help() {
    println!(
        "runtime_parser_diagnostics [OPTIONS] [SLUG ...]\n\nOptions:\n  --strict-exit            Return nightly KPI exit code (0 pass, 2 fail)\n  --summary-only           Print only nightly summary line\n  --json-stdout            Print pretty JSON payload to stdout\n  --json-out <PATH>        Write machine-readable output (.json overwrite, .jsonl append); repeatable\n  --json-out-dir <DIR>     Write timestamped .json snapshot and append to history .jsonl in DIR\n  -h, --help               Show this help\n\nDefaults:\n  - Slugs: {}\n  - Live KPI >= {}\n  - Prematch KPI >= {}",
        DEFAULT_RUNTIME_DIAGNOSTIC_SLUGS.join(", "),
        LIVE_THRESHOLD,
        PREMATCH_THRESHOLD,
    );
}

fn join_failing_slugs(failing_slugs: &[String]) -> String {
    if failing_slugs.is_empty() {
        "-".to_string()
    } else {
        failing_slugs.join("|")
    }
}

fn exit_code_for_run(run: &RuntimeDiagnosticsRun, strict_exit: bool) -> i32 {
    if strict_exit {
        run.nightly.exit_code
    } else {
        0
    }
}

fn maybe_write_json_output(
    run: &RuntimeDiagnosticsRun,
    json_stdout: bool,
    json_outs: &[PathBuf],
    json_out_dir: Option<&PathBuf>,
) {
    if let Some(dir) = json_out_dir {
        let snapshot_path = timestamped_snapshot_path(dir.as_path(), run.generated_at);
        let history_path = history_artifact_path(dir.as_path());
        write_json_output(&snapshot_path, run);
        write_json_output(&history_path, run);
        println!();
        println!("json artifact: {}", snapshot_path.display());
        println!("json artifact: {}", history_path.display());
    }

    for path in json_outs {
        write_json_output(path, run);
        println!();
        println!("json artifact: {}", path.display());
    }

    if json_stdout {
        println!();
        println!(
            "{}",
            serde_json::to_string_pretty(run).expect("serialize runtime diagnostics json")
        );
    }
}

fn timestamped_snapshot_path(base_dir: &Path, generated_at: DateTime<Utc>) -> PathBuf {
    let timestamp = generated_at.format("%Y%m%d_%H%M%S");
    base_dir.join(format!("runtime_parser_diagnostics_{timestamp}.json"))
}

fn history_artifact_path(base_dir: &Path) -> PathBuf {
    base_dir.join("runtime_parser_diagnostics_history.jsonl")
}

fn write_json_output(path: &PathBuf, run: &RuntimeDiagnosticsRun) {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("json") => write_json_snapshot(path, run),
        _ => append_json_line(path, run),
    }
}

fn write_json_snapshot(path: &PathBuf, run: &RuntimeDiagnosticsRun) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create json output directory");
    }

    std::fs::write(
        path,
        serde_json::to_vec_pretty(run).expect("serialize runtime diagnostics json"),
    )
    .expect("write runtime diagnostics json snapshot");
}

fn append_json_line(path: &PathBuf, run: &RuntimeDiagnosticsRun) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create json output directory");
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("open json output file");

    writeln!(
        file,
        "{}",
        serde_json::to_string(run).expect("serialize runtime diagnostics json line")
    )
    .expect("write runtime diagnostics json line");
}

#[cfg(test)]
mod tests {
    use super::{
        exit_code_for_run, history_artifact_path, join_failing_slugs, timestamped_snapshot_path,
    };
    use chrono::{TimeZone, Utc};
    use parsers::diagnostics::{
        RuntimeCountReport, RuntimeDiagnosticsAggregate, RuntimeDiagnosticsNightlySummary,
        RuntimeDiagnosticsRun, RuntimeDiagnosticsSummary,
    };
    use std::path::Path;

    #[test]
    fn builds_timestamped_snapshot_path() {
        let generated_at = Utc.with_ymd_and_hms(2026, 4, 13, 8, 58, 27).unwrap();
        let path = timestamped_snapshot_path(Path::new("artifacts/runtime"), generated_at);

        assert_eq!(
            path,
            Path::new("artifacts/runtime/runtime_parser_diagnostics_20260413_085827.json")
        );
    }

    #[test]
    fn builds_history_artifact_path() {
        let path = history_artifact_path(Path::new("artifacts/runtime"));

        assert_eq!(
            path,
            Path::new("artifacts/runtime/runtime_parser_diagnostics_history.jsonl")
        );
    }

    #[test]
    fn joins_failing_slugs_for_summary_output() {
        assert_eq!(join_failing_slugs(&[]), "-");
        assert_eq!(
            join_failing_slugs(&["winline".into(), "betcity".into()]),
            "winline|betcity"
        );
    }

    #[test]
    fn strict_exit_uses_nightly_status_code() {
        let run = RuntimeDiagnosticsRun {
            generated_at: Utc::now(),
            summary: RuntimeDiagnosticsSummary {
                requested_slugs: vec!["winline".into()],
                total_reports: 1,
                passed_reports: 0,
                failed_reports: 1,
                live_threshold: 150,
                prematch_threshold: 3000,
            },
            aggregate: RuntimeDiagnosticsAggregate {
                total_events: 0,
                live_events: 0,
                prematch_events: 0,
                live_threshold_passes: 0,
                prematch_threshold_passes: 0,
                passed_reports: 0,
                failed_reports: 1,
            },
            nightly: RuntimeDiagnosticsNightlySummary {
                status: "fail",
                exit_code: 2,
                failing_slugs: vec!["winline".into()],
                live_gap_total: 150,
                prematch_gap_total: 3000,
            },
            reports: vec![RuntimeCountReport {
                bookmaker_slug: "winline".into(),
                total_events: 0,
                live_events: 0,
                prematch_events: 0,
                live_kpi: parsers::diagnostics::RuntimeKpiCheck {
                    actual: 0,
                    target: 150,
                    missing: 150,
                    passed: false,
                },
                prematch_kpi: parsers::diagnostics::RuntimeKpiCheck {
                    actual: 0,
                    target: 3000,
                    missing: 3000,
                    passed: false,
                },
                live_threshold_met: false,
                prematch_threshold_met: false,
                passed: false,
                runtime_only: true,
                duration_ms: 10,
                error: Some("boom".into()),
            }],
        };

        assert_eq!(exit_code_for_run(&run, false), 0);
        assert_eq!(exit_code_for_run(&run, true), 2);
    }
}
