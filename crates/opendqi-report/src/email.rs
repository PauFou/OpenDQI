//! SMTP email notifications for scan reports (CLAUDE.md priority #8).
//!
//! Loads an `SmtpConfig` from a YAML file passed via `--email-config`
//! on `opendqi {emir,sftr} scan`, builds a MIME multipart email
//! containing the HTML report (inline) plus `summary.json` and
//! `issues.csv` as attachments, and sends via lettre's SMTP transport.
//!
//! The SMTP password is **never** stored in the YAML — the config
//! carries a `password_env` field naming an environment variable
//! (default `OPENDQI_SMTP_PASS`) from which the password is read at
//! send time.

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use lettre::message::header::ContentType;
use lettre::message::{Attachment, Body as LettreBody, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use opendqi_core::ScanSummary;
use serde::{Deserialize, Serialize};

/// SMTP configuration loaded from `--email-config <yml>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    /// Toggle: when `false`, send is a no-op (useful for staging).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// SMTP host (e.g. `smtp.gmail.com`).
    pub host: String,
    /// SMTP port (typically 587 for STARTTLS, 465 for SMTPS).
    pub port: u16,
    /// Whether to upgrade to TLS via STARTTLS (port 587). When `false`
    /// the transport is plain SMTP (use only for local relays).
    #[serde(default = "default_true")]
    pub use_tls: bool,
    /// SMTP username for authentication.
    pub username: String,
    /// Name of the environment variable holding the SMTP password.
    /// Defaults to `OPENDQI_SMTP_PASS`. Never store the password in
    /// YAML.
    #[serde(default = "default_password_env")]
    pub password_env: String,
    /// `From:` header (e.g. `OpenDQI <reports@example.com>`).
    pub from: String,
    /// `To:` recipients.
    pub to: Vec<String>,
    /// Subject template. Supports `{regime}`, `{records}`,
    /// `{critical}`, `{high}`, `{score}` placeholders.
    #[serde(default = "default_subject")]
    pub subject_template: String,
}

fn default_true() -> bool {
    true
}
fn default_password_env() -> String {
    "OPENDQI_SMTP_PASS".into()
}
fn default_subject() -> String {
    "OpenDQI scan {regime} — {critical} critical / {high} high (score {score:.1})".into()
}

impl SmtpConfig {
    /// Read an `SmtpConfig` from a YAML file.
    pub fn from_yaml_file(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading SMTP config {}", path.display()))?;
        let cfg: SmtpConfig = serde_yaml::from_str(&text)
            .with_context(|| format!("parsing SMTP config {}", path.display()))?;
        Ok(cfg)
    }

    fn rendered_subject(&self, summary: &ScanSummary) -> String {
        let critical = summary
            .issues_by_severity
            .get(&opendqi_core::Severity::Critical)
            .copied()
            .unwrap_or(0);
        let high = summary
            .issues_by_severity
            .get(&opendqi_core::Severity::High)
            .copied()
            .unwrap_or(0);
        self.subject_template
            .replace("{regime}", &summary.regime.to_string().to_uppercase())
            .replace("{records}", &summary.records_processed.to_string())
            .replace("{critical}", &critical.to_string())
            .replace("{high}", &high.to_string())
            .replace("{score:.1}", &format!("{:.1}", summary.quality_score))
            .replace("{score}", &format!("{:.1}", summary.quality_score))
    }
}

/// Send the scan report via SMTP. Returns `Ok(false)` when the
/// config is `enabled: false` (no-op); `Ok(true)` on successful send.
pub fn send_report_email(
    config: &SmtpConfig,
    summary: &ScanSummary,
    html_report: &Path,
    summary_json: &Path,
    issues_csv: &Path,
) -> Result<bool> {
    if !config.enabled {
        return Ok(false);
    }
    let password = std::env::var(&config.password_env)
        .map_err(|_| anyhow!("SMTP password env var '{}' is not set", config.password_env))?;

    let html = std::fs::read_to_string(html_report)
        .with_context(|| format!("reading {}", html_report.display()))?;
    let summary_bytes = std::fs::read(summary_json)
        .with_context(|| format!("reading {}", summary_json.display()))?;
    let issues_bytes =
        std::fs::read(issues_csv).with_context(|| format!("reading {}", issues_csv.display()))?;

    let subject = config.rendered_subject(summary);
    let mut builder = Message::builder()
        .from(config.from.parse().context("parsing `from` address")?)
        .subject(subject);
    for to in &config.to {
        builder = builder.to(to.parse().with_context(|| format!("parsing `to` {to}"))?);
    }

    let mp = MultiPart::mixed()
        .singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_HTML)
                .body(html),
        )
        .singlepart(Attachment::new("summary.json".into()).body(
            LettreBody::new(summary_bytes),
            ContentType::parse("application/json").unwrap(),
        ))
        .singlepart(Attachment::new("issues.csv".into()).body(
            LettreBody::new(issues_bytes),
            ContentType::parse("text/csv").unwrap(),
        ));
    let msg = builder.multipart(mp).context("building MIME message")?;

    let creds = Credentials::new(config.username.clone(), password);
    let mailer = if config.use_tls {
        SmtpTransport::starttls_relay(&config.host)?
            .port(config.port)
            .credentials(creds)
            .build()
    } else {
        SmtpTransport::builder_dangerous(&config.host)
            .port(config.port)
            .credentials(creds)
            .build()
    };
    mailer.send(&msg).context("SMTP send failed")?;
    Ok(true)
}

