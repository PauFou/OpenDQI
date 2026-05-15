-- m0002: faithful auth.092 validation-rule list.
--
-- auth.092 lists several DtldVldtnRule codes per rejected transaction;
-- OpenDQI now keeps the full list (FeedbackRecord.validation_rule_codes).
-- Additive and backward-compatible: existing feedback rows get NULL,
-- which the loader reads as an empty list (the scalar `reason_code`
-- column is retained and still equals the first rule).
ALTER TABLE feedbacks ADD COLUMN validation_rule_codes_json TEXT;
