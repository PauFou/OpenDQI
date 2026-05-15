//! `opendqi completions <shell>` and `opendqi man` — generate shell
//! completion scripts and a man page from the clap command tree.
//! Both write to stdout only; no filesystem or network side effects.

use std::io;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Args, CommandFactory};
use clap_complete::Shell;

/// Arguments for `opendqi completions`.
#[derive(Args)]
pub struct CompletionsArgs {
    /// Target shell. Generated script is written to stdout — redirect
    /// it to the shell's completion directory.
    pub shell: Shell,
}

/// `opendqi completions <shell>` — emit the completion script.
pub fn run_completions(args: CompletionsArgs) -> Result<ExitCode> {
    let mut cmd = crate::Cli::command();
    clap_complete::generate(args.shell, &mut cmd, "opendqi", &mut io::stdout());
    Ok(ExitCode::SUCCESS)
}

/// `opendqi man` — render the top-level man page (roff) to stdout.
pub fn run_man() -> Result<ExitCode> {
    let cmd = crate::Cli::command();
    let man = clap_mangen::Man::new(cmd);
    let mut out = Vec::new();
    man.render(&mut out).context("rendering man page")?;
    use std::io::Write;
    io::stdout()
        .write_all(&out)
        .context("writing man page to stdout")?;
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    #[test]
    fn cli_command_tree_is_valid() {
        // clap's built-in invariant checker — catches arg conflicts,
        // duplicate names, etc. across the whole command tree.
        crate::Cli::command().debug_assert();
    }

    #[test]
    fn completions_generate_without_panicking() {
        use clap_complete::Shell;
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            let mut cmd = crate::Cli::command();
            let mut buf = Vec::new();
            clap_complete::generate(shell, &mut cmd, "opendqi", &mut buf);
            assert!(!buf.is_empty(), "{shell:?} produced an empty script");
        }
    }

    #[test]
    fn man_renders_without_panicking() {
        let cmd = crate::Cli::command();
        let mut buf = Vec::new();
        clap_mangen::Man::new(cmd).render(&mut buf).unwrap();
        assert!(!buf.is_empty());
    }
}
