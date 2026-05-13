# Field-coverage audit

Every typed field on every OpenDQI record type should be exercised by
at least one DQ check. This document is the manually-maintained
coverage matrix: it lists each record type, each typed field, and the
checks that cover it (presence, format, sanity, or cross-field
consistency). Fields covered only through the raw-field catch-all do
not count.

The audit was last run for the Phase 6 milestone (May 2026). Each
audit pass should:

1. List every typed field on every record type.
2. Map each field to the `EMIR.*` / `SFTR.*` check IDs that exercise it.
3. Identify fields with no check; create one if the gap is material.
4. Update the matrix below.

## EmirRecord (`auth.030`-derived)

| Field | Covered by |
|---|---|
| `uti` | `EMIR.COMP.UTI_MISSING`, `EMIR.UNI.DUPLICATE_UTI`, `EMIR.CON.NEWT_FORBIDS_PRIOR_UTI`, `EMIR.CON.MODI_PRESERVES_UTI` |
| `prior_uti` | `EMIR.CON.NEWT_FORBIDS_PRIOR_UTI`, `EMIR.CON.MODI_PRESERVES_UTI` |
| `action_type` | `EMIR.VLD.ACTION_TYPE_ENUM`, `EMIR.CON.ACTION_EVENT_COMPATIBILITY`, lifecycle/TAR/audit checks |
| `event_type` | `EMIR.VLD.EVENT_TYPE_ENUM`, `EMIR.CON.ACTION_EVENT_COMPATIBILITY` |
| `entity_responsible_for_reporting` | `EMIR.VLD.LEI_FORMAT_ERR`, `EMIR.RMT.INTRAGROUP_NEEDS_INDICATOR` |
| `counterparty_1` | `EMIR.COMP.COUNTERPARTY_1_MISSING`, `EMIR.VLD.LEI_FORMAT_RC`, `EMIR.CON.SELF_DEALING`, `EMIR.RMT.INTRAGROUP_NEEDS_INDICATOR` |
| `counterparty_2` | `EMIR.COMP.COUNTERPARTY_2_MISSING`, `EMIR.VLD.LEI_FORMAT_OC`, `EMIR.CON.SELF_DEALING`, `EMIR.RMT.INTRAGROUP_NEEDS_INDICATOR` |
| `asset_class` | `EMIR.COMP.ASSET_CLASS_MISSING`, `EMIR.VLD.ASSET_CLASS_ENUM`, asset-class consistency checks |
| `product_id` | `EMIR.COMP.PRODUCT_ID_MISSING`, `EMIR.ACC.COMMODITY_REQUIRES_PRODUCT_ID` |
| `underlying_id` | `EMIR.ACC.EQ_REQUIRES_UNDERLYING`, `EMIR.ACC.CR_REQUIRES_UNDERLYING` |
| `notional_amount` | `EMIR.ACC.ZERO_NOTIONAL`, `EMIR.ACC.NEGATIVE_NOTIONAL`, `EMIR.VLD.NOTIONAL_PRECISION*`, `EMIR.ACC.NOTIONAL_ABNORMAL_MAGNITUDE`, `EMIR.ACC.IR_REQUIRES_NOTIONAL`, `EMIR.RMT.INITIAL_MARGIN_THRESHOLD`, `EMIR.RMT.NFC_ABOVE_CLEARING_THRESHOLD` |
| `notional_currency` | `EMIR.COMP.NOTIONAL_CURRENCY_MISSING`, `EMIR.VLD.CURRENCY_NOTIONAL`, `EMIR.CON.NOTIONAL_VAL_CURRENCY_MISMATCH` |
| `price` | `EMIR.VLD.PRICE_PRECISION*`, `EMIR.CON.PRICE_REQUIRES_CURRENCY`, `EMIR.ACC.PRICE_NEGATIVE` ¹ |
| `price_currency` | `EMIR.CON.PRICE_REQUIRES_CURRENCY`, `EMIR.CON.PRICE_VAL_CURRENCY_MISMATCH` |
| `execution_timestamp` | `EMIR.TIM.LATE_REPORTING`, `EMIR.CON.REPORTING_BEFORE_EXECUTION`, `EMIR.CON.EVENT_BEFORE_EXECUTION`, `EMIR.RMT.LATE_CONFIRMATION` |
| `event_timestamp` | `EMIR.CON.EVENT_BEFORE_EXECUTION` |
| `reporting_timestamp` | `EMIR.TIM.LATE_REPORTING`, `EMIR.CON.REPORTING_BEFORE_EXECUTION`, `EMIR.CON.VALUATION_AFTER_REPORTING`, `EMIR.RMT.LATE_CONFIRMATION` |
| `effective_date` | `EMIR.CON.EFFECTIVE_AFTER_MATURITY` |
| `maturity_date` | `EMIR.ACC.ABNORMAL_MATURITY`, `EMIR.CON.MATURITY_IN_PAST`, `EMIR.CON.EFFECTIVE_AFTER_MATURITY`, `EMIR.CON.TERMINATION_AFTER_MATURITY` |
| `termination_date` | `EMIR.CON.TERMINATION_AFTER_MATURITY`, `EMIR.CON.ETRM_REQUIRES_TERMINATION_DATE`, `EMIR.CON.NEWT_FORBIDS_TERMINATION_DATE`, `EMIR.CON.VALUATION_AFTER_TERMINATION` |
| `valuation_amount` | `EMIR.COMP.MISSING_VALUATION`, `EMIR.VLD.VALUATION_PRECISION*`, `EMIR.CON.ETRM_REQUIRES_VALUATION`, `EMIR.CON.VALU_REQUIRES_VALUATION`, `EMIR.CON.MTM_CHANGE_REQUIRES_VALUATION` |
| `valuation_currency` | `EMIR.COMP.VALUATION_CURRENCY_MISSING`, `EMIR.VLD.CURRENCY_VALUATION`, `EMIR.CON.NOTIONAL_VAL_CURRENCY_MISMATCH`, `EMIR.CON.PRICE_VAL_CURRENCY_MISMATCH` |
| `valuation_timestamp` | `EMIR.COMP.VALUATION_TIMESTAMP_MISSING`, `EMIR.CON.VALUATION_AFTER_REPORTING`, `EMIR.RMT.DAILY_VALUATION_MISSING` |
| `initial_margin_posted` | `EMIR.COMP.INITIAL_MARGIN_MISSING_FOR_FULL`, `EMIR.ACC.NEGATIVE_INITIAL_MARGIN_POSTED`, `EMIR.VLD.MARGIN_PRECISION`, `EMIR.CON.IM_NEEDS_COLLATERAL_PORTFOLIO`, `EMIR.CON.MARU_REQUIRES_MARGIN`, `EMIR.RMT.INITIAL_MARGIN_THRESHOLD` |
| `initial_margin_collected` | `EMIR.ACC.NEGATIVE_INITIAL_MARGIN_COLLECTED`, …same as posted |
| `variation_margin_posted` | `EMIR.COMP.VARIATION_MARGIN_MISSING_FOR_FULL`, `EMIR.ACC.NEGATIVE_VARIATION_MARGIN_POSTED`, `EMIR.CON.VM_NEEDS_COLLATERAL_PORTFOLIO`, `EMIR.RMT.VARIATION_MARGIN_MISSING` |
| `variation_margin_collected` | `EMIR.ACC.NEGATIVE_VARIATION_MARGIN_COLLECTED`, …same as posted |
| `collateral_portfolio_code` | `EMIR.COMP.COLLATERAL_PORTFOLIO_REQUIRED_FOR_FULL`, `EMIR.CON.IM/VM_NEEDS_COLLATERAL_PORTFOLIO`, `EMIR.CON.POSC/MARU_REQUIRES_PORTFOLIO`, `EMIR.RMT.PORTFOLIO_RECONCILIATION_MISSING` |
| `clearing_status` | `EMIR.COMP.CLEARING_STATUS_MISSING`, `EMIR.VLD.CLEARING_STATUS_ENUM`, `EMIR.CON.CLEARED_REQUIRES_CCP`, `EMIR.CON.NCLR_FORBIDS_CCP`, every `EMIR.RMT.*` filters on this |
| `collateralisation_category` | `EMIR.VLD.COLLATERALISATION_CATEGORY_ENUM`, `EMIR.COMP.COLLATERAL_PORTFOLIO_REQUIRED_FOR_FULL`, `EMIR.RMT.COLLATERAL_CATEGORY_REQUIRED` |
| `clearing_ccp_lei` | `EMIR.VLD.LEI_FORMAT_CCP`, `EMIR.CON.CLEARED_REQUIRES_CCP`, `EMIR.CON.NCLR_FORBIDS_CCP` |
| `intragroup_indicator` | `EMIR.COMP.INTRAGROUP_INDICATOR_MISSING`, `EMIR.RMT.INTRAGROUP_NEEDS_INDICATOR` |
| `hedging_indicator` | `EMIR.CON.HEDGING_REQUIRES_NFC` |
| `valuation_type` | `EMIR.VLD.VALUATION_TYPE_ENUM` |
| `trading_capacity` | `EMIR.COMP.TRADING_CAPACITY_MISSING`, `EMIR.VLD.TRADING_CAPACITY_ENUM` |
| `commercial_or_treasury_financing` | `EMIR.CON.COMMERCIAL_OR_TREASURY_REQUIRES_NFC` ¹ |
| `reporting_obligation_indicator` | `EMIR.VLD.REPORTING_OBLIGATION_INDICATOR_ENUM` ¹ |
| `corporate_sector` | `EMIR.VLD.CORPORATE_SECTOR_ENUM` ¹ |
| `nature` | `EMIR.COMP.NATURE_MISSING`, `EMIR.VLD.NATURE_ENUM`, `EMIR.CON.HEDGING_REQUIRES_NFC`, `EMIR.RMT.NFC_ABOVE_CLEARING_THRESHOLD`, `EMIR.RMT.LATE_CONFIRMATION` |
| `master_agreement_type` | `EMIR.COMP.MASTER_AGREEMENT_TYPE_MISSING`, `EMIR.VLD.MASTER_AGREEMENT_TYPE_ENUM`, `EMIR.RMT.MASTER_AGREEMENT_REQUIRED` |
| `master_agreement_version` | `EMIR.COMP.MASTER_AGREEMENT_VERSION_MISSING`, `EMIR.VLD.MASTER_AGREEMENT_VERSION_FORMAT`, `EMIR.VLD.ISDA_VERSION_PLAUSIBLE` |
| `confirmation_method` | `EMIR.RMT.UNCLEARED_NEEDS_CONFIRMATION` |
| `mtm_value_change` | `EMIR.CON.MTM_CHANGE_REQUIRES_VALUATION` |
| `delta` | `EMIR.ACC.DELTA_OUT_OF_RANGE` ¹ |
| `gamma` | `EMIR.ACC.GAMMA_NEGATIVE` ¹ |
| `vega` | — (no check yet; future milestone) |
| `leg2_notional_amount` | `EMIR.CON.LEG2_NOTIONAL_NEEDS_CURRENCY` |
| `leg2_notional_currency` | `EMIR.ACC.FX_REQUIRES_LEG2_CURRENCY`, `EMIR.CON.LEG1_LEG2_SAME_CURRENCY` |
| `leg1_payment_frequency` | `EMIR.ACC.IR_REQUIRES_LEG1_FREQ` |
| `leg2_payment_frequency` | — (only used by `IR_REQUIRES_LEG1_FREQ` for context) |

