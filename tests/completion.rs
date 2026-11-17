use std::process::Command;

fn path_with_test_binary() -> std::ffi::OsString {
    let mut paths = vec![std::path::PathBuf::from(env!("CARGO_BIN_EXE_serper"))
        .parent()
        .unwrap()
        .to_path_buf()];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    std::env::join_paths(paths).unwrap()
}

#[test]
fn generates_completion_scripts_without_an_api_key() {
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        let output = Command::new(env!("CARGO_BIN_EXE_serper"))
            .args(["completion", shell])
            .env_remove("SERPER_KEY")
            .env_remove("SERPER_COMPLETE")
            .output()
            .unwrap();
        assert!(output.status.success(), "{shell}: {output:?}");
        let script = String::from_utf8(output.stdout).unwrap();
        assert!(script.contains("serper"), "empty script for {shell}");
    }
}

#[test]
fn completion_rejects_an_unknown_shell() {
    let output = Command::new(env!("CARGO_BIN_EXE_serper"))
        .args(["completion", "unknown"])
        .env_remove("SERPER_KEY")
        .env_remove("SERPER_COMPLETE")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown"));
}

#[test]
fn dynamic_shells_complete_option_and_positional_values() {
    for shell in ["fish", "powershell", "elvish"] {
        for (words, expected) in [
            (vec!["serper", "web", "-t", "w"], "week"),
            (vec!["serper", "web", "--search-time", "w"], "w"),
            (vec!["serper", "completion", "b"], "bash"),
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_serper"))
                .arg("--")
                .args(&words)
                .env("SERPER_COMPLETE", shell)
                .env("_CLAP_COMPLETE_INDEX", (words.len() - 1).to_string())
                .env_remove("SERPER_KEY")
                .output()
                .unwrap();
            assert!(output.status.success(), "{shell}: {words:?}: {output:?}");
            let candidates = String::from_utf8(output.stdout).unwrap();
            assert!(
                candidates
                    .lines()
                    .any(|line| line.split('\t').next() == Some(expected)),
                "{shell}: {words:?}: {candidates:?}"
            );
        }
    }
}

#[test]
fn powershell_completes_values_without_evaluating_input() {
    let script = r#"
        $ErrorActionPreference = 'Stop'
        & $env:SERPER_TEST_BIN completion powershell | Out-String | Invoke-Expression
        $env:SERPER_COMPLETE = 'previous-value'
        function Check($line, $expected) {
            $result = [System.Management.Automation.CommandCompletion]::CompleteInput(
                $line, $line.Length, $null
            )
            if ($result.CompletionMatches.CompletionText -notcontains $expected) {
                throw "Missing $expected for $line"
            }
            if ($env:SERPER_COMPLETE -ne 'previous-value') {
                throw 'Completion did not restore the environment'
            }
        }
        Check 'serper web --se' '--search-time'
        Check 'serper web -t w' 'week'
        Check 'serper web --search-time w' 'w'
        Check "serper web -q 'hello world' -t we" 'week'
        Check "serper web -t 'we" "'week'"
        Check "serper completion 'b" "'bash'"
        Check 'serper web -q $(throw "input was evaluated") -t w' 'week'
        Check 'serper completion b' 'bash'
    "#;
    let result = Command::new("pwsh")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("PATH", path_with_test_binary())
        .env("SERPER_TEST_BIN", env!("CARGO_BIN_EXE_serper"))
        .env_remove("SERPER_KEY")
        .env_remove("SERPER_COMPLETE")
        .output();
    match result {
        Ok(output) => assert!(output.status.success(), "{output:?}"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("could not run PowerShell: {error}"),
    }
}

#[test]
fn fish_completes_option_and_positional_values() {
    let result = Command::new("fish")
        .args([
            "-N",
            "-c",
            "serper completion fish | source; complete -C 'serper web -t w'; complete -C 'serper completion b'",
        ])
        .env("PATH", path_with_test_binary())
        .env_remove("SERPER_KEY")
        .env_remove("SERPER_COMPLETE")
        .output();
    match result {
        Ok(output) => {
            assert!(output.status.success(), "{output:?}");
            let candidates = String::from_utf8(output.stdout).unwrap();
            for expected in ["week", "w", "bash"] {
                assert!(
                    candidates
                        .lines()
                        .any(|line| line.split('\t').next() == Some(expected)),
                    "missing {expected}: {candidates:?}"
                );
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("could not run Fish: {error}"),
    }
}

#[cfg(unix)]
#[test]
fn bash_completes_commands_options_and_search_time_values() {
    let script = r#"
        source <("$SERPER_BIN" completion bash)
        contains() {
            local item
            for item in "${COMPREPLY[@]}"; do
                [[ "$item" == "$1" ]] && return 0
            done
            return 1
        }
        COMP_WORDS=(serper sc); COMP_CWORD=1
        _serper serper sc serper
        contains scrape || exit 1
        COMP_WORDS=(serper web --se); COMP_CWORD=2
        _serper serper --se web
        contains --search-time || exit 1
        COMP_WORDS=(serper web -t w); COMP_CWORD=3
        _serper serper w -t
        contains week && contains w || exit 1
        COMP_WORDS=(serper scrape --no-m); COMP_CWORD=2
        _serper serper --no-m scrape
        contains --no-markdown || exit 1
    "#;
    let output = Command::new("bash")
        .args(["-c", script])
        .env("SERPER_BIN", env!("CARGO_BIN_EXE_serper"))
        .env_remove("SERPER_KEY")
        .env_remove("SERPER_COMPLETE")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}
