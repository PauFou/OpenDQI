//! `opendqi smtp-test` — standalone helper that validates an
//! `--email-config` YAML and (by default) sends a hello-world test
//! email so an operator can catch SMTP misconfigurations before
//! deploying a scheduled scan.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Args;
use opendqi_report::{send_smtp_test, SmtpConfig};
use tracing::info;

/// Arguments for `opendqi smtp-test`.
#[derive(Args)]
pub struct SmtpTestArgs {
    /// Path to the SMTP YAML configuration. Same schema as
    /// `--email-config` on the scan commands.
    #[arg(long, value_name = "PATH")]
    pub email_config: PathBuf,
    /// Do not send. Validate the YAML, print the parsed config (sans
    /// password), and check that the password env var is set. Exits
    /// 0 if everything looks good.
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(args: SmtpTestArgs) -> Result<ExitCode> {
    let cfg = SmtpConfig::from_yaml_file(&args.email_config)
        .with_context(|| format!("loading SMTP config from {}", args.email_config.display()))?;

    println!("SMTP config loaded from {}", args.email_config.display());
    println!("  enabled:        {}", cfg.enabled);
    println!("  host:           {}", cfg.host);
    println!("  port:           {}", cfg.port);
    println!("  use_tls:        {}", cfg.use_tls);
    println!("  username:       {}", cfg.username);
    println!("  password_env:   {}", cfg.password_env);
    println!("  from:           {}", cfg.from);
    println!("  to ({} addr):    {}", cfg.to.len(), cfg.to.join(", "));

    if !cfg.enabled {
        println!("\n(enabled=false → not contacting the SMTP server. Set enabled: true to actually send.)");
        return Ok(ExitCode::SUCCESS);
    }

    if std::env::var(&cfg.password_env).is_err() {
        return Err(anyhow::anyhow!(
            "SMTP password env var '{}' is not set — populate it (e.g. via .env) before running smtp-test.",
            cfg.password_env
        ));
    }

    if args.dry_run {
        println!("\nDry-run OK: YAML valid, env var present. No email sent.");
        return Ok(ExitCode::SUCCESS);
    }

    println!("\nSending a test email …");
    let sent = send_smtp_test(&cfg)?;
    if sent {
        info!(to = ?cfg.to, "test email sent");
        println!(
            "✓ Test email delivered to {} recipient(s). Check your inbox.",
            cfg.to.len()
        );
        Ok(ExitCode::SUCCESS)
    } else {
        // Should be unreachable because `enabled: false` is handled above.
        println!("(skipped: enabled=false)");
        Ok(ExitCode::SUCCESS)
    }
}