¹ Added during the Phase 6 coverage audit.

## SftrRecord (`auth.052`-derived)

| Field | Covered by |
|---|---|
| `uti` | `SFTR.COMP.UTI_MISSING`, `SFTR.UNI.DUPLICATE_UTI`, `SFTR.CON.NEWT_FORBIDS_PRIOR_UTI` |
| `prior_uti` | `SFTR.CON.NEWT_FORBIDS_PRIOR_UTI` |
| `action_type` | `SFTR.VLD.ACTION_TYPE_ENUM`, lifecycle/TAR checks |
| `event_type` | `SFTR.VLD.EVENT_TYPE_ENUM` ¹ |
| `entity_responsible_for_reporting` | `SFTR.VLD.LEI_FORMAT_ERR` |
| `counterparty_1` | `SFTR.COMP.COUNTERPARTY_1_MISSING`, `SFTR.VLD.LEI_FORMAT_RC`, `SFTR.CON.SELF_DEALING` |
| `counterparty_2` | `SFTR.COMP.COUNTERPARTY_2_MISSING`, `SFTR.VLD.LEI_FORMAT_OC`, `SFTR.CON.SELF_DEALING` |
| `sft_type` | `SFTR.COMP.SFT_TYPE_MISSING`, `SFTR.VLD.SFT_TYPE_ENUM`, `SFTR.CON.REBATE_REQUIRES_REPO_OR_BSB`, `SFTR.CON.LENDING_FEE_REQUIRES_SLEB` |
| `master_agreement_type` | `SFTR.VLD.MASTER_AGREEMENT_TYPE_ENUM` ¹ |
| `master_agreement_version` | `SFTR.VLD.MASTER_AGREEMENT_VERSION_FORMAT`, `SFTR.VLD.GMRA_GMSLA_VERSION_PLAUSIBLE` |
| `loan_value` | `SFTR.ACC.NEGATIVE_LOAN`, `SFTR.VLD.LOAN_PRECISION`, `SFTR.CON.LOAN_NEEDS_CURRENCY` |
| `loan_currency` | `SFTR.COMP.LOAN_CURRENCY_MISSING`, `SFTR.VLD.CURRENCY_LOAN`, `SFTR.CON.LOAN_COLL_CURRENCY_MISMATCH` |
| `collateral_value` | `SFTR.COMP.COLLATERAL_VALUE_MISSING`, `SFTR.ACC.NEGATIVE_COLLATERAL`, `SFTR.VLD.COLLATERAL_PRECISION`, `SFTR.CON.COLL_NEEDS_CURRENCY` |
| `collateral_currency` | `SFTR.COMP.COLLATERAL_CURRENCY_MISSING`, `SFTR.VLD.CURRENCY_COLLATERAL`, `SFTR.CON.LOAN_COLL_CURRENCY_MISMATCH` |
| `haircut` | `SFTR.COMP.HAIRCUT_MISSING`, `SFTR.ACC.HAIRCUT_OUT_OF_RANGE`, `SFTR.VLD.HAIRCUT_PRECISION` |
| `reuse_indicator` | `SFTR.CON.REUU_REQUIRES_REUSE_INDICATOR`, `SFTR.CON.REUSE_INDICATOR_REQUIRES_PORTFOLIO` ¹ |
| `rebate_rate` | `SFTR.CON.REBATE_REQUIRES_REPO_OR_BSB`, `SFTR.VLD.RATE_PRECISION` |
| `lending_fee` | `SFTR.CON.LENDING_FEE_REQUIRES_SLEB`, `SFTR.VLD.RATE_PRECISION`, `SFTR.ACC.LENDING_FEE_NEGATIVE` ¹ |
| `execution_timestamp` | `SFTR.TIM.LATE_REPORTING`, `SFTR.CON.SETTLEMENT_BEFORE_EXECUTION` |
| `event_timestamp` | — (used contextually by TAR) |
| `reporting_timestamp` | `SFTR.TIM.LATE_REPORTING` |
| `effective_date` | `SFTR.CON.MATURITY_BEFORE_EFFECTIVE` |
| `maturity_date` | `SFTR.CON.MATURITY_BEFORE_EFFECTIVE`, TSR `ACTIVE_PAST_MATURITY` |
| `termination_date` | `SFTR.CON.NEWT_FORBIDS_TERMINATION_DATE`, `SFTR.CON.ETRM_REQUIRES_TERMINATION_DATE` |
| `settlement_date` | `SFTR.CON.SETTLEMENT_BEFORE_EXECUTION` |
| `collateral_portfolio_code` | `SFTR.CON.COLU_REQUIRES_PORTFOLIO`, `SFTR.CON.REUSE_INDICATOR_REQUIRES_PORTFOLIO` ¹ |
| `collateral_isin` | `SFTR.VLD.ISIN_COLLATERAL` |

