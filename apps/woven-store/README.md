# WovenStore

WovenStore is a bounded read-only catalog browser for installed applications.
It enumerates `/share/applications` and reports `.app` descriptors without
installing, removing, or trusting packages. Package acquisition, signature
policy, dependency solving, and transactional installation remain owned by a
future package service.

Modes are `--list` and `--help`.
