# Test Harness Consolidation & Auto-Feature Gating - Implementation Plan

## Current State Analysis

**Problems**:
- 27 hand-written PowerShell stage acceptance scripts
- 75+ Cargo features with manual dependency tracking
- No build caching per CPU count
- Sequential execution only
- Feature gate rules scattered (AGENTS.md comments)
- No automated dependency validation

**Metrics**:
- Current full chain: ~2-3 hours (27 stages × ~5-10 min each)
- Opportunity: Parallel + caching → 30-45 minutes

## Solution Overview

### 1. Feature Metadata Extraction (`features.toml`)
Extract from Cargo.toml:
- Feature name
- Dependencies (prerequisites)
- Test marker (serial output to expect)
- CPU matrix (1, 2, 4, or combo)
- Timeout (default 300s)

### 2. Python Test Orchestrator (`test-orchestrator.py`)
- Parse features.toml
- Build dependency graph
- Parallel-run independent stages
- Cache builds per feature/CPU
- Aggregate results
- Report timing + failures

### 3. Build Caching
- Cache key: feature + CPU + source hash
- Store: target/cache/{feature}-{cpu}-{hash}.tar.zst
- Reuse if no source changes

### 4. Result Aggregation
- JSON output: pass/fail, timing, serial logs
- HTML report (optional): timeline, critical path
- Exit code: 0 if all pass, >0 otherwise

## File Structure

```
C:\Users\antho\WovenHat-OS\
├── features.toml                    # Feature metadata (NEW)
├── scripts/
│   ├── test-orchestrator.py         # Main orchestrator (NEW)
│   └── build-cache-manager.py       # Cache helpers (NEW)
├── run-stage*.ps1                   # Keep for backward compat
└── docs/
    └── TEST-ORCHESTRATION.md        # Guide (NEW)
```

## Implementation Steps

1. Create features.toml from Cargo.toml
2. Implement test-orchestrator.py
3. Add build caching
4. Test on single stage
5. Expand to full chain
6. Add parallel execution
7. Document + integrate