¹ Added during the Phase 6 coverage audit.

## TrStateRecord (EMIR TSR, `auth.107`)

| Field | Covered by |
|---|---|
| `state_as_of` | `EMIR.TST.STALE_VALUATION` (reference clock) |
| `uti` | `EMIR.TST.DUPLICATE_ACTIVE_UTI` |
| `reporting_counterparty`, `other_counterparty` | covered as TR-side LEI fields by future TSR LEI checks (roadmap) |
| `status` | filters `EMIR.TST.*` outstanding-only checks |
| `notional_amount`, `notional_currency` | TSR layer relies on the existing record reuse for context |
| `valuation_amount` | `EMIR.TST.MISSING_VALUATION` |
| `valuation_currency` | — (TSR-side currency mismatch is a future check) |
| `valuation_timestamp` | `EMIR.TST.STALE_VALUATION`, `EMIR.TST.VALUATION_AFTER_TERMINATION` |
| `effective_date`, `maturity_date`, `termination_date` | `EMIR.TST.ACTIVE_PAST_MATURITY`, `EMIR.TST.PLACEHOLDER_MATURITY` |
| `collateral_portfolio_code` | — (covered indirectly through cross-batch checks) |

## SftrTrStateRecord (SFTR TSR, `auth.079`)

| Field | Covered by |
|---|---|
| `state_as_of` | `SFTR.TST.STALE_VALUATION` |
| `uti` | `SFTR.TST.DUPLICATE_ACTIVE_UTI` |
| `status` | filters `SFTR.TST.*` outstanding-only checks |
| `sft_type`, `loan_value`, `loan_currency` | covered indirectly by single-batch SFTR checks at submission time |
| `collateral_value` | `SFTR.TST.MISSING_COLLATERAL` |
| `haircut` | `SFTR.TST.HAIRCUT_OUT_OF_RANGE_ON_OUTSTANDING` |
| `reuse_indicator` | covered by single-batch `SFTR.CON.REUU_REQUIRES_REUSE_INDICATOR` |
| `maturity_date`, `termination_date` | `SFTR.TST.ACTIVE_PAST_MATURITY` |

