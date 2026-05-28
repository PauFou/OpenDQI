---
name: Feature request
about: Propose a new check, DQI indicator, ESMA message parser, CLI subcommand, or Python entry point.
title: '[feat] <one-line summary>'
labels: enhancement
assignees: PauFou
---

## Use case

<!-- One paragraph: who would use this and what problem it
     solves. The more concrete, the easier it is to scope. -->

## Proposed surface

Which OpenDQI surface does this touch?

- [ ] New per-record granular check (`EMIR.*` or `SFTR.*`)
- [ ] New roll-up DQI indicator (Data Quality Pack)
- [ ] New ESMA / ISO 20022 message parser (`auth.XXX.YYY.ZZ`)
- [ ] New CLI subcommand (`opendqi <regime> <new-cmd>`)
- [ ] New Python entry point (`opendqi.<regime>.<new_fn>`)
- [ ] New output format / report template
- [ ] Documentation / examples
- [ ] Other (describe below)

## Alternatives considered

<!-- What workarounds exist today? Why are they insufficient? -->

## ESMA references (if regulatory)

<!-- If this is a new check / DQI / parser, cite the relevant
     ESMA Usage Guideline, RTS article, or ISO 20022 doc. -->

## Sketch of the API or output

<!-- Optional: pseudocode / mock CLI invocation / Python snippet
     showing what the feature would look like from the user's
     side. -->

```python
result = opendqi.emir.new_feature(...)
print(result.summary)
```

## Honest scope check

- Is this in the scope OpenDQI documents (EMIR / SFTR / adjacent
  ISO 20022 regulatory reporting)?
- Is this a recurring problem for many users, or a one-off?
- Would a downstream adopter be willing to contribute the
  implementation under the CLA?
