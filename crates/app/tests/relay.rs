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
