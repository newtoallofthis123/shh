# Security Policy

`shh` handles environment-variable secrets and macOS Keychain access, so please
report suspected vulnerabilities privately.

## Supported versions

The supported code line is the current `main` branch and the latest published
release, if one exists. Older commits and local forks may not receive fixes.

## Reporting a vulnerability

Do not open a public issue for vulnerabilities or leaked secrets.

Preferred reporting path:

1. Use GitHub's private vulnerability reporting or Security Advisories feature
   for this repository, if available.
2. If private reporting is not available, contact a maintainer through their
   GitHub profile and ask for a private disclosure channel.

Include enough detail to reproduce and assess the issue:

- Affected version or commit.
- macOS version and CPU architecture.
- Exact command or workflow that exposes the issue.
- Expected behavior and actual behavior.
- Whether real secrets, Keychain items, logs, or crash reports were exposed.

Please do not include live API keys, passwords, tokens, or other real secrets in
the report. Use fake values when possible.

## Response expectations

Maintainers will try to acknowledge valid reports promptly, investigate the
impact, and coordinate a fix before public disclosure. Security fixes may be
released without detailed public exploit information until users have had a
reasonable chance to update.