/// Send a tiny hello-world email via the same SMTP transport — used
/// by `opendqi smtp-test` to validate the SMTP setup without
/// running a scan. Returns `Ok(false)` when `enabled: false`,
/// `Ok(true)` on successful send.
pub fn send_smtp_test(config: &SmtpConfig) -> Result<bool> {
    if !config.enabled {
        return Ok(false);
    }
    let password = std::env::var(&config.password_env)
        .map_err(|_| anyhow!("SMTP password env var '{}' is not set", config.password_env))?;

    let html = "<p>OpenDQI SMTP test — \
        if you can read this email your <code>--email-config</code> \
        wiring is working.</p>";

    let subject = "OpenDQI SMTP test";
    let mut builder = Message::builder()
        .from(config.from.parse().context("parsing `from` address")?)
        .subject(subject);
    for to in &config.to {
        builder = builder.to(to.parse().with_context(|| format!("parsing `to` {to}"))?);
    }
    let mp = MultiPart::mixed().singlepart(
        SinglePart::builder()
            .header(ContentType::TEXT_HTML)
            .body(html.to_string()),
    );
    let msg = builder.multipart(mp).context("building MIME message")?;

    let creds = Credentials::new(config.username.clone(), password);
    let mailer = if config.use_tls {
        SmtpTransport::starttls_relay(&config.host)?
            .port(config.port)
            .credentials(creds)
            .build()
    } else {
        SmtpTransport::builder_dangerous(&config.host)
            .port(config.port)
            .credentials(creds)
            .build()
    };
    mailer.send(&msg).context("SMTP send failed")?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_config() -> SmtpConfig {
        SmtpConfig {
            enabled: true,
            host: "smtp.example.com".into(),
            port: 587,
            use_tls: true,
            username: "user@example.com".into(),
            password_env: "TEST_OPENDQI_SMTP_PASS".into(),
            from: "OpenDQI <bot@example.com>".into(),
            to: vec!["compliance@example.com".into()],
            subject_template: default_subject(),
        }
    }

    fn dummy_summary() -> ScanSummary {
        use chrono::Utc;
        use opendqi_core::{DqDimension, Regime, Severity};
        use std::collections::BTreeMap;
        let mut by_sev: BTreeMap<Severity, u32> = BTreeMap::new();
        by_sev.insert(Severity::High, 3);
        by_sev.insert(Severity::Critical, 1);
        let now = Utc::now();
        ScanSummary {
            regime: Regime::Emir,
            files_processed: 1,
            records_processed: 50,
            issues_total: 4,
            issues_by_severity: by_sev,
            issues_by_dimension: BTreeMap::<DqDimension, u32>::new(),
            quality_score: 78.5,
            started_at: now,
            finished_at: now,
        }
    }

    #[test]
    fn subject_template_substitutes_placeholders() {
        let cfg = dummy_config();
        let s = cfg.rendered_subject(&dummy_summary());
        assert!(s.contains("EMIR"));
        assert!(s.contains("1 critical"));
        assert!(s.contains("3 high"));
        assert!(s.contains("78.5"));
    }

    #[test]
    fn disabled_config_is_a_noop() {
        let mut cfg = dummy_config();
        cfg.enabled = false;
        // Even with missing password env var, disabled config doesn't
        // touch the transport — just returns Ok(false).
        let html = std::env::temp_dir().join("opendqi-test-report.html");
        let json = std::env::temp_dir().join("opendqi-test-summary.json");
        let csv = std::env::temp_dir().join("opendqi-test-issues.csv");
        std::fs::write(&html, b"<html/>").unwrap();
        std::fs::write(&json, b"{}").unwrap();
        std::fs::write(&csv, b"x\n").unwrap();
        let res = send_report_email(&cfg, &dummy_summary(), &html, &json, &csv).unwrap();
        assert!(!res);
        std::fs::remove_file(&html).ok();
        std::fs::remove_file(&json).ok();
        std::fs::remove_file(&csv).ok();
    }

    #[test]
    fn yaml_round_trip_with_defaults() {
        let yaml =
            "host: smtp.example.com\nport: 587\nusername: u\nfrom: From <a@b.c>\nto: [t@b.c]\n";
        let cfg: SmtpConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.host, "smtp.example.com");
        assert!(cfg.enabled);
        assert!(cfg.use_tls);
        assert_eq!(cfg.password_env, "OPENDQI_SMTP_PASS");
    }

    #[test]
    fn smtp_test_disabled_is_a_noop() {
        let mut cfg = dummy_config();
        cfg.enabled = false;
        // Missing password env is fine because disabled returns early.
        let res = send_smtp_test(&cfg).unwrap();
        assert!(!res);
    }
}
