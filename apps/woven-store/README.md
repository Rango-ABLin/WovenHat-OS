# WovenStore

WovenStore is a bounded Ring-3 client of the `woven.package` service. It lists
`/share/applications` descriptors and can request Query, Install, Remove and
Rollback transactions through the authenticated IPC boundary. The app holds no
`StorageIo`; package bytes, signature policy, dependency solving and the
durable transaction remain owned by the privileged service.

Modes are `--list`, `--query PATH`, `--install PATH`, `--remove PATH`,
`--rollback PATH` and `--help`. The GUI provides the same actions with a
bounded path field and a status line.
