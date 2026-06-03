# Security Policy

## Supported Versions

We release patches for security vulnerabilities. Which versions are eligible for receiving such patches depends on the CVSS v3.0 Rating:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please report them via email to: security@nextim.example.com

You should receive a response within 48 hours. If for some reason you do not, please follow up via email to ensure we received your original message.

Please include the following information in your report:

- Type of issue (e.g. buffer overflow, SQL injection, cross-site scripting, etc.)
- Full paths of source file(s) related to the manifestation of the issue
- The location of the affected source code (tag/branch/commit or direct URL)
- Any special configuration required to reproduce the issue
- Step-by-step instructions to reproduce the issue
- Proof-of-concept or exploit code (if possible)
- Impact of the issue, including how an attacker might exploit the issue

This information will help us triage your report more quickly.

## Preferred Languages

We prefer all communications to be in English or Chinese.

## Security Update Process

1. The security report is received and assigned to a primary handler
2. The problem is confirmed and a list of affected versions is determined
3. Code is audited to find any potential similar problems
4. Fixes are prepared for all supported releases
5. New releases are issued and announcements are made

## Security Best Practices

When deploying NextIM, we recommend:

### Network Security
- Use TLS/SSL for all connections (via reverse proxy like nginx)
- Configure firewall rules to restrict access
- Use VPN for administrative access

### Authentication & Authorization
- Use strong, unique passwords
- Implement rate limiting
- Monitor for suspicious activity

### Data Protection
- Enable encryption at rest for sensitive data
- Regular backups with encryption
- Secure key management

### System Hardening
- Keep system and dependencies up to date
- Run services with minimal privileges
- Use security scanning tools

### Monitoring
- Enable audit logging
- Monitor for unusual patterns
- Set up alerts for security events

## Known Security Considerations

### End-to-End Encryption
- E2EE is optional and must be explicitly enabled
- Key management is the user's responsibility
- Backup keys securely to prevent data loss

### Message Signing
- All messages are signed by default
- Verify signatures before trusting messages
- Use appropriate trust levels (Public/TOFU/Verified)

### Node Discovery
- DHT discovery exposes node existence
- Consider using private peer lists for sensitive deployments
- Implement access controls as needed

## Security Advisories

Security advisories will be published on:
- GitHub Security Advisories
- Project website
- Mailing list (if available)

## Acknowledgments

We appreciate the security research community's efforts in responsibly disclosing vulnerabilities. Contributors who report valid security issues will be acknowledged in our security advisories (unless they prefer to remain anonymous).

## Contact

For security-related questions or concerns, please contact:
- Email: security@nextim.example.com
- PGP Key: [To be added]

---

Last updated: 2026-03-21
