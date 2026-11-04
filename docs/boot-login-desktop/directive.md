# WOVENHAT OS — MASTER AUTONOMOUS IMPLEMENTATION DIRECTIVE

## Project: Complete Bootloader, System Initialization, Secure Login and Graphical Desktop Startup

**Repository:** https://github.com/Rango-ABLin/WovenHat-OS.git

**Operating System:** WovenHat OS  
**Primary Language:** Rust  
**Architecture:** x86_64, UEFI  
**Target:** Native, independently bootable operating system  
**Development Agents:** Claude Code and OpenAI Codex  
**Project Objective:** Implement a complete, secure, performant and visually polished boot-to-login-to-desktop experience.

**Core principle:** WovenHat OS must remain a genuine Rust-based operating system, not a Linux distribution or a graphical simulation running on another operating system.

---

# PART 1 — YOUR MISSION

You are a principal operating systems architect, senior Rust kernel engineer, security engineer, graphics engineer, and release engineer working on WovenHat OS.

Your assignment is to inspect the existing WovenHat OS repository and implement a complete, production-oriented startup architecture.

The required startup sequence is:

```text
POWER ON
    |
    v
UEFI FIRMWARE
    |
    v
WOVENHAT UEFI BOOTLOADER
    |
    v
KERNEL LOADING
    |
    v
KERNEL INITIALIZATION
    |
    +-- Memory Management
    +-- CPU and SMP Initialization
    +-- Interrupt Controllers
    +-- Scheduler
    +-- Device Discovery
    +-- Storage and File Systems
    +-- Graphics and Input
    +-- Security Infrastructure
    |
    v
SYSTEM INITIALIZATION PROCESS
    |
    +-- Essential System Services
    +-- Device Management
    +-- Logging
    +-- User and Session Services
    +-- Display Infrastructure
    |
    v
WOVENHAT BOOT SPLASH
    |
    v
WOVENLOGIN DISPLAY MANAGER
    |
    +-- User Discovery
    +-- Authentication
    +-- Session Creation
    +-- Security Policy
    |
    v
AUTHENTICATED USER SESSION
    |
    v
WOVENSHELL DESKTOP
    |
    +-- Desktop Compositor
    +-- Bottom Taskbar
    +-- Start Menu
    +-- Window Management
    +-- Application Launcher
    +-- System Tray
    +-- Notifications
    +-- Settings
    |
    v
FULLY OPERATIONAL WOVENHAT OS
```

This is not a request for a demonstration, mockup, architectural document, or isolated prototype.

Implement the actual software, integrate it into the existing OS, test it, and provide reproducible evidence.

Do not claim production readiness until all mandatory acceptance gates have passed.

---

# PART 2 — NON-NEGOTIABLE ENGINEERING RULES

1. Inspect the existing repository before making architectural decisions.
2. Preserve working kernel functionality and previously completed milestones.
3. Reuse existing boot, framebuffer, scheduler, userspace, IPC, VFS, graphics, and WovenShell components wherever possible.
4. Do not introduce a second competing bootloader, scheduler, graphics stack, or session architecture unnecessarily.
5. Do not assume a component exists merely because it is mentioned in documentation.
6. Identify the actual source files, interfaces, entry points, and current behavior.
7. Avoid unrelated rewrites.
8. Keep kernel responsibilities separate from userspace responsibilities.
9. Implement security at the kernel and privileged-service boundaries, not merely in the interface.
10. Ensure that no graphical authentication screen can bypass real authorization.
11. Do not use Linux, GNOME, GDM, systemd, X11, Wayland, or another operating system as the underlying runtime.
12. Do not introduce large dependencies that are incompatible with the existing Rust target or freestanding environment.
13. Prefer small, independently testable changes.
14. Keep CI green and preserve 1-, 2-, and 4-CPU acceptance testing.
15. Never suppress failing tests merely to obtain a green build.
16. Never hardcode passwords, authentication secrets, or privileged bypasses.
17. Never mark an untested implementation as complete.
18. Do not delete, reset, force-push, or rewrite another agent's work.
19. Keep WovenWeb development paused; it is outside the scope of this assignment.
20. Continue implementation autonomously through all feasible software milestones.

**Important:** Both agents must follow the same repository coordination protocol and must never make concurrent uncoordinated edits to the same files.

---

# PART 3 — REPOSITORY DISCOVERY AND BASELINE AUDIT

Before writing implementation code, perform a complete repository inspection.

## 3.1 Identify the current architecture

Locate and inspect:

