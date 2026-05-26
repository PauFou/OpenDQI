# Competing Use — definition for OpenDQI

This file is referenced by [LICENSE.md](LICENSE.md) (Permitted
Purpose clause). It exists to make the boundary between Permitted Use
and Competing Use explicit for the specific domain OpenDQI addresses
(EMIR/SFTR regulatory data quality), so adopters can decide on day one
whether their planned use is in scope without needing legal review.

## Competing Use

A Competing Use of OpenDQI means using the Software to provide, **as a
third party for compensation**, any of the following to one or more
licensees:

1. A data-quality engine, validation service, pre-submission gate, or
   reconciliation tool for **EMIR**, **SFTR**, or any **comparable
   transaction-reporting regulatory flow** (UK EMIR, MiFIR, etc.).
2. A hosted, packaged, or otherwise-resold variant of OpenDQI that
   substitutes for using OpenDQI directly (e.g. "OpenDQI as a Service",
   "EmirGuard Cloud", or any similarly-positioned offering).
3. A product or service whose primary value proposition is data
   quality on the regulatory reporting flows OpenDQI covers, and
   whose implementation relies materially on OpenDQI code.

## Not Competing Use (explicitly permitted)

The following uses are NOT Competing Uses and are fully permitted under
LICENSE.md, even when they involve a commercial context:

1. **Internal use by a reporting firm** — running OpenDQI inside a
   bank, broker, asset manager, or any other entity that is itself
   subject to EMIR/SFTR reporting obligations, to check or improve
   its own reports. Even if that firm is highly profitable, even if
   it sells regulated financial services, this remains internal use.
2. **Internal use by a service provider doing operational work for
   reporting firms** — running OpenDQI as part of professional
   services delivered to a single licensee firm in connection with
   their own reporting flow.
3. **Bundling OpenDQI into a product whose value proposition lies
   elsewhere** — e.g. a data lineage product that happens to surface
   OpenDQI quality scores as one signal among many, where the
   product is not positioned as a regulatory DQ engine.
4. **Modifications, forks, internal patches, contributions back** —
   freely permitted; redistribution preserves these Terms and
   Conditions per the Redistribution clause of LICENSE.md.
5. **Educational, research, journalistic, or regulatory-research
   use** — fully permitted, with or without compensation, provided
   the resulting service is not itself a Competing Use as defined
   above.

## Doubt

If you are unsure whether your planned use is a Competing Use, the
safest course is to open a discussion thread on the GitHub repository
before going to production. The Licensor will respond in good faith
with a written clarification you can rely on for that specific use.

## Conversion to Apache-2.0

This boundary applies only while a given OpenDQI release is in its
FSL window. Two years after each release tag (its Change Date), that
release converts automatically to Apache-2.0 under which there is no
Competing Use restriction. The pre-conversion releases remain
restricted on their original terms.
