# Security policy

Designer is pre-alpha and has no supported production release. Do not treat
untrusted files as safe simply because they have a .dsg extension.

Report vulnerabilities privately using the repository's GitHub Security Advisory
reporting feature once enabled. Before a public repository exists, contact its
maintainer privately; do not open a public exploit report. Include affected
revision, minimal reproduction and impact, without credentials or private files.

Before publication, enable private vulnerability reporting. Maintain bounded
archive decoding, path validation, asset size limits, version validation and
current-user-only local IPC. MCP must never provide arbitrary shell execution.
No fixed response time is promised at this stage.