- UEFI bootloader and EFI application entry points.
- Kernel entry point and boot information structures.
- Framebuffer initialization.
- Physical and virtual memory managers.
- Paging and memory protection.
- GDT, IDT, interrupts, APIC and timer initialization.
- Scheduler and SMP startup.
- ELF userspace loader.
- Process creation and lifecycle.
- System call entry and dispatch.
- IPC and synchronization primitives.
- VFS and storage drivers.
- WovenFS and FAT32 integration.
- Input device handling.
- Graphics rendering pipeline.
- WovenShell window manager.
- Desktop userspace.
- Existing authentication or credential infrastructure.
- Existing initialization or service management.
- QEMU build and launch scripts.
- CI workflows.
- Architecture and roadmap documents.

Do not invent source paths. Discover the real paths and record them.

## 3.2 Establish the baseline

Record:

```text
Repository:
Branch:
Commit SHA:
Rust toolchain:
Cargo version:
Target triple:
Host operating system:
QEMU version:
OVMF firmware:
Build command:
Test command:
Image generation command:
Known passing suites:
Known failing suites:
Existing boot behavior:
Existing desktop behavior:
```

Run the existing checks before changing anything.

If the baseline fails, distinguish pre-existing failures from regressions introduced by this assignment.

## 3.3 Produce an architecture inventory

Create:

```text
docs/boot-login-desktop/
    architecture.md
    baseline.md
    component-inventory.md
    dependency-map.md
    acceptance-matrix.md
    security-model.md
    implementation-ledger.md
```

These files must describe the real repository, not hypothetical implementations.

---

# PART 4 — BOOTLOADER IMPLEMENTATION

## Objective

Make the WovenHat bootloader reliably locate, validate, load, and transfer control to the kernel.

## Required capabilities

### 4.1 UEFI entry

Implement or verify:

- UEFI application entry point.
- Firmware environment detection.
- Boot volume access.
- Kernel image discovery.
- Boot configuration parsing.
- Framebuffer discovery.
- Memory map acquisition.
- ACPI table discovery.
- Kernel image validation.
- Kernel loading and relocation where applicable.
- Stack preparation.
- Boot information handoff.
- Correct UEFI ExitBootServices sequencing.
- Transfer of control to the kernel.

Reuse the existing implementation whenever it already satisfies these requirements.

### 4.2 Boot configuration

Provide a versioned boot configuration supporting:

```text
Default boot entry
Kernel image path
Root filesystem selection
Boot timeout
Graphics mode
Diagnostic mode
Recovery mode
Serial logging
Kernel command line
```

Reject malformed configuration safely.

Never interpret arbitrary untrusted configuration as executable instructions.

### 4.3 Boot failure handling

Provide meaningful failure behavior for:

- Missing kernel.
- Corrupted kernel.
- Unsupported image format.
- Invalid boot configuration.
- Memory allocation failure.
- Unsupported graphics mode.
- Failure to exit UEFI boot services.

Never display a fake successful boot screen after a fatal failure.

### 4.4 Secure boot architecture

Design a verifiable kernel image trust mechanism.

Where supported, integrate with UEFI Secure Boot appropriately.

Do not claim Secure Boot compatibility unless signatures, trust anchors, firmware configuration, and verification behavior have been tested.

Document the distinction between image integrity checking and authenticated boot.

---

# PART 5 — KERNEL STARTUP AND SYSTEM READINESS

## Objective

Provide deterministic kernel initialization and a reliable handoff to the first userspace process.

## Required startup stages

```text
EARLY KERNEL
    -> Boot information validation
    -> Early console
    -> Physical memory initialization
    -> Virtual memory initialization
    -> CPU initialization
    -> Interrupt infrastructure
    -> Scheduler initialization
    -> SMP startup
    -> Device discovery
    -> Storage initialization
    -> Root filesystem mounting
    -> Graphics and input initialization
    -> Security initialization
    -> First userspace process
```

Do not assume every service must initialize serially. Preserve existing safe parallel initialization where supported.

## 5.1 Kernel readiness state

Introduce a clear readiness contract.

Examples:

```rust
enum KernelReadiness {
    EarlyBoot,
    MemoryReady,
    SchedulerReady,
    DevicesReady,
    RootFilesystemReady,
    UserspaceReady,
    Operational,
    Degraded,
    Failed,
}
```

This is an illustrative interface. Adapt it to the existing architecture.

## 5.2 Startup diagnostics

Record:

- Startup stage.
- Start and completion timestamps.
- Success or failure.
- Error code.
- Recovery action.
- CPU identifier where relevant.

Support serial diagnostics during early boot.

Do not require the graphical system to function before errors can be reported.

## 5.3 Multicore stability

Preserve all existing SMP functionality.

Test:

- 1 CPU.
- 2 CPUs.
- 4 CPUs.

