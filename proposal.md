# Live Event Ad Creative Readiness Proposal

Converted from `Proposal_ Ad Creative Readiness (2).docx` (IAB Tech Lab working-group draft, v0.02). Stale v0.01 copy in the Word file is omitted; worksheets are kept.

Source images: `md-extract/media/media/`.

---

[**Proposal Versions + Worksheets 1**](#proposal-versions-worksheets)

> [**Proposal Tab 1**](#proposal-tab)
>
> [**Worksheets Tab 1**](#worksheets-tab)

[**Proposal Development Phases 2**](#proposal-development-phases)

[**Potential Standards Delivery Strategy
3**](#potential-standards-delivery-strategy)

# Proposal Versions + Worksheets

The Creative Readiness project will use two coordinated document tabs to
document discussions, conclusions and proposal versions.

## [Proposal Tab](#proposal-tab)

This document tab contains the most recent concise, current working
group position. The highest numbered version subtab is the latest
proposal for discussion.\
\
By also including historical versions with commentary it should reflect
conclusions rather than preserve the entire debate that produced them.

## Worksheets Tab

The Worksheets act as the proposal's detailed deliberation record. Each
significant object, definition, architecture choice, dependency or
recommendation should have a corresponding Worksheet when additional
investigation or consensus is required. The relationship is analogous to
a tax form and its supporting worksheets: detailed inputs are resolved
in Worksheets and the resulting conclusion is carried into the primary
proposal.

A Worksheet may contain:

- decision question;

- relevance/priority;

- known facts;

- existing market behavior;

- use cases;

- alternatives;

- advantages;

- disadvantages;

- dependencies;

- open questions;

- dissenting views;

- preliminary direction;

- final working group decision;

- resulting proposal changes.

# Proposal Development Phases

![](md-extract/media/media/image2.png){width="6.5in"
height="6.611111111111111in"}

# Potential Standards Delivery Strategy

![](md-extract/media/media/image1.png){width="7.023408792650919in"
height="3.387878390201225in"}

v0.02

Live Event Ad Creative Readiness Proposal

Version 0.02

[**Sponsors / +1s 1**](#sponsors-1s)

[**Supporting Documents 1**](#supporting-documents)

[**Executive Summary 2**](#executive-summary)

[**Background and Industry Context
3**](#background-and-industry-context)

[**Problem Statements 4**](#problem-statements)

[**Goals 9**](#goals)

[**Scope 10**](#scope)

[**Stakeholder Use Cases 10**](#stakeholder-use-cases)

[**Definitions 14**](#definitions)

[**Conclusion 15**](#conclusion)

[**Version 0.01 to 0.02 Change Log
16**](#version-0.01-to-0.02-change-log)

[**Open Questions 16**](#open-questions)

# Sponsors / +1s

  -----------------------------------------------------------------------
  **Supply**                          **Demand**
  ----------------------------------- -----------------------------------
                                      

                                      
  -----------------------------------------------------------------------

# Supporting Documents

- [LEAS SubDoc - Creative Compatibility for Live
  Events](https://docs.google.com/document/d/1TR55j6qK7PXCCHIqrjuZhKQULc4A0FTzbX4ylD28RVY/edit?tab=t.0#heading=h.i2vm5361qnb2)

- [Proposal - Ad Management API 1.2
  (Basis)](https://docs.google.com/document/d/1yV7tYoZkc9aRoJHyGjZR02ugnF4QLea5EO6-L3dgNXI/edit?tab=t.0)

- [Ad Management API v1.1 (current
  version)](https://github.com/InteractiveAdvertisingBureau/AdManagementAPI/blob/master/Ad%20Management%20API%20v1.md)

- Creative Readiness Worksheets

# Executive Summary

Live streaming environments significantly reduce the industry\'s
tolerance for creative delivery failure. Creative assets that have not
been validated, approved, distributed, transcoded or otherwise prepared
for the applicable execution environment may become unusable when a
high-concurrency live advertising opportunity occurs.

Existing creative workflows are frequently inherited from Video on
Demand assumptions, including asynchronous review, retriable failures
and manual remediation. Those assumptions do not consistently support
live events where an ad opportunity may last only seconds and where a
failure may simultaneously affect large audiences and high-value
inventory.

In many live streaming environments, the entity establishing or
enforcing creative readiness requirements is not necessarily a single
publisher. Streaming platforms, SSPs, marketplaces, SSAI providers, SGAI
systems, ad servers, virtual MVPDs and other supply-side intermediaries
may operate shared infrastructure or enforce requirements across
multiple publishers. Creative readiness therefore requires a
multi-entity model.

Creative readiness also does not always begin with advance creative
registration. Some marketplaces and auction platforms discover creatives
dynamically through programmatic auction traffic. They may inspect all
bid creatives, inspect only creatives associated with winning bids, or
trigger review the first time an unknown creative becomes relevant to an
auction outcome.

When a previously unseen creative appears on a winning bid, a platform
may:

- allow the creative to serve while simultaneously initiating review;

- select the next eligible bid containing a previously reviewed creative
  while reviewing the unknown creative for subsequent opportunities;

- prevent the creative from serving until review is complete; or

- apply different behavior based on transaction type, publisher policy,
  event risk or other context.

This proposal therefore recognizes both **proactive readiness** and
**auction-triggered readiness** as legitimate industry workflows.

The proposal explores a standardized industry approach to **Live Event
Ad Creative Readiness**: determining whether an advertising creative has
satisfied the policy, technical, operational and contextual requirements
necessary for reliable use when a live monetization opportunity occurs.

The IAB Tech Lab Ad Management API (AMA) is an important existing
standard for creative submission, review and feedback. However, AMA
adoption is not universal, and proprietary creative ingest and review
APIs are widely used throughout the market. This proposal therefore does
not yet presume that AMA must be the exclusive implementation mechanism.

The working group will determine whether the final recommendation
should:

- require AMA;

- recommend AMA as the exclusive preferred standard while permitting
  other implementations; or

- recommend AMA while defining interoperable Creative Readiness
  capabilities that proprietary or third-party creative ingest/review
  APIs should also support.

A companion **Worksheets** document will be used to deliberate these and
other decisions. The Worksheets retain detailed questions, facts,
alternatives, assumptions, dissent and decision history while this
proposal remains the concise record of working group conclusions.

A near-term extension of AMA, potentially similar in scope to the
previously proposed publisher-level audit functionality for AMA v1.2,
may provide meaningful improvements before every aspect of Creative
Readiness is resolved. Broader capabilities may require subsequent AMA
evolution, implementation guidance, compatibility requirements for
proprietary APIs or additional Tech Lab specifications.

# Background and Industry Context

The Advanced TV ecosystem is experiencing:

- rapid growth in live streaming inventory;

- increased use of automated and programmatic decisioning;

- more complex streaming and ad insertion architectures;

- increased use of shared platforms serving multiple publishers;

- higher financial and reputational consequences when live delivery
  fails.

At the same time, creative validation remains:

- fragmented across platforms;

- asynchronous and inconsistent;

- dependent on proprietary workflows;

- disconnected from some downstream delivery constraints;

- difficult for buyers to discover before creative submission.

The market also uses more than one creative review pattern.

Some platforms require advance registration and approval.

Others discover creatives dynamically through bid traffic and build a
known-creative inventory over time.

Still others combine both approaches, requiring pre-registration for
certain premium or restrictive environments while allowing reactive
review elsewhere.

Creative Readiness must therefore describe the lifecycle and evidence
required to establish readiness without assuming there is only one path
by which a creative enters that lifecycle.

# Problem Statements

## Runtime Failure Visibility

Creative problems may not become visible until execution. Systems can
sometimes skip an unusable creative, substitute another advertisement or
apply fallback behavior, so not every creative issue produces a
catastrophic streaming failure.

However, discovering incompatibility during a high-value live
opportunity can still result in lost revenue, underdelivery, unnecessary
processing, reduced fill, degraded viewer experience or emergency
operational intervention.

The objective is to move detection as far upstream as reasonably
possible while recognizing that not every market workflow performs
complete validation before bidding.

## Fragmented Policy Enforcement

DSPs, SSPs, marketplaces, publishers, streaming platforms and ad
delivery systems may each enforce different creative policies or
specifications.

These requirements are not consistently represented in a common
machine-readable form. Buyers may therefore rely on documentation,
account-specific guidance, trial-and-error submissions or proprietary
APIs to learn what is acceptable.

## Lack of Pre-Flight Validation

There is no broadly adopted standardized mechanism allowing buyers or
their automation systems to determine all relevant downstream
requirements and validate a creative before submission.

A creative that satisfies one platform may subsequently fail another
participant\'s requirements.

Pre-flight validation remains an important goal, particularly for
high-value live events, but Creative Readiness should not assume that
all readiness workflows begin before auction participation.

## Auction-Triggered Creative Discovery and Validation

Some marketplaces and auction platforms use auction traffic itself as a
mechanism for discovering new creatives.

Implementations may inspect:

- creatives from all bids received;

- only creatives associated with competitive bids;

- only creatives associated with winning bids; or

- creatives the first time a previously unknown creative ID is
  encountered.

When a winning bid contains an unreviewed creative, the platform may:

- serve the creative and accept the risk while review occurs
  asynchronously;

- fall back to another eligible bid containing a previously reviewed
  creative;

- reject or defer the unknown creative until review completes;

- or apply policy depending on the publisher, deal, event or inventory
  type.

These workflows demonstrate that creative readiness is not universally a
pre-auction state.

The standard should support both **proactive readiness**, established
before auction activity, and **reactive or auction-triggered
readiness**, initiated when a creative is first observed.

Creative Readiness should provide interoperable identity, state,
feedback and policy information while leaving the auction disposition of
an unknown creative to seller/platform policy.

## Inconsistent Creative Identification

Creative identifiers vary between advertiser systems, DSPs, SSPs,
marketplaces, publisher ad servers and delivery platforms.

A single advertising creative may therefore accumulate multiple IDs,
while different versions of a creative can sometimes be difficult to
distinguish.

This complicates:

- approval reuse;

- identifying whether a platform has previously seen the creative;

- transcoding reuse;

- caching;

- version management;

- readiness tracking.

Reliable identity becomes particularly important in auction-triggered
workflows because platforms must determine whether a newly observed ID
represents:

- a truly new creative;

- an existing creative under a different system ID;

- the same advertising concept with modified media;

- or a previously prepared asset that can safely reuse prior readiness
  evidence.

## SSAI and Operational Readiness Gaps

Traditional SSAI environments often need creative assets sufficiently
early to transcode, package, cache or otherwise prepare them for
insertion.

Approval does not guarantee that the creative has reached the SSAI
system or that the media asset required for execution has been prepared.

Creative readiness must therefore distinguish approval from operational
preparation.

Auction-triggered discovery also creates a specific live-event risk: a
newly observed creative may be acceptable from a policy perspective but
may not yet have completed processing required for scalable insertion.

## SGAI and Hybrid Execution Readiness

Server-Guided Ad Insertion introduces a different division of
responsibilities.

In SGAI environments, server-side systems may determine, prepare or
signal advertising while the client or player retrieves and executes
some or all of the advertising experience.

Readiness may consequently depend on server preparation and:

- player/device capabilities;

- signaling;

- asset accessibility;

- execution behavior.

The project should avoid defining operational readiness exclusively
around traditional server-side transcoding and stitching.

It must support SSAI, SGAI, CSAI and hybrid architectures where
appropriate.

## Deal and Supply Chain Ambiguity

In Programmatic Guaranteed, PMP and other deal-based workflows, buyers
may know the deal or marketplace relationship without knowing every
publisher, delivery environment or validation authority downstream.

A scalable solution must determine how applicable creative requirements
and approval destinations can be discovered from the commercial or
supply context already known to the buyer.

## Distributed Validation Authority

Creative validation responsibilities are often distributed among several
entities.

A marketplace may enforce common requirements across hundreds of
publishers.

A publisher may impose additional restrictions.

A streaming platform may control technical delivery requirements.

An SSAI or SGAI environment may impose operational requirements.

A creative may therefore be:

- known to the marketplace;

- approved by the marketplace;

- pending publisher approval;

- technically prepared for one delivery environment;

- unprepared for another.

Readiness must support multiple independent authorities and contexts.

## Proprietary Creative Ingest and Review Fragmentation

The market already contains multiple proprietary creative submission,
ingest, review and approval APIs.

Low adoption of the Ad Management API creates risk if Creative Readiness
assumes that every participant will adopt AMA before the readiness model
can provide value.

At the same time, allowing every proprietary interface to express
readiness differently would preserve the fragmentation this project is
intended to reduce.

The working group must determine whether interoperability should be
achieved by:

- standardizing the API itself;

- standardizing capabilities and semantics required of any creative
  ingest/review API;

- or some combination.

Auction-triggered validation increases the importance of this issue
because existing proprietary systems may already maintain known-creative
stores and auction-integrated review processes that should not need to
be discarded to participate in Creative Readiness.

## Inefficient Feedback Loops

Approval and rejection feedback is often inconsistent, delayed or
difficult to automate.

A simple rejected status is not always sufficient.

Buyers may need to understand whether a creative is:

- unknown;

- newly observed;

- pending review;

- policy rejected;

- technically invalid;

- operationally unprepared;

- approved but contextually ineligible;

- or awaiting downstream processing.

Structured, machine-readable feedback is required for automation.

## Metadata and Format Inconsistencies

Duration, encoding, audio, dimensions, MIME types, identifiers, tracking
information and other metadata may be absent, inaccurate or incompatible
with the eventual execution environment.

These problems can prevent downstream preparation even when the
advertising concept itself is acceptable.

## Creative Distribution and Execution-Path Gaps

A creative may be technically compliant and approved but still fail to
reach every system that needs it.

Creative readiness must contemplate the path from buyer submission
through SSP or marketplace, publisher ad systems, streaming platform and
the applicable SSAI, SGAI or other execution environment.

## Last-Minute Creative Changes

Advertisers frequently replace or update creative close to an event.

A replacement that bypasses existing readiness steps can invalidate
previous approval, transcoding, caching or compatibility results.

The industry needs a way to distinguish cosmetic or metadata changes
from changes that require renewed validation or operational preparation.

# Goals

## Primary Goal

Establish a standardized approach for defining, communicating and
validating Live Event Ad Creative Readiness.

## Secondary Goals

- Enable pre-flight validation.

- Support proactive creative registration and readiness.

- Support auction-triggered creative discovery and validation.

- Allow platforms to distinguish previously reviewed creatives from
  unknown or pending creatives.

- Improve visibility into applicable requirements.

- Standardize interoperable readiness and feedback semantics.

- Support multiple validation authorities.

- Improve confidence in live campaign delivery.

- Support SSAI, SGAI and hybrid execution environments.

- Support automation and future creative-agent workflows.

- Build on existing market implementations rather than requiring
  unnecessary replacement.

- Preserve reasonable compatibility with existing Ad Management API
  integrations.

- Determine which capabilities must be standardized even when
  proprietary creative ingest/review APIs remain in use.

- Avoid prescribing seller auction-risk decisions when standardized
  information can enable those decisions instead.

# Scope

## In Scope

- Live CTV and streaming environments

- Programmatic and direct-sold creative preparation workflows

- Proactive creative registration

- Auction-triggered creative discovery

- Creative ingest and review interfaces

- Ad Management API evolution

- Proprietary API interoperability requirements

- SSAI, SGAI and hybrid ad insertion environments

- Creative validation

- Creative approval

- Operational preparation

- Creative distribution

- Readiness and eligibility signaling

- Unknown and pending-review creative states

- Structured feedback

- Multi-party validation

- Creative identity and version relationships

- Reuse of previous creative review and processing where identity can be
  established

## Out of Scope

Unless subsequently determined necessary by a Worksheet decision:

- defining individual publisher content moderation policies;

- measurement and attribution standards;

- player UX design;

- defining implementation architecture of creative agents;

- standardizing commercial campaign decisioning logic;

- requiring a specific transcoding or CDN implementation;

- prescribing whether a seller must serve, reject or replace an unknown
  creative that appears on a winning bid.

# Stakeholder Use Cases

## Buyers / Advertisers

Primary use cases include:

- discovering applicable creative requirements;

- pre-validating creatives;

- registering creatives across relevant supply paths;

- understanding whether a creative is already known or reviewed;

- understanding approval and readiness status;

- managing creative versions and last-minute swaps;

- determining event-specific eligibility;

- avoiding campaign underdelivery caused by unready creatives;

- receiving actionable feedback when auction activity triggers review.

## DSPs

Primary use cases include:

- policy and requirement discovery;

- pre-bid eligibility filtering;

- proactive creative submission;

- determining whether a supply platform already knows a creative;

- creative submission orchestration;

- interoperability with AMA and proprietary ingest APIs;

- consolidated readiness state management;

- deal-aware routing;

- buyer-facing remediation feedback;

- readiness-aware bidding logic;

- understanding whether an unknown creative may bid but not yet serve;

- avoiding repeatedly submitting creatives already known to supply
  platforms.

## SSPs / Marketplaces

Primary use cases include:

- exposing common readiness requirements;

- supporting publisher-specific overlays;

- accepting proactive creative submissions;

- discovering new creatives through auction traffic;

- inspecting creatives from all bids or selected bids;

- determining whether a creative ID is known;

- initiating review for unknown creatives;

- allowing, rejecting or deferring unknown creatives according to seller
  policy;

- falling back to a previously reviewed creative when the winning bid
  contains an unknown creative;

- asynchronously reviewing unknown winning creatives for future
  auctions;

- routing creatives to applicable downstream validation authorities;

- consolidating approval and readiness results;

- maintaining a reusable store of known reviewed creatives;

- communicating pending, approved and rejected states upstream;

- preserving existing proprietary creative workflows;

- optionally exposing standardized AMA interfaces;

- translating proprietary downstream results into standardized readiness
  semantics.

## Streaming Platforms

Primary use cases include:

- declaring platform-level creative requirements;

- applying publisher or event-specific overlays;

- identifying applicable execution environments;

- ensuring creative distribution to delivery systems;

- providing readiness feedback upstream;

- supporting multiple SSAI, SGAI or hybrid architectures;

- determining whether auction-discovered creatives can be operationally
  prepared quickly enough for live execution.

## SSAI / DAI Providers

Primary use cases include:

- receiving eligible creative assets before execution;

- validating media compatibility;

- transcoding and packaging;

- caching;

- maintaining prepared media identity;

- communicating operational readiness;

- handling multiple downstream delivery paths;

- reusing previously prepared assets where safe;

- avoiding unnecessary just-in-time processing during peak concurrency;

- distinguishing a newly observed creative ID from an already processed
  media asset.

## SGAI Systems / Player Execution Environments

Primary use cases include:

- validating server-side ad decision and signaling readiness;

- validating asset accessibility;

- determining player/device support;

- verifying compatibility with relevant manifest and signaling models;

- ensuring advertisements can be retrieved and executed when directed;

- distinguishing server readiness from end-to-end execution readiness.

## Publishers / Media Owners

Primary use cases include:

- publisher policy enforcement;

- event-specific restrictions;

- inventory-specific restrictions;

- approval and rejection;

- readiness auditing;

- delegating some requirements to platforms or marketplaces;

- applying additional requirements beyond shared platform policies;

- determining whether reactive validation is permissible for particular
  premium inventory;

- requiring proactive approval for selected events or inventory.

## Creative Agents / Automation Systems

Primary use cases include:

- discovering machine-readable creative requirements;

- determining whether a creative is already known;

- validating a creative against multiple environments;

- creating compliant variants;

- remediating known failures;

- identifying environments in which a creative is ready;

- integrating with AMA or compatible proprietary creative ingest/review
  APIs.

## Ad Operations Teams

Primary use cases include:

- monitoring approvals;

- monitoring operational readiness;

- identifying unknown or newly observed creatives;

- monitoring auction-triggered review queues;

- identifying unresolved dependencies before an event;

- managing last-minute swaps;

- troubleshooting failure ownership;

- viewing event readiness dashboards;

- preserving a decision and audit trail.

# Definitions

## Ad Creative Readiness

The state in which an advertising creative has satisfied the applicable
policy, technical and operational requirements necessary to participate
reliably in a specified live advertising execution environment.

Creative Readiness may be reached through proactive preparation or
through a reactive lifecycle initiated when a creative is first
discovered in auction activity.

Contextual eligibility may be a component of readiness or a separate
determination. This remains a Worksheet decision.

## Proactive Readiness

A workflow in which creative registration, validation, approval and
applicable operational preparation occur before the relevant auction or
delivery opportunity.

## Auction-Triggered Readiness

A workflow in which creative review or preparation begins because a
previously unknown creative is observed through programmatic auction
activity.

## Known Creative

A creative whose identity can be reliably associated with previous
review or readiness evidence held by the relevant validation authority.

The exact identity rules remain a Worksheet decision.

## Delivery Environment

The collection of systems, platforms and operational workflows
responsible for preparing and executing advertising within a streaming
context.

## Validation Authority

An entity responsible for determining whether a creative satisfies one
or more applicable requirements.

Examples include:

- SSPs;

- marketplaces;

- publishers;

- streaming platforms;

- ad servers;

- SSAI providers;

- SGAI systems;

- other operational intermediaries.

## Creative Ingest / Review API

Any standardized or proprietary machine-to-machine interface used to
submit creatives, request validation or review, retrieve status, receive
feedback or otherwise manage creative approval and readiness.

The Ad Management API is one example.

# Conclusion

Live event creative preparation is not a single approval step. It is a
distributed lifecycle involving buyers, demand platforms, marketplaces,
publishers, streaming infrastructure and multiple execution
architectures.

That lifecycle may begin well before an event through proactive
registration, or it may begin when a creative is first observed through
programmatic auction activity.

Creative Readiness should support both.

The objective is not to dictate whether a seller accepts the risk of
serving an unknown creative or chooses a previously reviewed fallback
bid. The objective is to make the creative\'s identity, status,
applicable requirements and readiness evidence sufficiently
interoperable that each participant can make that decision predictably.

The project should build on the Ad Management API where it provides a
useful common standard while acknowledging proprietary creative ingest
and review systems as an existing market reality.

The working group\'s task is to determine the **minimum interoperable
capabilities and semantics required for a creative to move predictably
from creation or first observation to reliable live execution**.

# Version 0.01 to 0.02 Change Log

Changes incorporated into this draft include:

- Added distributed validation authority model.

- Expanded stakeholder use cases.

- Added SSAI operational readiness considerations.

- Added SGAI and hybrid execution scenarios.

- Added creative distribution and execution-path requirements.

- Added proprietary creative ingest/review API adoption problem.

- Removed assumption that AMA must necessarily be the exclusive Creative
  Readiness implementation.

- Added alternatives for AMA requirements and recommendations.

- Added Proposal + Worksheets collaboration methodology.

- Added potential near-term AMA v1.2 workstream.

- Added longer-term AMA v2 / broader Creative Readiness workstream.

- Added phased project deliberation model.

- Clarified that readiness and contextual eligibility remain an open
  decision.

- Expanded compatibility considerations for existing AMA v1.x
  implementations.

- Added proactive and auction-triggered Creative Readiness models.

- Added current market patterns where platforms inspect all bid
  creatives or only winning creatives.

- Added unknown-winning-creative options including serve-and-review,
  fallback-and-review and reject-until-ready.

- Clarified that seller auction disposition should not necessarily be
  standardized.

- Added known-creative lookup and review-reuse considerations.

# Open Questions

Major unresolved questions include:

- Must AMA be implemented to claim Creative Readiness interoperability?

- Should AMA instead be the recommended reference implementation?

- What minimum capabilities must proprietary creative ingest/review APIs
  support?

- Are readiness and contextual eligibility separate concepts?

- Can a creative legitimately participate in an auction before it is
  ready to serve?

- Which readiness dimensions, if any, must be complete before bidding?

- Should UNKNOWN, PENDING_REVIEW or equivalent concepts be standardized?

- Should buyers be able to query whether a creative is already known to
  a platform?

- What identity is sufficient to reuse previous creative review?

- Should seller treatment of an unknown winning creative remain entirely
  proprietary?

- Can platforms declare that particular live inventory requires
  proactive readiness?

- What entity or context owns a readiness state?

- What constitutes an immutable creative identity?

- When must a changed creative be revalidated?

- How should policy requirements be discovered?

- Which operational readiness concepts apply consistently across SSAI
  and SGAI?

- How should multi-party feedback be normalized?

- Which Creative Readiness improvements can responsibly ship in AMA
  v1.2?

- Which improvements require AMA v2 or another standard?


---

Worksheets

> Live Event Ad Creative Readiness Worksheets

[Purpose 1](#purpose)

[**How to Use the Worksheets 1**](#how-to-use-the-worksheets)

[**Proposed Decision-Making Journey
2**](#proposed-decision-making-journey)

[**Worksheet Priority 2**](#worksheet-priority)

[**Worksheet Decision Output Template
4**](#worksheet-decision-output-template)

[**Latest Notes (8/27/2026) 4**](#latest-notes-8272026)

# Purpose

These Worksheets are the working group\'s detailed deliberation
environment for the Live Event Ad Creative Readiness Proposal. The
proposal itself should remain a concise record of current consensus and
available as the highest version number under the Proposal
tab.

Worksheets preserve:

- questions

- alternatives

- supporting facts

- existing market behavior

- implementation realities

- objections

- dependencies

- dissent

- and decision history

# How to Use the Worksheets

Each Worksheet contains:

- **Decision Question -** What must the group decide?

- **Why This Matters -** Why the question should probably be resolved at
  that point.

- **Known Facts / Current Observations -** Information already gathered.

- **Options / Questions for Deliberation -** Alternatives requiring
  working group discussion.

- **Dependencies -** Other decisions affected by or dependent on this
  Worksheet.

- **Provisional Direction -** Current best hypothesis, not working group
  consensus.

- **Final Decision -** To be completed by the working group.

- **Proposal Impact -** What must change in the main proposal after
  resolution.

The rankings are deliberately challengeable. The working group may
reorder them whenever dependencies or market feedback indicate a better
sequence.

# Proposed Decision-Making Journey

## ![](md-extract/media/media/image3.png){width="6.5in" height="4.333333333333333in"}

# Worksheet Priority 

1.  Definition and Readiness Dimensions\
    (Everything depends on what "ready" means)

2.  Readiness vs. Eligibility\
    (Prevents readiness from being conflated with contextual permission)

3.  Validation Authority\
    (Determines who may create or contribute readiness decisions)

4.  Delivery Environment\
    (Establishes the context in which readiness has meaning)

5.  Creative Identity and Versioning\
    (Required before status or prior review can reliably follow a
    creative)

6.  AMA vs. Proprietary API Strategy\
    (Determines whether later work describes one API or interoperable
    capabilities)

7.  Near-Term AMA v1.2\
    (Determines what value can be delivered without waiting for the full
    project)

8.  AMA v1.x / v2 Compatibility\
    (Constrains how broader changes can be introduced)

9.  Creative Policy Discovery\
    (Defines how buyers learn requirements before submission or bidding)

10. Validation Inputs / Requirement Schema\
    (Defines what discovered requirements contain)

11. Structured Feedback and State Model\
    (Defines how validation results and unknown states return upstream)

12. Auction-Triggered Creative Discovery and Validation\
    (Establishes how the lifecycle works when creatives are first seen
    during auction traffic)

13. SSAI Operational Readiness\
    (Defines preparation requirements for major server-side live
    workflows)

14. SGAI and Hybrid Execution Readiness\
    (Ensures readiness is not defined exclusively around SSAI)

15. Multi-SSAI / Multi-Execution Distribution\
    (Resolves preparation across more than one execution path)

16. Creative Transformation Rights\
    (Determines when downstream processing changes creative
    identity/readiness)

17. Creative Distribution, Caching and Reuse\
    (Determines when preparation is complete and when prior work can be
    reused)

18. Deal and Supply-Path Routing\
    (Applies the model to transaction context)

19. Creative Agents and Automation\
    (Depends on machine-readable policies, states and API strategy)

20. Last-Minute Changes and Operational Exceptions\
    (Best resolved once the normal lifecycle is stable)

# Worksheet Decision Output Template

When a Worksheet reaches sufficient consensus, record:

- **Decision:** \[Working group conclusion\]

- **Normative / Recommended / Informational:** \[Classification\]

- **Reasoning:** \[Concise rationale\]

- **Dissent / Alternative Views:** \[If applicable\]

- **Affected Use Cases:** \[List\]

- **Affected Standards:** \[AMA v1.2 / AMA v2 / other\]

- **Proposal Changes Required:** \[Specific sections\]

- **Implementation Guidance Needed:** \[Yes / No + description\]

# Latest Notes (8/27/2026)

The Worksheets currently:

- formalize the Worksheet collaboration methodology;

- rank decisions by presumed dependency and relevance;

- provide a phased deliberation journey;

- explicitly address low AMA adoption;

- compare requiring AMA, exclusively recommending AMA, and recommending
  AMA alongside interoperable proprietary APIs;

- provisionally identify the third option as the most pragmatic starting
  hypothesis;

- separate potential near-term AMA v1.2 work from longer-term Creative
  Readiness work;

- expand SSAI operational analysis;

- include SGAI and hybrid execution readiness;

- include multi-execution-path scenarios;

- include creative transformation, distribution and reuse questions;

- explicitly recognize proactive and auction-triggered Creative
  Readiness workflows;

- document current marketplace patterns for reviewing all observed
  creatives, winning creatives, serving while reviewing and falling back
  while reviewing;

- introduce known-creative identity and reuse as a foundational problem;

- preserve seller discretion over unknown-creative auction treatment;

- allow high-concurrency live environments to potentially require
  proactive readiness;

- retain unresolved decisions rather than prematurely hardcoding them
  into the proposal.

Phase 1

1 -Definition and Readiness Dimensions

Worksheet 1\
Definition and Readiness Dimensions

Every subsequent Worksheet depends upon what the group believes
readiness includes.

# Decision Question

What does "Ad Creative Readiness" mean?

# Known Facts / Current Observations

The current working definition includes four broad dimensions.

## Policy and Compliance Readiness

The "may we?" question.

Potential considerations:

- publisher policy;

- platform policy;

- category restrictions;

- legal requirements;

- regional restrictions;

- event restrictions.

## Technical Readiness

The "can this asset execute?" question.

Potential considerations:

- codec;

- bitrate;

- dimensions;

- duration;

- MIME type;

- audio;

- metadata;

- signaling;

- device compatibility.

## Operational / Supply-Chain Readiness

The "has everything necessary actually happened?" question.

Potential considerations:

- creative registration;

- downstream distribution;

- transcoding;

- packaging;

- caching;

- ID mapping;

- preparation by execution systems.

## Timing / Contextual Readiness

The "is this appropriate for this opportunity now?" question.

Potential considerations:

- break duration;

- event;

- program;

- competitive separation;

- pacing;

- supply path.

# Questions for Deliberation

- Are these four dimensions correct?

- Is policy approval part of readiness or a separate concept?

- Is contextual eligibility actually a readiness dimension?

- Must all dimensions be satisfied before a creative can be called
  "ready"?

- Does readiness describe only a completed condition, or should the
  standardized lifecycle also represent unknown, pending-review and
  partially prepared creatives?

- Can a creative legitimately enter an auction before it is ready?

# Provisional Direction

Retain the readiness dimensions as the analytical framework while
distinguishing the **readiness lifecycle** from the final condition of
being ready.

The lifecycle may begin through:

- proactive registration; or

- observation of a creative during auction activity.

# Final Decision

TBD

2 - Readiness vs. Contextual Eligibility

> Worksheet 2\
> Readiness vs. Contextual Eligibility

# Decision Question

Is "ready" an intrinsic state of a creative, or is final eligibility
always contextual?

# Example

A creative could simultaneously be:

- technically prepared for Platform A;

- approved by Publisher A;

- prohibited by Publisher B;

- eligible for Event X;

- too long for a specific available break.

# Options

## Option A --- Eligibility is the final readiness state

Simple but risks conflating preparation with transaction context.

## Option B --- Readiness and eligibility are separate

A creative may be operationally ready while eligibility is evaluated for
a particular opportunity.

## Option C --- Hierarchical model

Base readiness exists independently with contextual eligibility layered
over it.

# Provisional Direction

Option B or C appears more scalable than treating LIVE_ELIGIBLE as a
universal intrinsic state.

### Final Decision

TBD

3 - Validation Authority

> Worksheet 3\
> Validation Authority

# Decision Question

Who is allowed or required to determine readiness?

# Candidate Validation Authorities

- SSP / marketplace

- publisher

- media owner

- streaming platform

- ad server

- SSAI provider

- SGAI system

- other execution platform

# Questions

- Can multiple authorities independently contribute status?

- Can one authority act on behalf of another?

- Can a marketplace aggregate publisher requirements?

- Must the buyer know every downstream authority?

- How is authority identity represented?

# Provisional Direction

Support distributed validation rather than assuming one publisher equals
one validation authority.

# Final Decision

TBD

4 - Delivery Environment

> Worksheet 4\
> Delivery Environment

# Decision Question

What is the context against which technical and operational readiness is
evaluated?

# Examples

A delivery environment may encompass:

- streaming platform;

- publisher ad server;

- SSAI configuration;

- SGAI/player architecture;

- CDN;

- device family;

- manifest protocol;

- event-specific infrastructure.

# Questions

- Does a delivery environment require a persistent identifier?

- Can one platform expose several delivery environments?

- How granular should the concept be?

- Does the buyer need to know the environment directly?

# Final Decision

TBD

5 - Creative Identity and Versioning

> Worksheet 5\
> Creative Identity and Versioning

# Decision Question

How do systems determine that they are discussing the same creative
asset?

# Why It Matters

Readiness cannot reliably follow a creative across systems without a
stable identity model.

Auction-triggered workflows make this especially important because
marketplaces must determine whether a creative observed during an
auction is truly new.

# Candidate Identifiers

- DSP creative ID

- SSP creative ID

- crid

- mid

- UniversalAdID

- media file URL

- media hash

- creative fingerprint

# Key Questions

- Must UniversalAdID be required?

- Is media identity different from advertising-creative identity?

- What change creates a new creative version?

- Can readiness be inherited after metadata-only changes?

- How should media hash or URL reuse affect transcoding readiness?

- Can two different creative IDs safely reuse prior policy review?

- Can two creative IDs safely reuse prior technical processing?

- What identity is sufficient to say "we have seen this creative
  before"?

# Final Decision

TBD

Phase 2

6 - AMA vs. Proprietary APIs

> Worksheet 6\
> Ad Management API vs. Proprietary Creative Ingest / Review APIs

# Decision Question

Should Creative Readiness depend on adoption of the IAB Tech Lab Ad
Management API?

# Known Facts / Current Observations

- AMA provides a standardized creative submission and audit model.

- Existing AMA adoption appears limited relative to the total market.

- Multiple SSPs, marketplaces and other platforms already operate
  proprietary creative ingest/review APIs.

- Some proprietary systems are tightly integrated with live auction
  creative discovery.

- Requiring replacement of functioning proprietary workflows may slow
  Creative Readiness adoption.

- Allowing every proprietary API to express readiness differently would
  undermine interoperability.

- Existing AMA users should not be penalized for adopting the Tech Lab
  standard.

# Option A --- Require AMA

Creative Readiness compliance requires use of AMA.

## Advantages

- strongest API interoperability;

- one standardized implementation;

- accelerates AMA adoption.

## Risks

- migration cost;

- potentially slow adoption;

- may exclude otherwise capable systems;

- could require marketplaces to replace working auction-integrated
  review systems.

# Option B --- Recommend AMA Exclusively

AMA is the only standardized/recommended API, but proprietary
implementations remain possible.

# Option C --- Recommend AMA and Standardize Required Capabilities for Any API

AMA becomes the preferred/reference implementation while proprietary
APIs may satisfy Creative Readiness if they expose defined functionality
and semantics.

Potential capabilities include:

- creative submission or reference;

- known-creative lookup;

- applicable validation-context identification;

- requirement discovery;

- creative/version identity;

- multi-authority status;

- unknown/pending/approved/rejected states;

- structured failure feedback;

- readiness retrieval;

- contextual association such as deal/publisher/environment;

- update/revalidation behavior;

- extensibility/versioning.

# Provisional Direction

**Option C currently appears most pragmatic.**

Recommend AMA as the reference and preferred standardized interface
while defining minimum Creative Readiness capabilities and semantics
that proprietary APIs should support.

# Final Decision

TBD

7 - Near-Term AMA v1.2 Opportunity

> Worksheet 7\
> Near-Term AMA v1.2 Opportunity

# Decision Question

Can useful Creative Readiness improvements be delivered through an
additive AMA v1.2 before the full project is complete?

# Existing Proposal

The prior AMA v1.2 proposal introduces publisher-level audit capability
using publisher IDs and deal IDs.

Potential value includes:

- multiple publisher approvals;

- marketplace coordination;

- deal-to-publisher mapping;

- distinct exchange and publisher audit states.

# Additional Question from Auction-Triggered Workflows

Should AMA v1.2 also consider a lightweight mechanism for determining
whether a creative is already known or has existing audit state?

This should only be included if it can remain narrow and
backward-compatible.

# Guardrail

AMA v1.2 should avoid prematurely encoding unresolved Creative Readiness
architecture.

# Final Decision

TBD

8 - AMA v1.x Compatibility and Potential AMA v2

> Worksheet 8\
> AMA v1.x Compatibility and Potential AMA v2

# Decision Question

How can new functionality be introduced without unnecessarily disrupting
AMA v1.x adopters?

# Considerations

- preserve existing creative submission patterns;

- favor additive fields;

- avoid requiring every publisher to operate an API;

- support marketplaces representing many publishers;

- allow optional readiness extensions;

- support proactive and reactive workflows;

- define version negotiation;

- determine what requires a major version.

# Potential AMA v2 Topics

- policy discovery;

- generalized validation authorities;

- readiness lifecycle states;

- known-creative lookup;

- operational execution readiness;

- auction-triggered review;

- broader context mapping;

- agentic workflows.

# Final Decision

TBD

Phase 3

9 - Creative Policy Discovery

> Worksheet 9\
> Creative Policy Discovery

# Decision Question

How does a buyer learn creative requirements before submission or
bidding?

# Options

- URI exposed through AMA;

- API endpoint returning requirements;

- standardized policy document;

- marketplace-level aggregate policy;

- base policy plus publisher/event overlays;

- equivalent discovery through proprietary APIs.

# Additional Auction-Time Question

Should a platform be able to declare that a particular inventory
context:

- permits unknown creatives to serve;

- permits unknown creatives to bid but not serve;

- requires proactive approval before serving;

- requires full operational preparation before bidding?

# Final Decision

TBD

10 - Validation Inputs / Requirement Schema

> Worksheet 10\
> Validation Inputs / Requirement Schema

# Decision Question

What requirements must be expressible in machine-readable policies?

# Candidate Categories

- media type;

- codec;

- bitrate;

- resolution;

- aspect ratio;

- duration;

- audio;

- container;

- VAST requirements;

- identifiers;

- tracking;

- privacy macros;

- SSAI constraints;

- SGAI constraints;

- event restrictions;

- content/category restrictions;

- device limitations;

- proactive registration requirement;

- unknown-creative treatment where appropriate.

# Final Decision

TBD

11 - Structured Feedback and Readiness States

> Worksheet 11\
> Structured Feedback and Readiness States

# Decision Question

What status and failure information must return to the buyer?

# Candidate Feedback Dimensions

- validation authority;

- requirement identifier;

- status;

- reason;

- remediability;

- affected environment;

- affected publisher/deal;

- revalidation requirement;

- timestamp/version.

# Candidate Lifecycle Concepts

The working group should evaluate whether standardized semantics are
needed for:

- UNKNOWN

- OBSERVED

- PENDING_REVIEW

- APPROVED

- REJECTED

- TECHNICALLY_READY

- OPERATIONALLY_READY

- EXPIRED

- REVALIDATION_REQUIRED

These names are illustrative, not proposed normative values.

# Questions

- Is there one readiness state or multiple dimensions?

- Should readiness be derived rather than declared?

- Are standard error codes required?

- How should proprietary APIs map native statuses?

- Is "unknown creative" a standardized state or simply absence of a
  record?

- Should auction-triggered review be visible to the DSP?

# Final Decision

TBD

Phase 4

12 - Auction-Triggered Validation

> Worksheet 12\
> Auction-Triggered Creative Discovery and Validation

# Decision Question

How should Creative Readiness support workflows in which a creative is
first discovered, reviewed or prepared through programmatic auction
activity rather than advance registration?

# Why This Matters

The group should first understand:

- creative identity;

- API strategy;

- applicable requirements;

- readiness/status semantics.

Only then can it consistently describe what happens when an unknown
creative appears during an auction.

This Worksheet should precede SSAI and SGAI operational readiness
because auction-triggered discovery determines how much downstream
preparation can have occurred before the first attempted impression.

# Known Facts / Current Market Behavior

Not every platform requires creative registration before bidding.

Some platforms use programmatic auctions themselves as the creative
discovery mechanism.

## Pattern A --- Review All Observed Bid Creatives

The platform inspects creatives appearing across received bids,
regardless of whether the bid wins.

**Potential advantages**

- builds the platform\'s known-creative inventory rapidly;

- increases probability that future winners are already reviewed;

- may reduce first-win latency later.

**Potential disadvantages**

- consumes review/scanning resources for creatives that may never serve;

- potentially increases media processing and storage cost;

- difficult at very high bid volume.

## Pattern B --- Review Winning Creatives

The platform begins review when a previously unknown creative appears on
a winning bid.

**Potential advantages**

- concentrates resources on creatives with demonstrated ability to win;

- limits unnecessary processing.

**Potential disadvantages**

- requires a decision about the current impression before review is
  complete.

## Pattern C --- Serve While Reviewing

The platform permits the unknown winning creative to render while review
begins asynchronously.

**Potential advantages**

- preserves the highest auction value;

- minimizes immediate demand loss.

**Risks**

- first impression may violate policy;

- technical incompatibility may be discovered only after attempted
  delivery;

- inappropriate for some high-risk or premium environments.

## Pattern D --- Fall Back While Reviewing

The unknown winning creative does not serve.

The platform awards the impression to the next eligible bid containing a
previously reviewed creative while simultaneously reviewing the original
winning creative for later opportunities.

**Potential advantages**

- reduces creative risk;

- retains newly discovered demand for subsequent auctions.

**Potential disadvantages**

- seller may receive a lower price;

- original winning buyer loses the current opportunity;

- additional auction logic is required.

## Pattern E --- Reject Until Ready

The creative cannot serve until all required approval or preparation is
complete.

**Potential advantages**

- strongest control;

- consistent with restrictive approval policies.

**Potential disadvantages**

- creates lost opportunities until processing completes;

- potentially discourages new creative participation.

# Important Distinction

These workflows suggest two primary paths into Creative Readiness.

## Proactive Readiness

Creative registration, validation and operational preparation occur
before relevant auction opportunities.

## Reactive / Auction-Triggered Readiness

The creative is discovered through auction activity and the readiness
lifecycle begins because the creative has been observed.

Both paths may ultimately produce the same readiness evidence.

They differ primarily in **when the lifecycle begins**.

# Relationship to Restrictive Bidding

Restrictive bidding does not necessarily mean an unknown creative cannot
appear in a bid.

It means the seller requires approval before that creative may serve.

A possible workflow is:

Unknown creative wins\
→ review triggered\
→ creative cannot serve yet\
→ next ready bid used\
→ review completes\
→ creative becomes eligible for future auctions.

This is different from prohibiting the buyer from bidding altogether.

# Questions for Deliberation

- Should proactive and auction-triggered workflows both be first-class
  Creative Readiness models?

- Is advance registration required for high-concurrency live events,
  recommended, or an implementation choice?

- Should unknown creatives be allowed to bid?

- Should the standard distinguish "may bid" from "may serve"?

- Is a standardized UNKNOWN or PENDING_REVIEW concept useful?

- Is "first time observed" useful standardized information or
  implementation detail?

- Should platforms communicate that review was triggered by auction
  activity?

- Should a DSP be able to determine before bidding whether a creative is
  already known?

- Could AMA or a proprietary API expose known-creative lookup?

- What identity is sufficient to safely reuse previous review?

- What creative changes invalidate review?

- Does changing only tracking URLs invalidate policy approval?

- Does a media URL change imply new review?

- Can content hashes support technical-processing reuse?

- Should publisher approval remain independent when marketplace review
  already exists?

- How does an auction-triggered creative subsequently enter SSAI or SGAI
  preparation?

- Should high-concurrency inventory be able to declare that proactive
  readiness is mandatory?

# Auction Disposition

The standard could expose information allowing a seller to choose among:

1.  **Serve** --- allow the unknown creative and assume risk.

2.  **Fallback** --- choose another ready bid.

3.  **Reject** --- prevent the creative from participating further.

4.  **Defer** --- allow future participation only after review.

The working group should determine whether standardized terminology for
these outcomes is useful.

The actual auction decision should likely remain a seller/platform
implementation policy.

# Live Event Considerations

Reactive validation may be acceptable for ordinary programmatic
inventory but materially less attractive during high-concurrency events.

Live events amplify:

- review bursts;

- transcoding demand;

- cache misses;

- infrastructure load;

- first-impression failure risk;

- revenue loss from fallback;

- operational uncertainty.

A Creative Readiness policy may therefore need to indicate that specific
live inventory requires proactive registration or completion of specific
readiness dimensions before serving.

# Relationship to AMA vs. Proprietary APIs

This use case strengthens the case for standardizing capabilities rather
than assuming one submission path.

AMA and proprietary APIs could expose equivalent semantics for:

- whether a creative is known;

- current review state;

- applicable validation authorities;

- previous approval;

- readiness evidence;

- revalidation requirements.

Auction traffic could act as another trigger into the same readiness
lifecycle.

# Provisional Direction

Support both proactive and auction-triggered readiness.

Do not require the standard to dictate whether an unknown winning
creative is served, rejected or replaced.

Instead, standardize enough:

- identity;

- state;

- policy;

- feedback;

- readiness evidence;

for each platform to make that decision predictably.

For premium high-concurrency environments, allow validation authorities
to declare that advance registration or selected readiness dimensions
are mandatory before a creative may serve.

# Final Decision

TBD

13 - SSAI Operational Readiness

> Worksheet 13\
> SSAI Operational Readiness

# Decision Question

What must be true before a creative is operationally ready for
traditional SSAI?

# Candidate Requirements

- creative received;

- media accessible;

- transcoding complete;

- required renditions created;

- packaging complete;

- cache availability;

- media identity known;

- trackers preserved;

- creative mapped to ad-serving identity.

# Auction-Triggered Consideration

A creative discovered during an auction may be policy acceptable but
operationally unprepared for SSAI.

The group should determine:

- whether such a creative can ever serve immediately;

- whether fallback is required;

- whether previously processed media can be recognized and reused.

# Final Decision

TBD

14 - SGAI/Hybrid Readiness

> Worksheet 14\
> Server-Guided Ad Insertion and Hybrid Execution Readiness

# Decision Question

How should Creative Readiness account for SGAI, where server-side
preparation and client-side execution responsibilities differ from
traditional SSAI?

# Observations

SGAI can involve:

- server-side ad decisioning;

- server-generated ad pod information;

- manifest or break signaling;

- client/player retrieval;

- client-side playback execution;

- device/player dependencies.

A creative may be server-ready but not end-to-end ready.

# Potential Readiness Dimensions

## Server Preparation

- decisioning complete;

- ad pod established;

- creative metadata available;

- URLs accessible.

## Signaling

- manifest/break signaling valid;

- required SGAI metadata present;

- protocol support available.

## Client Capability

- supported player;

- supported device/OS;

- media format compatibility;

- access to resources.

## Execution

- asset retrievable;

- tracking supported;

- expected playback behavior available.

# Auction-Triggered Consideration

A newly discovered creative may require less centralized preprocessing
than in some SSAI implementations, but that does not necessarily mean it
is immediately execution-ready across all clients.

# Final Decision

TBD

15 - Multiple SSAI / Execution Paths

> Worksheet 15\
> Multiple SSAI / Execution Paths

## **Decision Question**

How is readiness represented when the same inventory can be delivered
through multiple execution infrastructures?

# Example

A platform may use:

- SSAI Provider A for one device family;

- SSAI Provider B for another;

- SGAI for selected devices;

- alternate infrastructure for redundancy.

# Questions

- Is readiness environment-specific?

- Must a creative be prepared everywhere before being called ready?

- Can readiness be expressed as a set of prepared execution paths?

- How does auction-triggered validation interact with multiple paths?

# Final Decision

TBD

16 - Creative Transformation Rights

> Worksheet 16\
> Creative Transformation Rights

## **Decision Question**

What transformations may downstream systems perform before prior
readiness or identity becomes invalid?

# Examples

- transcoding;

- bitrate ladder generation;

- audio normalization;

- container changes;

- resizing;

- slate insertion;

- media replacement.

# Questions

- Which transformations are expected operational preparation?

- Which create a materially different creative?

- Does transformed media inherit policy approval?

- Can a newly observed creative reuse previously transformed media?

- What buyer authorization is required?

# Final Decision

TBD

17 - Creative Distribution, Caching and Reuse

> Worksheet 17\
> Creative Distribution, Caching and Reuse

# Decision Question

How does the ecosystem determine that the creative has reached every
system required for execution, and when can previous processing be
reused?

# Questions

- Who distributes the creative downstream?

- Does the publisher ad server push to SSAI?

- Does SSAI pull from the original media URL?

- Can an SSP coordinate preparation?

- How are cached/transcoded assets matched to later bids?

- Can prepared media be reused when a new creative ID references
  identical media?

- Can auction-time recognition avoid redundant processing?

# Potential Identity Inputs

- media URL;

- content hash;

- mezzanine identifier;

- UniversalAdID;

- system-specific media ID.

# Final Decision

TBD

Phase 5

18 - Deal and Supply-Path Routing

> Worksheet 18\
> Deal and Supply-Path Routing

# Decision Question

How does a buyer determine which readiness requirements and validation
authorities apply to a deal or supply path?

# Existing Market Insight

A buyer may know:

- SSP;

- seat;

- deal ID;

without knowing every downstream publisher or platform.

# Questions

- Does the deal map to applicable publishers?

- Does the deal map to a policy set?

- Does the marketplace perform routing?

- Does Deal Sync provide context?

- Can the deal declare proactive readiness requirements?

- Are auction-triggered workflows allowed for some deals but prohibited
  for others?

# Final Decision

TBD

19 - Creative Agent and Automation Compatibility

> Worksheet 19\
> Creative Agent and Automation Compatibility

# Decision Question

What Creative Readiness capabilities must be machine-consumable so
future creative agents can use them?

# Candidate Capabilities

- discover requirements;

- determine whether a creative is already known;

- validate assets;

- select compatible formats;

- identify failed constraints;

- remediate;

- generate variants;

- select supply paths where a creative is ready;

- submit proactively;

- react to auction-triggered pending status;

- integrate with AMA or compatible proprietary APIs.

# Final Decision

TBD

20 - Last-Minute Changes - Operational Exceptions

> Worksheet 20\
> Last-Minute Changes and Operational Exceptions

# Decision Question

What happens when a creative changes after some or all readiness work
has completed?

# Scenarios

- media replacement;

- tracking-only change;

- metadata correction;

- new VAST wrapper;

- late advertiser substitution;

- emergency creative pull;

- replacement immediately before an event.

# Questions

- Which changes invalidate policy approval?

- Which invalidate transcoding?

- Which cause a previously "known" creative to become unknown?

- Which require a new creative identity?

- Can readiness be partially inherited?

- Can auction-triggered review safely recover a late replacement?

- When should live inventory prohibit reactive recovery and require
  proactive revalidation?

# Final Decision

TBD