## MarginActivityRecord (EMIR MAR, `auth.108`)

| Field | Covered by |
|---|---|
| `action_type` | `EMIR.MAR.MARGIN_TYPE_ENUM` |
| `collateral_portfolio_code` | `EMIR.MAR.PORTFOLIO_CODE_MISSING`, `EMIR.MAR.DUPLICATE_MARGIN_CALL`, `EMIR.MAR.LARGE_MARGIN_DELTA` |
| `initial_margin_posted`, `variation_margin_posted` | `EMIR.MAR.POSTED_NEGATIVE`, `EMIR.MAR.LARGE_MARGIN_DELTA`, `EMIR.MAR.MARGIN_NEEDS_CURRENCY` |
| `initial_margin_collected`, `variation_margin_collected` | `EMIR.MAR.COLLECTED_NEGATIVE`, `EMIR.MAR.MARGIN_NEEDS_CURRENCY` |
| `margin_currency` | `EMIR.MAR.MARGIN_NEEDS_CURRENCY` |
| `event_timestamp`, `reporting_timestamp` | `EMIR.MAR.TIMELINESS`, `EMIR.MAR.LARGE_MARGIN_DELTA` (ordering), `EMIR.MAR.DUPLICATE_MARGIN_CALL` |
| `excess_collateral`, `collateral_haircut` | — (presence-only ingestion; no sanity check yet) |