Investigate any scheduler handoff, task ownership, interrupt nesting, or lock-ordering regression.

Specifically check the previously observed nested scheduler hand-off failure on the single-CPU path.

Do not introduce blocking waits in interrupt context.

---

# PART 6 — WOVENINIT SYSTEM INITIALIZATION

## Objective

Implement or extend the first userspace initialization process.

Preferred component name:

**WovenInit**

If the repository already contains a suitable initialization process, extend it rather than creating a duplicate.

## 6.1 Responsibilities

WovenInit must:

1. Become the designated first userspace process.
2. Initialize required userspace infrastructure.
3. Launch essential system services.
4. Track service lifecycle and readiness.
5. Handle dependency ordering.
6. Detect unexpected service termination.
7. Restart eligible services using bounded policies.
8. Collect startup errors.
9. Launch the graphical display manager when prerequisites are satisfied.
10. Coordinate shutdown and reboot.

## 6.2 Service model

Implement a minimal service description format or typed Rust service registry.

Support:

```text
Service name
Executable path
Dependencies
Startup policy
Restart policy
Privilege requirements
Readiness contract
Timeout
Resource limits
```

Avoid implementing a complex service-management ecosystem unnecessarily.

## 6.3 Suggested service graph

```text
WovenInit
    |
    +-- Device Service
    +-- Logging Service
    +-- Storage Service
    +-- Security Service
    +-- Authentication Service
    +-- Session Service
    +-- Display Service
    +-- Input Service
    |
    +-- WovenLogin
```

Adapt this graph to actual kernel/userspace capabilities.

Not every item must be a separate process. A small OS may initially use fewer processes while preserving clear interfaces.

## 6.4 Recovery behavior

If a nonessential service fails:

- Log the failure.
- Retry according to policy.
- Preserve operational services.
- Avoid infinite restart loops.

If an essential service fails:

- Enter a controlled degraded or recovery state.
- Provide serial diagnostics.
- Do not silently enter a privileged desktop session.

---

# PART 7 — WOVENBOOT GRAPHICAL SPLASH

## Objective

Create a professional graphical startup experience.

Preferred name:

**WovenBoot**

## 7.1 Visual requirements

Use WovenHat's established visual identity.

Design language:

- Minimal.
- Premium.
- Clean typography.
- Dark startup background.
- Centered WovenHat logo.
- Subtle loading animation.
- Smooth visual transition.
- No excessive gradients or decorative clutter.

Suggested presentation:

```text
           WOVENHAT LOGO

              WovenHat

         Weave Will into Honor

                • • •
```

The startup splash must reflect actual system readiness.

Do not use a timer to pretend booting has completed.

## 7.2 Rendering architecture

Prefer the existing framebuffer or graphics abstraction.

The splash must not require the entire desktop compositor to initialize.

Support:

- Resolution discovery.
- DPI-aware layout.
- Aspect ratio preservation.
- Correct framebuffer stride.
- Pixel format handling.
- Safe memory bounds.
- Double buffering where available.
- Reduced animation when performance is constrained.

## 7.3 Transition to login

When the display manager is ready:

1. Stop or transfer ownership of the splash surface.
2. Synchronize display ownership.
3. Clear or replace the previous frame.
4. Present the login interface.
5. Avoid flashing, tearing, or stale graphics.

Use explicit ownership and readiness synchronization.

---

# PART 8 — WOVENLOGIN GRAPHICAL DISPLAY MANAGER

## Objective

Implement a complete native graphical login manager.

Component name:

**WovenLogin**

WovenLogin must run as a userspace application with only the permissions required to present the interface and request authentication.

## 8.1 Login screen design

Implement:

- WovenHat branding.
- Current time and date.
- User selection.
- User avatar or fallback icon.
- Password input.
- Keyboard navigation.
- Sign-in action.
- Authentication feedback.
- Power menu.
- Restart action.
- Shutdown action.
- Accessibility controls.
- Session status.
- Secure login state transitions.

The login screen must be responsive to different resolutions.

## 8.2 User interface quality

The interface must look like a modern, carefully designed operating system.

Requirements:

- Consistent typography.
- Proper spacing.
- High-contrast controls.
- Crisp graphics.
- Smooth cursor movement.
- Correct keyboard focus.
- Visible hover and focus states.
- No clipping.
- No overlapping text.
- No broken scaling.
- No placeholder UI elements presented as functional controls.

## 8.3 Password handling

Password input must:

- Mask entered characters.
- Avoid logging secrets.
- Avoid unnecessarily retaining plaintext credentials.
- Use bounded buffers.
- Clear sensitive buffers when practical.
- Never store passwords in graphical state longer than necessary.

