# EMIR ↔ SFTR coverage parity

This document is the **comparative audit** between OpenDQI's EMIR and
SFTR check catalogs. It exists for two reasons:

1. **Maintenance**: when a new check is added on one side, the
   maintainer should consult this doc to decide whether the same
   semantic applies to the other regime.
2. **Onboarding**: a reader who sees a check on EMIR but not on SFTR
   (or vice versa) can confirm here whether that asymmetry is a gap
   to close or a deliberate "regime-specific" decision.

Updated for Phase 7.6.

## Catalog counts per family

| Family | EMIR | SFTR | Reason for the gap |
|---|---|---|---|
| `COMP.*` (completeness) | 18 | 9 | EMIR carries deriv-specific completeness (notional currency, valuation timestamp, clearing status, nature, trading capacity, intragroup indicator, …). |
| `VLD.*` (validity / enum) | 23 | 13 | EMIR adds asset-class deep enums (commodity base, credit sector, corporate sector, reporting obligation), CCP LEI format, valuation_type, trading_capacity. None apply to SFTs. |
| `ACC.*` (accuracy / magnitude) | 13 | 5 | Greek letters (delta/gamma/vega), price negative for non-IR/CR, IR/CR/EQ/FX/CO requires-underlying / requires-leg2-currency are deriv-only. SFTR has `ABNORMAL_MATURITY`, `LOAN_ABNORMAL_MAGNITUDE`, `NEGATIVE_LOAN`, `NEGATIVE_COLLATERAL`, `HAIRCUT_OUT_OF_RANGE`, `LENDING_FEE_NEGATIVE`. |
| `CON.*` (consistency / cross-field) | 27 | 16 | Leg1/leg2 (4), MTM change requires valuation, valuation after termination/reporting, IM/VM needs collateral portfolio, NCLR forbids CCP, POSC requires portfolio, MARU requires margin/portfolio, hedging requires NFC, commercial-or-treasury requires NFC are all deriv-only. SFTR adds `MATURITY_BEFORE_EFFECTIVE`, `SETTLEMENT_BEFORE_EXECUTION`, `REUSE_INDICATOR_REQUIRES_PORTFOLIO`, `REUU_REQUIRES_REUSE_INDICATOR`, `LOAN_COLL_CURRENCY_MISMATCH`, `LOAN_NEEDS_CURRENCY`, `COLL_NEEDS_CURRENCY`, `REBATE_REQUIRES_REPO_OR_BSB`, `LENDING_FEE_REQUIRES_SLEB`, `COLU_REQUIRES_PORTFOLIO`, `NEWT_FORBIDS_*`, `ETRM_REQUIRES_TERMINATION_DATE`, `EVENT_BEFORE_EXECUTION` ¹, `REPORTING_BEFORE_EXECUTION` ¹, `MATURITY_IN_PAST` ¹, `TERMINATION_AFTER_MATURITY` ¹, `MODI_PRESERVES_UTI` ¹, `ACTION_EVENT_COMPATIBILITY` ¹. |
| `UNI.*` (uniqueness) | 1 | 1 | `DUPLICATE_UTI` on both sides. |
| `TIM.*` (timeliness) | 2 | 2 | `LATE_REPORTING` on both sides; SFTR adds `LATE_REPORTING_SETTLEMENT` ¹. |
| `FBK.*` (TR feedback `auth.092`) | 4 | — | EMIR-only. SFTR has no rejection-feedback message (real `auth.080` is a reconciliation status advice → `SFTR.REC.*`); the synthetic `SFTR.FBK.*` checks were removed in Milestone 0.4. |
| `REC.*` (reconciliation `auth.106` / `auth.083`) | 6 | 3 | Both layers exist with the same caveat (`auth.106`/`auth.083` are placeholders, see `docs/auth-messages.md`). |
| `TST.*` (TR State Report — `auth.107` / `auth.079`) | 7 | 12 | SFTR has more state-oriented checks because SFTR margin lending state lives only in the TSR (see `docs/sftr-margin-lending.md`). EMIR margin state has its own `auth.109`. |
| `TST.LFC.*` (cross-batch TSR drift) | 4 | 3 | Symmetric; EMIR's `MATURITY_CHANGED` doesn't exist on the SFTR side (SFT maturity rarely revised at the TR). |
| `TRA.*` (Trade Activity replay `auth.030` / `auth.052`) | 5 | 5 | Symmetric: repeated correction, spike TERM, spike MODI, duplicate NEWT, NEWT not in TSR. |
| `AUD.*` (TR audit consolidated) | 3 | 3 | Symmetric: NEWT_IN_TAR_NOT_IN_TSR, OUTSTANDING_IN_TSR_NOT_IN_TAR, REJECTED_BUT_OUTSTANDING_IN_TSR. |
| `BREC.*` (book vs TSR reconciliation) | 7 | 7 | Symmetric: 5 mismatches + 2 missing-direction. |
| `MAR.*` (margin activity) | 8 + 3 LFC | 5 | EMIR has `auth.108` margin activity; SFTR uses inline `auth.052` rows with `sft_type=MGLD` / `action_type=MARU`. The 3 EMIR.MAR.LFC checks have no SFTR equivalent (no separate margin scan). |
| `MSR.*` (margin state) | 8 + 3 LFC | 6 | Same rationale: EMIR has `auth.109` margin state; SFTR margin state lives in the `auth.079` TSR rows with `sft_type=MGLD`. |
| `RMT.*` (Article 11 risk mitigation) | 10 | 0 | **EMIR-specific regulation.** Article 11 mandates apply only to non-cleared OTC derivatives. SFTR has its own risk-mitigation regime (SFT regulation Article 15) which is structurally different — not ported. |
| `VLD.* coverage gap-fillers` | 6 | 4 | Phase 6 audit added 6 EMIR (corporate sector enum, reporting obligation enum, price negative, delta out of range, gamma negative, commercial-or-treasury-requires-NFC) and 4 SFTR (reuse-indicator-requires-portfolio, event-type-enum, master-agreement-type-enum, lending-fee-negative). |

