use std::process::{Command, Output};

fn cli(args: &[&str], env: &[(&str, &str)]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_update_dns_record"))
        .env_clear()
        .envs(env.iter().copied())
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn required_options_are_reported_without_environment() {
    let output = cli(&[], &[]);
    assert_eq!(output.status.code(), Some(2));
    let error = String::from_utf8(output.stderr).unwrap();
    for option in ["--api-token", "--zone-id", "--record-name"] {
        assert!(error.contains(option), "{error}");
    }
}

#[test]
fn missing_interfaces_fail_before_logging_or_network() {
    let output = cli(
        &["-a", "token", "-z", "zone", "-r", "name", "--dry-run"],
        &[],
    );
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        output.stderr,
        b"At least one of v4_int or v6_int must be set\n"
    );
    assert!(output.stdout.is_empty());
}

#[test]
fn required_values_can_come_from_environment() {
    let output = cli(
        &[],
        &[
            ("CLOUDFLARE_API_TOKEN", "token"),
            ("CLOUDFLARE_ZONE_ID", "zone"),
            ("CLOUDFLARE_RECORD_NAME", "name"),
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        output.stderr,
        b"At least one of v4_int or v6_int must be set\n"
    );
}

#[test]
fn command_line_overrides_invalid_environment_boolean() {
    let output = cli(
        &["-a", "token", "-z", "zone", "-r", "name", "--dry-run"],
        &[("CLOUDFLARE_DRY_RUN", "invalid")],
    );
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        output.stderr,
        b"At least one of v4_int or v6_int must be set\n"
    );
}

#[test]
fn help_and_version_do_not_initialize_runtime_dependencies() {
    let help = cli(&["--help"], &[]);
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    for option in ["--v4-int", "--v6-int", "--dry-run"] {
        assert!(text.contains(option));
    }
    let version = cli(&["--version"], &[]);
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8(version.stdout).unwrap().trim(),
        format!("update_dns_record {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn help_hides_token_values_but_keeps_environment_variable_name() {
    for help_flag in ["--help", "-h"] {
        for token in ["review-only-dummy-token", "dummy token with spaces-日本語"] {
            let output = cli(&[help_flag], &[("CLOUDFLARE_API_TOKEN", token)]);
            assert!(output.status.success());
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.contains("CLOUDFLARE_API_TOKEN"));
            assert!(!text.contains(token));
            assert!(output.stderr.is_empty());
        }
    }
}