Authentication must be performed by a privileged authentication service, not by comparing plaintext passwords inside the display manager.

## 8.4 Authentication states

Implement an explicit state machine:

```text
Locked
    |
    v
UserSelected
    |
    v
CredentialsEntered
    |
    v
Authenticating
    |
    +-- Failed -> Locked/UserSelected
    |
    +-- Success -> SessionCreation
                         |
                         v
                    SessionReady
                         |
                         v
                    DesktopLaunch
```

Reject invalid state transitions.

Prevent duplicate session launches caused by repeated clicks or keyboard events.

---

# PART 9 — USER ACCOUNTS AND AUTHENTICATION

## Objective

Implement real local user authentication.

## 9.1 Account model

Support:

```text
User identifier
Username
Display name
Credential verifier
Account status
Privilege level
Home directory
Preferred session
Security metadata
```

Separate public profile information from privileged credential records.

## 9.2 Credential storage

Use a well-reviewed password hashing and key-derivation implementation suitable for the target.

Prefer Argon2id when supported by the actual environment.

Use:

- Unique per-user salts.
- Appropriate memory and iteration parameters.
- Versioned credential records.
- Constant-time comparison where applicable.
- Secure random generation.
- Strict storage permissions.

Do not invent custom password cryptography.

Do not use plaintext, reversible encryption, unsalted SHA-256, or hardcoded credentials.

If secure entropy or suitable cryptographic primitives are unavailable, implement those prerequisites first and block password authentication until they are ready.

## 9.3 Privileged authentication service

Authentication must be mediated through a restricted IPC interface.

The service must:

- Validate requests.
- Enforce account policy.
- Apply rate limits.
- Avoid username enumeration where practical.
- Log security events without logging credentials.
- Return minimal authentication results.
- Prevent unauthorized clients from minting sessions.

## 9.4 First-run account creation

Implement a secure initial account setup flow.

The first user must be able to create an account during first-run setup.

Do not ship a known default password.

Do not automatically create an unrestricted administrator session.

If the system cannot securely provision an initial account, provide an explicit setup/recovery mode rather than bypassing authentication.

---

# PART 10 — SESSION MANAGER AND PRIVILEGE ISOLATION

## Objective

Create secure authenticated desktop sessions.

Preferred component name:

**WovenSession**

## 10.1 Session lifecycle

Implement:

```text
Authentication success
    -> Privileged session authorization
    -> User identity assignment
    -> Process security context
    -> User environment initialization
    -> Home directory access
    -> Desktop process launch
    -> Session monitoring
    -> Logout
    -> Credential/session cleanup
    -> Return to login
```

## 10.2 Security requirements

The session manager must enforce:

- User identity boundaries.
- Process privilege separation.
- File access permissions.
- IPC access controls.
- Session ownership.
- Resource cleanup.
- Session termination.
- No arbitrary privilege escalation.

The desktop must not run with kernel privileges.

Do not rely on environment variables or user-supplied identifiers as proof of authentication.

Session authorization must be backed by kernel-enforced credentials or equivalent protected capabilities.

## 10.3 Lock and logout

Support:

- Lock session.
- Unlock session.
- Log out.
- Switch user where architecture permits.
- Restart.
- Shutdown.

Unlock must require valid authentication.

Logging out must terminate or safely detach session-owned processes according to policy.

---

# PART 11 — WOVENSHELL DESKTOP INTEGRATION

## Objective

Launch the existing WovenShell desktop only after successful session creation.

Do not replace WovenShell with an unrelated desktop environment.

## 11.1 Required desktop elements

Implement or integrate:

- Native desktop compositor.
- Bottom taskbar.
- WovenHat Start button.
- Start menu.
- Application launcher.
- Running application indicators.
- Window focus.
- Window movement.
- Window resizing.
- Minimize and maximize.
- System tray.
- Clock.
- Network indicator.
- Volume control.
- Battery status when supported.
- Notifications.
- Settings entry point.
- Lock and logout controls.

## 11.2 Desktop design

Use the established WovenHat desktop direction:

```text
+----------------------------------------------------+
|                                                    |
|                                                    |
|                                                    |
|                DESKTOP WORKSPACE                   |
|                                                    |
|                                                    |
|                                                    |
+----------------------------------------------------+
| Start | Applications       | Network | Clock | ... |
+----------------------------------------------------+
```

**Critical requirement:** The taskbar must be at the bottom.

Do not introduce a GNOME-style top bar, Activities overlay, or workspace preview.

Use a taskbar height of approximately 50 logical pixels, adjusted appropriately for scaling.

## 11.3 Graphics engineering

Investigate and correct the existing desktop-userspace failures.

Pay particular attention to:

- Framebuffer pixel format.
- Pitch/stride calculations.
- Alpha blending.
- Buffer ownership.
- Surface lifetime.
- Dirty rectangle handling.
- Cursor rendering.
- Font rasterization.
- Window clipping.
- Z-order.
- Screen scaling.
- Input event routing.
- Frame synchronization.
- Memory allocation failures.
- Userspace/kernel graphics ABI compatibility.

Avoid unnecessary full-screen redraws.

Ensure all rendering respects buffer bounds and pixel format.

## 11.4 Startup contract

WovenShell must:

1. Receive an authorized session context.
2. Initialize graphical resources.
3. Register input handlers.
4. Initialize window management.
5. Launch desktop components.
6. Signal desktop readiness.
7. Become interactive.
8. Report failures to the session manager.

A desktop process must not report readiness before its required components are operational.

---

# PART 12 — INPUT, ACCESSIBILITY AND DISPLAY RELIABILITY

Implement or verify:

- Keyboard initialization.
- Mouse initialization.
- Focus handling.
- Keyboard layouts.
- Tab navigation.
- Enter and Escape behavior.
- Password-field focus.
- Cursor visibility.
- Display resolution changes.
- Scaling.
- High-contrast presentation.
- Clear error messaging.

The system must remain usable if a mouse is unavailable.

Provide keyboard-only login and shutdown/recovery navigation.

Avoid exposing sensitive data through accessibility or debugging interfaces.

---

# PART 13 — ERROR HANDLING AND RECOVERY

## 13.1 Boot failure

If the kernel cannot load, provide a firmware-compatible error path or recovery entry where supported.

## 13.2 Kernel initialization failure

Provide serial diagnostics and a controlled failure mode.

## 13.3 WovenInit failure

Do not continue into an uncontrolled privileged environment.

## 13.4 Display manager failure

Attempt bounded recovery.

If graphical login cannot start, offer a secure text-mode recovery/login path only when the necessary authentication and privilege enforcement mechanisms are available.

## 13.5 Authentication failure

Return to a safe login state without crashing or exposing account data.

## 13.6 Desktop failure

If WovenShell crashes:

- Preserve session integrity.
- Record diagnostic information.
- Restart according to a bounded policy.
- Return to login or recovery when restart limits are reached.

Never bypass authentication to recover from a graphical failure.

---

# PART 14 — PERFORMANCE AND RESOURCE MANAGEMENT

Optimize startup without sacrificing correctness.

Measure:

```text
Firmware-to-bootloader time
Bootloader-to-kernel time
Kernel initialization time
Userspace initialization time
Login readiness time
Authentication latency
Desktop startup time
Time to first interactive frame
Peak boot memory usage
Idle CPU usage
Desktop frame latency
```

Establish baseline measurements before optimization.

Prefer:

- Dependency-aware service startup.
- Efficient graphics buffering.
- Minimal allocations in hot paths.
- Reduced unnecessary redraws.
- Bounded queues.
- Correct synchronization.
- Lazy initialization of nonessential desktop services.

Do not invent performance measurements.

Record actual timings from reproducible tests.

---

# PART 15 — CLAUDE CODE AND CODEX COLLABORATION PROTOCOL

Both agents must operate as cooperating engineers on one project.

Neither agent owns the entire repository.

The repository and its committed state are the shared source of truth.

## 15.1 Coordination files

Create or reuse:

```text
docs/agent-coordination.md

docs/boot-login-desktop/
    implementation-ledger.md
    work-ownership.md
    handoff.md
    test-results.md
    blockers.md
    acceptance-matrix.md
```

Do not create competing coordination documents if suitable ones already exist.

## 15.2 Task ownership

Each work item must record:

```text
Task ID:
Component:
Assigned agent:
Branch:
Base commit:
Owned files:
Dependencies:
Current status:
Implementation summary:
Tests executed:
Test results:
Known issues:
Next action:
```

## 15.3 Agent responsibilities

Suggested initial division:

### Claude Code

Primary focus:

- Bootloader.
- Kernel startup.
- WovenInit.
- Service lifecycle.
- Authentication service.
- Security architecture.
- Session authorization.
- Recovery behavior.

### OpenAI Codex

Primary focus:

- WovenBoot graphics.
- WovenLogin graphical interface.
- WovenShell integration.
- Desktop rendering fixes.
- Input and accessibility.
- Graphical integration testing.
- Desktop performance.

These are starting responsibilities, not permanent restrictions.

Either agent may continue another agent's work after acquiring ownership.

## 15.4 Shared-file coordination

Before editing:

1. Synchronize with the integration branch.
2. Read the current coordination ledger.
3. Inspect active work ownership.
4. Confirm file ownership.
5. Claim a bounded task.
6. Work on a dedicated branch or isolated worktree.
7. Record changes and tests.
8. Commit the completed unit.
9. Publish the branch when authorized.
10. Request integration or handoff.

A coordination document alone is not an atomic lock.

Use branch protection, pull requests, and an explicit integration owner or another reliable locking mechanism to prevent conflicting changes.

Do not concurrently modify the same files without coordination.

## 15.5 Branch strategy

Suggested naming:

```text
feature/boot-startup
feature/woveninit
feature/wovenboot
feature/wovenlogin
feature/authentication
feature/session-manager
feature/desktop-startup
fix/desktop-userspace
test/boot-login-integration
```

Use the repository's existing branching conventions where available.

Never force-push shared branches.

Never overwrite another agent's unmerged work.

## 15.6 Handoff requirements

At the end of each work unit, update:

```text
docs/boot-login-desktop/handoff.md
```

Include:

- What changed.
- Commit identifiers.
- Files changed.
- Interface changes.
- Tests passed.
- Tests failed.
- Remaining issues.
- Exact next action.
- Any ownership restrictions.

The receiving agent must inspect actual code and commits rather than trusting a summary alone.

## 15.7 Resumption protocol

Whenever Claude or Codex starts a new session:

```text
1. Inspect repository status.
2. Identify the current branch and HEAD.
3. Read coordination files.
4. Inspect active PRs and commits.
5. Review acceptance results.
6. Identify incomplete work.
7. Check task ownership.
8. Resume the highest-priority unblocked task.
9. Preserve existing working changes.
10. Continue implementation.
```

Never restart the project from scratch merely because the conversational context was lost.

---

# PART 16 — IMPLEMENTATION PHASES

Complete the following phases in dependency order.

Parallelize only when interfaces and file ownership are stable.

## Phase A — Baseline and architecture

Deliver:

- Repository audit.
- Architecture inventory.
- Existing startup trace.
- Known failures.
- Component dependency graph.
- Security model.
- Acceptance matrix.

**Exit gate:** Existing architecture and integration points are understood.

## Phase B — Bootloader and kernel handoff

Deliver:

- Verified UEFI boot.
- Kernel image loading.
- Boot information contract.
- Startup diagnostics.
- Controlled failure handling.

**Exit gate:** Kernel boots reliably through the supported QEMU configurations.

## Phase C — Kernel readiness and WovenInit

Deliver:

- Kernel readiness signaling.
- First userspace process.
- Essential service startup.
- Service dependency handling.
- Startup failure recovery.

**Exit gate:** Essential userspace services start deterministically.

## Phase D — Graphics and boot splash

Deliver:

- WovenBoot implementation.
- Correct framebuffer rendering.
- Startup progress presentation.
- Display ownership transition.

**Exit gate:** Splash transitions to the display manager without corruption or deadlock.

## Phase E — Authentication infrastructure

Deliver:

- User account model.
- Secure credential storage.
- Authentication service.
- IPC authorization.
- First-run account setup.
- Rate limiting.
- Security tests.

**Exit gate:** Invalid credentials cannot create a privileged session.

## Phase F — WovenLogin

Deliver:

- Complete graphical login screen.
- User selection.
- Password input.
- Keyboard navigation.
- Authentication integration.
- Error handling.
- Shutdown/restart actions.

**Exit gate:** Login works through the actual authentication service.

## Phase G — Session management

Deliver:

- Authenticated session creation.
- Kernel-enforced session identity.
- User process context.
- Desktop launch authorization.
- Lock/unlock.
- Logout.
- Session cleanup.

**Exit gate:** Unauthorized processes cannot impersonate authenticated sessions.

## Phase H — WovenShell integration

Deliver:

- Desktop launch.
- Taskbar.
- Start menu.
- Application launching.
- Window management.
- Input integration.
- Graphical rendering corrections.
- Desktop readiness signaling.

**Exit gate:** Authenticated users reach a functional graphical desktop.

## Phase I — Full-system integration

Deliver:

- End-to-end startup.
- Login/logout cycles.
- Failure recovery.
- Multicore testing.
- Storage persistence.
- Security regression tests.
- Performance measurements.

**Exit gate:** All mandatory software acceptance tests pass.

## Phase J — Release candidate

Deliver:

- Reproducible bootable image.
- Documented build process.
- QEMU launcher.
- Release notes.
- Known limitations.
- Hardware qualification plan.
- Verified acceptance report.

**Exit gate:** Release candidate is reproducible and meets the agreed acceptance criteria.

---

# PART 17 — MANDATORY TEST MATRIX

Build automated tests wherever practical.

