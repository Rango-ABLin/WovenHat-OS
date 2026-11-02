# Phase 1 Task 1.2: Execution Instructions

## Stage 10.7 Acceptance Gate Testing

The Stage 10.7 acceptance gate validates all 12 improvements against
the production-tested Ring-3 async TCP feature set.

### How to Execute

Run from PowerShell (Windows Terminal):

```powershell
.\RUN-STAGE10.7.ps1
```

This script will:
1. Set up Cargo build environment
2. Create audit artifact directory with timestamp
3. Start PowerShell transcript for logging
4. Execute run-stage10-7-acceptance.ps1 which:
   - Runs host acceptance-harness regressions (Python tests)
   - Runs cargo clippy -D warnings (lint check)
   - Preserves Stage 10.6 (backward compatibility)
   - Tests Stage 10.7 on 1 CPU
   - Tests Stage 10.7 on 2 CPUs
   - Tests Stage 10.7 on 4 CPUs
5. Collects regression logs
6. Produces acceptance report

### Expected Duration

- Compilation: ~2-3 minutes (Release build)
- Host tests: ~1 minute
- Lint check: ~30 seconds
- Stage 10.6 preservation: ~30 seconds per CPU (1-2 minutes total)
- Stage 10.7 tests: ~10-15 minutes per CPU (30-45 minutes total)

**Total: ~45 minutes to 1 hour**

### Success Criteria

PASS when:
- All Python tests pass (0 failures)
- Clippy lint: 0 warnings
- Stage 10.6 preservation: PASS
- Stage 10.7 on 1 CPU: PASS
- Stage 10.7 on 2 CPUs: PASS
- Stage 10.7 on 4 CPUs: PASS
- All improvements integrated and working

FAIL if any stage returns non-zero exit code

### Output

Results saved to:
```
audit-artifacts/acceptance-YYYYMMDD-HHMMSS-fff/
├── acceptance.txt          (PowerShell transcript)
├── acceptance-output.txt   (test output)
└── [regression-logs]/
    ├── *.log               (per-CPU regression logs)
```

### Troubleshooting

If tests fail:
1. Check acceptance-output.txt for error details
2. Review regression logs for specific stage failures
3. Check audit-artifacts directory structure
4. Verify Cargo environment variables are set correctly

### Task 1.2 Status After Execution

Once Stage 10.7 acceptance completes:
- Document results in docs/ACCEPTANCE-RESULTS.md
- Update PHASE-1-STATUS.md with validation metrics
- Commit results to git
- Proceed to Task 1.3 (if all pass) or debug (if failures)

### Next Steps

1. Execute: `.\RUN-STAGE10.7.ps1`
2. Wait for completion (45 min - 1 hour)
3. Check audit-artifacts for results
4. Report back with acceptance status
5. Proceed with Phase 1 Task closure

---

**Task**: Phase 1 Task 1.2 - Acceptance Gate Testing
**Target**: All 12 improvements validated on 1/2/4 CPUs
**Success**: Stage 10.7 PASS on all CPU configurations
**Status**: Ready to execute
