use super::*;

#[test]
fn plain_values_are_single_quoted() {
    assert_eq!(sh_quote("claude"), "'claude'");
    assert_eq!(sh_quote("my agent"), "'my agent'");
}

#[test]
fn shell_metacharacters_are_neutralized() {
    // Each stays inside one quoted word — no command runs.
    assert_eq!(sh_quote("a; rm -rf /"), "'a; rm -rf /'");
    assert_eq!(sh_quote("$(whoami)"), "'$(whoami)'");
    assert_eq!(sh_quote("a && b"), "'a && b'");
}

#[test]
fn embedded_single_quotes_are_escaped() {
    // The close/escape/reopen dance keeps the value a single shell token.
    assert_eq!(sh_quote("x'; rm -rf /;'"), "'x'\\''; rm -rf /;'\\'''");
}

#[test]
fn minimize_squeezes_whitespace_and_blank_lines() {
    let input = "Fix   the   bug  \n\n\n\nin   the parser\n";
    assert_eq!(minimize_prompt(input), "Fix the bug\n\nin the parser");
}

#[test]
fn minimize_preserves_indentation_and_content() {
    // Leading indent survives so pasted code keeps its shape; only runs after
    // the indent collapse. No words are dropped.
    let input = "    let x =    1;\n\thelp   me   please";
    assert_eq!(minimize_prompt(input), "    let x = 1;\n\thelp me please");
}

#[test]
fn minimize_trims_outer_blank_lines() {
    assert_eq!(minimize_prompt("\n\n  hello  \n\n"), "  hello");
}

#[test]
fn split_args_tokenizes_on_whitespace() {
    assert_eq!(
        split_args("--dangerously-skip-permissions --foo"),
        vec!["--dangerously-skip-permissions", "--foo"]
    );
}

#[test]
fn split_args_keeps_quoted_values_together() {
    assert_eq!(
        split_args("--append-system-prompt \"be terse\" --x"),
        vec!["--append-system-prompt", "be terse", "--x"]
    );
    assert_eq!(split_args("   "), Vec::<String>::new());
}

#[test]
fn extract_json_pulls_the_object_from_a_wrapped_reply() {
    let reply = "Sure! Here's the team:\n```json\n{\"name\":\"web\",\"members\":[{\"name\":\"lead\"}]}\n```\nHope that helps.";
    let json = extract_json(reply).unwrap();
    let spec: TeamSpec = serde_json::from_str(json).unwrap();
    assert_eq!(spec.name, "web");
    assert_eq!(spec.members.len(), 1);
}

#[test]
fn extract_json_balances_nested_braces() {
    let reply = "{\"a\":{\"b\":1},\"c\":2} trailing junk }";
    assert_eq!(extract_json(reply).unwrap(), "{\"a\":{\"b\":1},\"c\":2}");
    assert!(extract_json("no json here").is_none());
}

#[test]
fn launch_member_quotes_hostile_values() {
    let cmd = launch_member("x'; rm -rf /;'", "worker role", "$(whoami)", true, false);
    // Every interpolated value stays a single quoted shell token.
    assert!(cmd.contains(" launch 'x'\\''; rm -rf /;'\\'''"), "member not quoted: {cmd}");
    assert!(cmd.contains("--role 'worker role'"), "role not quoted: {cmd}");
    assert!(cmd.contains("--agent '$(whoami)'"), "agent not quoted: {cmd}");
    assert!(cmd.contains(" --lead"));
}

#[test]
fn launch_member_omits_empty_agent_flag() {
    let cmd = launch_member("lead", "supervisor", "  ", false, true);
    assert!(!cmd.contains("--agent"));
    assert!(cmd.contains("--optimize"));
    assert!(cmd.contains(" launch 'lead' --role 'supervisor'"));
}