| Test | Expected result |
|---|---|
| UEFI boot | Kernel loads successfully |
| Missing kernel | Controlled failure |
| Invalid kernel image | Rejected safely |
| Kernel initialization | Reaches userspace |
| 1-CPU boot | Pass |
| 2-CPU boot | Pass |
| 4-CPU boot | Pass |
| WovenInit startup | Essential services ready |
| Splash rendering | Correct display output |
| Splash-to-login transition | No corruption |
| Login display | Fully rendered |
| Invalid password | Authentication rejected |
| Valid password | Session created |
| Repeated failed attempts | Rate limiting enforced |
| Unauthorized session request | Rejected |
| Desktop launch | Only after authorization |
| Desktop interaction | Functional |
| Logout | Returns to login |
| Lock/unlock | Authentication enforced |
| Display manager crash | Controlled recovery |
| Desktop crash | Controlled recovery |
| Restart | Correct shutdown/reboot |
| Storage persistence | User data retained |
| Repeated boot cycles | No unexpected failure |

## 17.1 Negative security testing

Explicitly test attempts to:

- Launch the desktop without authentication.
- Forge a session token.
- Modify a user identifier.
- Read another user's credential records.
- Bypass login through recovery mode.
- Reuse an expired session authorization.
- Overflow login input buffers.
- Exploit malformed IPC messages.
- Trigger unauthorized privilege elevation.

Every test must have a documented expected outcome.

## 17.2 Repeated boot testing

Run repeated boot and login cycles.

Include:

- Cold boots.
- Warm reboots.
- Login/logout loops.
- Invalid authentication attempts.
- Desktop restart loops.
- 1-, 2-, and 4-CPU configurations.

Record actual counts, failures, and logs.

Do not claim reliability based on a single successful boot.

---

# PART 18 — BUILD AND QEMU VALIDATION

Use the repository's real build system.

Do not assume a Cargo package name, feature name, target triple, or firmware path.

Discover them first.

Support the user's Windows development environment where practical.

The build workflow must:

1. Validate Rust toolchain availability.
2. Validate target support.
3. Validate Cargo workspace metadata.
4. Check required features.
5. Build the correct kernel package.
6. Build userspace components.
7. Assemble the bootable image.
8. Locate compatible OVMF firmware.
9. Launch QEMU.
10. Capture serial logs.
11. Preserve diagnostic artifacts.

Avoid the previously encountered errors involving:

- Incorrect kernel package names.
- Missing workspace members.
- Invalid forwarded Cargo features.
- Missing `x86_64-unknown-none` support.
- Invalid Cargo configuration quoting.
- Missing OVMF firmware.
- Incompatible linker configuration.

Do not work around these errors by disabling necessary functionality.

Produce a tested launcher that opens QEMU interactively.

The launcher must report the actual image path and relevant runtime configuration.

---

# PART 19 — CONTINUOUS INTEGRATION

Extend existing CI rather than duplicating pipelines.

Required checks:

```text
Rust formatting
Static analysis
Workspace validation
Kernel build
Userspace build
Boot image generation
Unit tests
Security tests
1-CPU boot tests
2-CPU boot tests
4-CPU boot tests
Authentication tests
Session isolation tests
Graphical startup tests
Desktop integration tests
Regression tests
```

Where graphical automation is unavailable, use serial readiness markers and test instrumentation.

A serial marker alone must not count as proof of correct graphical rendering.

Use screenshots, framebuffer inspection, or other appropriate visual verification for graphical acceptance.

Preserve logs and failure artifacts.

---

# PART 20 — ACCEPTANCE CRITERIA

The implementation is complete only when all mandatory requirements below are satisfied.

## Boot

- [ ] UEFI bootloader loads the kernel.
- [ ] Kernel initialization completes.
- [ ] Startup failures are handled safely.
- [ ] Boot diagnostics are available.

## Initialization

- [ ] WovenInit starts.
- [ ] Essential services become ready.
- [ ] Service failures are handled correctly.

## Graphics

- [ ] WovenBoot renders correctly.
- [ ] Login screen renders correctly.
- [ ] No framebuffer corruption.
- [ ] Input devices work.
- [ ] Display transition is reliable.

## Authentication

- [ ] First-run user setup works.
- [ ] Password verification is secure.
- [ ] Invalid login is rejected.
- [ ] Authentication cannot be bypassed.
- [ ] Credentials are protected.

## Sessions

- [ ] Authorized session creation works.
- [ ] Privilege boundaries are enforced.
- [ ] Desktop launches only after authentication.
- [ ] Lock and unlock work.
- [ ] Logout returns to login.

## Desktop