## MarginStateRecord (EMIR MSR, `auth.109`)

| Field | Covered by |
|---|---|
| `state_as_of` | `EMIR.MSR.MARGIN_STALE` |
| `uti` | `EMIR.MSR.MARGIN_MISSING_FOR_OUTSTANDING` |
| `collateral_portfolio_code` | — (presence-only ingestion) |
| `initial_margin_posted_current`, `initial_margin_collected_current` | `EMIR.MSR.INITIAL_MARGIN_NEGATIVE`, `EMIR.MSR.IM_POSTED_VS_COLLECTED_IMBALANCE`, `EMIR.MSR.MARGIN_MISSING_FOR_OUTSTANDING` |
| `variation_margin_posted_current`, `variation_margin_collected_current` | `EMIR.MSR.VARIATION_MARGIN_NEGATIVE`, `EMIR.MSR.MARGIN_MISSING_FOR_OUTSTANDING` |
| `margin_currency` | — (presence-only) |
| `collateral_market_value` | `EMIR.MSR.COLLATERAL_MARKET_VALUE_NEGATIVE` |
| `haircut_applied` | `EMIR.MSR.HAIRCUT_OUT_OF_RANGE` |
| `collateralization_category` | `EMIR.MSR.COLLATERALIZATION_CATEGORY_ENUM` |

## Roadmap

Fields marked "—" should grow checks as data shows up. The current
gaps are intentional: every check we ship is backed by either a real
ESMA validation rule or a clear data-quality intuition.
