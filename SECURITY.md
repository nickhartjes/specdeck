# Security

How SpecDeck handles your data, what the sharing service can see, and the known limits of the current release are described in [Security and privacy](https://nickhartjes.github.io/specdeck/security/overview/) in the documentation.

## Reporting a vulnerability

Please report vulnerabilities privately, not in a public issue, discussion or pull request.

1. **On GitHub:** use [Report a vulnerability](https://github.com/nickhartjes/specdeck/security/advisories/new) on the repository's Security tab.
2. **By email**, if you cannot use GitHub: write to `security@hartj.es`.

Please include:

- what is affected: the app (and which platform), the sharing service, the web viewer, or the Docker Compose or Kubernetes deployment;
- the SpecDeck version;
- how to reproduce it, and what an attacker could do with it.

SpecDeck is maintained by one person, in their own time, so answers are best effort. You will hear back once the report has been read, and again when there is a fix. Please give a reasonable time to fix it before you publish anything.

Issues already on the [known limits](https://nickhartjes.github.io/specdeck/security/known-limits/) list are known; a report is still welcome if you found a way to make one worse.

## Supported versions

Only the latest release gets security fixes. The app updates itself on most installs; see [Update](https://nickhartjes.github.io/specdeck/install/#update).