¹ Added by the Phase 7.6 parity push.

**Total EMIR**: ~225 checks. **Total SFTR**: ~98 checks.

The ~127-check gap is **structural**, not a deferral. The two regimes
cover fundamentally different product types (OTC derivatives vs.
securities financing transactions) and different regulatory regimes
(EMIR Article 10/11/etc. vs. SFT Regulation Article 4/15/etc.).

## Decision matrix for future check additions

When adding a new check to one regime, evaluate:

1. **Does the field on which the check operates exist on the other
   regime's record type?** If yes → likely should port.
2. **Does the regulatory concept apply to the other regime?** For
   example, "clearing CCP" exists in EMIR but not in SFTR.
3. **Is the check derivative-specific (asset class, Greeks, leg
   structure, notional, valuation derivative)?** Then it stays
   EMIR-only.
4. **Is the check SFT-specific (loan/collateral/haircut/sft_type
   semantics, reuse, securities lending fee, repo rebate)?** Then it
   stays SFTR-only.

When in doubt, add a row to the table above.

## Decisions captured by this audit (not gaps)

These EMIR-only check families are **deliberately not ported**:

- `EMIR.RMT.*` — Article 11 EMIR-only (10 checks).
- `EMIR.MAR.*` / `EMIR.MAR.LFC.*` / `EMIR.MSR.*` / `EMIR.MSR.LFC.*` —
  EMIR has dedicated `auth.108` / `auth.109` margin messages; SFTR
  margin is inline in `auth.052` / `auth.079` and is covered by
  `SFTR.MAR.*` / `SFTR.MSR.*` against MGLD rows (16 checks). See
  `docs/sftr-margin-lending.md`.
- Greeks (delta, gamma, vega), leg1/leg2 fields, asset-class deep
  requires (IR_REQUIRES_NOTIONAL, FX_REQUIRES_LEG2_CURRENCY,
  EQ_REQUIRES_UNDERLYING, CR_REQUIRES_UNDERLYING,
  COMMODITY_REQUIRES_PRODUCT_ID, IR_REQUIRES_LEG1_FREQ), MTM change,
  clearing-status / CCP / NCLR_FORBIDS_CCP, valuation derivative
  semantics (VALU_REQUIRES_VALUATION, VALUATION_AFTER_REPORTING,
  VALUATION_TYPE_ENUM), nature / trading_capacity / intragroup
  semantics, hedging_indicator, corporate_sector,
  reporting_obligation_indicator, commercial_or_treasury_financing.

These SFTR-only check families have no EMIR analog by construction:

- `SFTR.VLD.SFT_TYPE_*`, `SFTR.CON.REBATE_REQUIRES_REPO_OR_BSB`,
  `SFTR.CON.LENDING_FEE_REQUIRES_SLEB`, `SFTR.CON.COLU_REQUIRES_PORTFOLIO`,
  `SFTR.CON.REUU_REQUIRES_REUSE_INDICATOR`,
  `SFTR.CON.REUSE_INDICATOR_REQUIRES_PORTFOLIO`,
  `SFTR.CON.LOAN_COLL_CURRENCY_MISMATCH`, all MGLD-specific
  (`SFTR.MAR.MGLD_*`, `SFTR.MSR.MGLD_*`) — securities-financing
  semantics.

## Fixture parity (Phase 7.6 additions)

- `examples/sftr/tier2.csv` + `tier2.yml` — synthetic fixture that
  exercises the 9 new parity checks (mirrors `examples/emir/tier2.csv`).
- `sftr scan` now accepts CSV inputs via `--mapping` — was XML-only
  before, now matches the EMIR scan CLI.
