# ORLogin Agent Workspace & Knowledge Base

This directory contains persistent domain knowledge, reverse-engineered memory specifications, architectural rules, and agent skills for the **ORLogin** project.

## Directory Layout

- **[`knowledge/`](knowledge/)**:
  - [`architecture_and_security.md`](knowledge/architecture_and_security.md): Detailed specification of the cryptographic vault, in-memory protection, and process introspection.
  - [`decisions_and_learnings.md`](knowledge/decisions_and_learnings.md): Chronological record of architectural decisions, bugs resolved, and discoveries.
  - [`ro_memory_map.md`](knowledge/ro_memory_map.md): Authoritative virtual address table for OSRO Highrate (and Midrate) clients.
- **[`skills/`](skills/)**:
  - [`ro-client-login-automation/SKILL.md`](skills/ro-client-login-automation/SKILL.md): Comprehensive agent skill guide covering RO memory structures, login detection, window discovery, input simulation, and secure credential handling.

## Essential Invariants

1. **Security**: Never write plaintext passwords or encryption keys to disk, logs, or un-wiped memory.
2. **Elevated Execution**: Memory operations on `OSRO.exe` require Administrator privileges (embedded in [`ORLogin.manifest`](../ORLogin.manifest)).
3. **State Detection**: Never rely on `OnlineAddress (0x010A2FB0)` or character name presence alone. Always check `InGameAddress (0x010DCCBC) == 1` and `ModeAddress (0x0107AA84) == 2`.