- [ ] WovenShell starts reliably.
- [ ] Bottom taskbar works.
- [ ] Start menu works.
- [ ] Application launching works.
- [ ] Window management works.
- [ ] Keyboard and mouse work.
- [ ] Rendering is stable.

## Reliability

- [ ] 1-CPU acceptance passes.
- [ ] 2-CPU acceptance passes.
- [ ] 4-CPU acceptance passes.
- [ ] Repeated boot tests pass.
- [ ] Recovery paths work.
- [ ] No critical regression remains.

## Release

- [ ] Bootable image generated.
- [ ] Interactive QEMU launch works.
- [ ] Build instructions are reproducible.
- [ ] CI evidence is recorded.
- [ ] Known limitations are documented.

---

# PART 21 — COMPLETION REPORTING

After each implementation phase, provide:

```text
WOVENHAT OS IMPLEMENTATION REPORT

Phase:
Status:
Base commit:
Current commit:
Branch:
Agent:

Implemented:
Files changed:
Interfaces added:
Security changes:

Build result:
Unit tests:
Integration tests:
QEMU result:
1-CPU result:
2-CPU result:
4-CPU result:

Known failures:
Regressions:
Remaining dependencies:

Next task:
Next agent:
Handoff reference:
```

Use only these status values:

```text
NOT STARTED
IN PROGRESS
BLOCKED
IMPLEMENTED — UNVERIFIED
TESTED — PASS
TESTED — FAIL
COMPLETE
```

A phase may be marked COMPLETE only when its exit gate has passed and the evidence is recorded.

---

# PART 22 — AUTONOMOUS EXECUTION INSTRUCTIONS

Begin immediately.

Do not stop after writing a plan.

Execute this sequence:

1. Inspect the repository.
2. Establish the actual baseline.
3. Identify existing components.
4. Create the coordination and acceptance records.
5. Select the highest-priority unblocked task.
6. Implement the smallest complete integration unit.
7. Compile.
8. Run relevant tests.
9. Diagnose failures.
10. Correct the implementation.
11. Re-run regression tests.
12. Commit the verified work.
13. Update the handoff ledger.
14. Continue to the next task.

When a task depends on unfinished work from the other agent, move to another independent task rather than making conflicting edits.

If an external dependency, hardware requirement, permission boundary, or unavailable capability prevents progress, record the exact blocker and continue with other feasible tasks.

Do not fabricate test results.

Do not mark work complete merely because the code compiles.

Do not create a second implementation when an existing one can be corrected.

Do not abandon the current architecture without demonstrating why it cannot satisfy the requirements.

Do not publish releases, merge protected branches, or perform destructive repository operations without the required authorization.

Continue until all feasible implementation and software verification tasks are complete.

---

# PART 23 — FINAL SYSTEM EXPECTATION

When WovenHat OS is started in QEMU or on supported hardware, the user should experience:

**Step 1:** Computer powers on.

**Step 2:** UEFI firmware loads the WovenHat bootloader.

**Step 3:** The bootloader loads the Rust kernel.

**Step 4:** The kernel initializes hardware, memory, scheduling, storage, graphics and security.

**Step 5:** WovenInit starts essential services.

**Step 6:** A polished WovenHat splash screen appears during startup.

**Step 7:** The system transitions to the WovenLogin graphical screen.

**Step 8:** The user selects an account and enters a password.

**Step 9:** The authentication service validates credentials.

**Step 10:** WovenSession creates a secure user session.

**Step 11:** WovenShell launches the desktop.

**Step 12:** The user can interact with the taskbar, Start menu, windows, applications and system controls.

**Step 13:** Locking, logging out, restarting and shutting down behave correctly.

The entire process must be implemented using the actual WovenHat OS architecture.

# FINAL COMMAND TO BOTH AGENTS

**CLAUDE CODE AND OPENAI CODEX:**

You are jointly responsible for delivering the WovenHat OS boot-to-login-to-desktop milestone.

Treat this directive as a shared engineering specification.

Treat the repository, committed implementation, coordination ledger and test evidence as the authoritative state.

Claude must be able to continue Codex's work.

Codex must be able to continue Claude's work.

Neither agent may overwrite the other's progress.

Implement real functionality, not visual simulations.

Prioritize correctness, security, stability, performance, maintainability and graphical quality.

Preserve all working WovenHat OS milestones.

Start by inspecting the actual repository and identifying the current boot path.

Then proceed through implementation, integration, testing and release-candidate preparation.

**The assignment ends only when the complete boot-to-login-to-desktop workflow has passed its mandatory acceptance gates, or when remaining blockers are explicitly documented with reproducible evidence.**

**BEGIN IMPLEMENTATION NOW.**