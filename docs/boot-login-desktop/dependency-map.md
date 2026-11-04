# Startup dependency map

Status: IN PROGRESS. This records observed edges, not a proposed second stack.

1. Root Cargo kernel artifact -> root build.rs -> UefiBoot -> UEFI disk image.
2. Firmware dependency -> BootInfo -> kernel_main.
3. Kernel memory/CPU/interrupt/scheduler/device setup -> mounted storage/VFS.
4. Persistent account import -> identity_runtime authority.
5. Embedded WovenShell ELF + accounts/settings argv -> session::start ->
   userspace ELF loader -> task::spawn_user_system_service.
6. WovenShell -> compositor claim and input -> sign-in UI when accounts exist.
7. Account digest -> copied syscall request -> AccountManage check ->
   identity_runtime -> owner-bound session and task credentials -> desktop UI.

Audit missing-account startup, F12 diagnostic console authority, child-process
launch policy, service/desktop crash handling, and durable account publication
before treating these edges as a secure end-to-end acceptance result.

Order: finish Phase A baseline; evaluate Phase B boot handoff; then readiness
and init. Preserve integration d21807ba's recorded graphics acceptance and
WovenFS system-root migration; the initial local modernization status was stale.
