# Security Policy

## Reporting Vulnerabilities

Debuggatha handles sensitive information (API keys, workspace source code) and must maintain strict security standards. We take vulnerability reports seriously.

### How to Report

**Do NOT open public GitHub issues for security vulnerabilities.**

Instead, email: **security.sxnnyside@sxnnysideproject.com**

### Required Information

Include in your report:

1. **Description**  
   Clear explanation of the vulnerability

2. **Impact**  
   What can an attacker do? What data is at risk?

3. **Reproduction Steps**  
   Step-by-step instructions to trigger the issue

4. **Environment**  
   - VS Code version  
   - Debuggatha version  
   - Operating system  
   - AI provider (if relevant)

5. **Suggested Fix** (optional)  
   Proposed mitigation or patch

### Response Timeline

- **Acknowledgment**: Within 48 hours
- **Initial Assessment**: Within 5 business days
- **Coordinated Disclosure**: We will work with you on a reasonable timeline

### What Qualifies as a Security Issue?

- API key exposure or leakage
- Credential storage vulnerabilities
- Code injection via prompt manipulation
- Remote code execution
- Privilege escalation within VS Code
- Unsafe file read/write operations
- Cross-site scripting (XSS) in webview
- Sensitive data logging or telemetry

### Out of Scope

The following are NOT considered security vulnerabilities:

- AI model hallucinations or incorrect code suggestions
- Rate limiting bypass (this is an API provider concern)
- Performance issues or resource exhaustion without security impact
- UI rendering bugs without data exposure
- Third-party AI provider vulnerabilities (report to them directly)

### Safe Harbor

We will not pursue legal action against security researchers who:

- Report vulnerabilities responsibly via the designated channel
- Do not exploit vulnerabilities beyond proof-of-concept
- Do not disclose vulnerabilities publicly before coordinated release
- Do not access or modify data that is not their own

### Supported Versions

We provide security updates for:

- Latest stable release (currently v2.x)
- Previous major version for 6 months after new major release

Older versions are not supported. Please upgrade.

---

**Thank you for helping keep Debuggatha secure.**
