# Security policy

## Supported versions

Version `0.1.0` of `ph-surfaces` is the supported release. After later
versions exist, older releases may be assessed case by case; users should
expect to upgrade to the latest compatible release.

## Reporting a vulnerability

Do not disclose a suspected vulnerability in a public issue or discussion.
Use this repository's
[private GitHub security-advisory interface](https://github.com/photon-circus/ph-surfaces/security/advisories/new)
when it is available. If that option is unavailable, open a
[minimal contact-request issue](https://github.com/photon-circus/ph-surfaces/issues/new?title=Private%20security%20contact%20requested&body=Private%20security%20contact%20requested.)
with that title and body only; a maintainer will arrange a private channel.

Include:

- the affected crate and version;
- a description of the issue and its impact;
- reproduction steps or a minimal example;
- any suggested mitigation, if known.

Reports are acknowledged privately. A coordinated fix and disclosure follow
when the issue is confirmed.

## Scope

`ph-surfaces` is a `no_std` math library. Relevant concerns include panics on
public paths, reachable integer overflow or wraparound on public paths, and
any future unsafe code (currently forbidden). Evaluation is specified not to
overflow for any surface this crate can define.

Issues limited to interpolation results, documented rounding, or resource
accounting are ordinary correctness bugs unless disclosure would create a
concrete security risk; report those through the public issue tracker.
