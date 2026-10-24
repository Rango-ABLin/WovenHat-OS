"""Fail-closed serial/exit validation shared by foundation QEMU gates."""
import re


def validation_errors(returncode, log, required):
    """A terminal success requires every marker and no contradictory failure.

    Intentional user faults remain allowed: isolation gates deliberately fault
    unprivileged programs. Kernel panics and tagged FAILED/TIMEOUT outcomes do
    not become successful just because another test prints a success marker.
    """
    lines = log.splitlines()
    errors = []
    for marker in required:
        if not any(line == marker or
                   (line.startswith(marker) and line[len(marker):len(marker) + 1].isspace())
                   for line in lines):
            errors.append(f'Missing marker: {marker}')
    if returncode != 33:
        errors.append(f'QEMU exit {returncode}; expected 33')
    if re.search(r'KERNEL\s+PANIC|panicked at', log, re.IGNORECASE):
        errors.append('Serial log contains a kernel panic')
    if re.search(r'^\s*\[[^\]\r\n]+\][^\r\n]*\b(?:FAILED|TIMEOUT)\b', log, re.MULTILINE):
        errors.append('Serial log contains a failed or timed-out gate')
    return errors
