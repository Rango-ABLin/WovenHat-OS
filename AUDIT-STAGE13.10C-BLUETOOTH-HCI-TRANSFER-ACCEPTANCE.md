# Stage 13.10C — Bluetooth HCI Transfer Execution Contract Acceptance

Accepted: 2026-09-30

## Scope accepted

Stage 13.10C establishes a bounded transaction/execution contract between the hardware-independent HCI state machine and the Stage 13.10B USB transport description.

Accepted behavior:
- non-copyable HCI transaction state;
- one outstanding HCI command governed by controller command credits;
- bounded command encoding;
- matching Command Complete opcode/status validation;
- Reset and Read Local Version transaction sequencing in deterministic tests;
- bounded xHCI Bluetooth command, event, ACL-IN and ACL-OUT transfer plans;
- ACL-OUT rejection above the discovered endpoint packet capacity;
- runtime marker `[S13.10C] Bluetooth HCI transfer execution contract: PASSED`.

## Authoritative validation

GitHub Actions run: 36729724615
Accepted source SHA: `5aa5628ad815ef2f04e0e1b3a150dad0bc30b355`
Release-validation job: 109935801913
Result: SUCCESS
Artifact digest: `sha256:b9bb2fa1d58274d268f069b44440844adbb87e51f3a04e0547d8868b5080dcf0`

The run passed the generic lint/host/1-2-4 CPU matrix, Stage 13.3 through Stage 13.9 regression gates, and the dedicated Stage 13.10C Bluetooth HCI transfer 1/2/4-core acceptance gate.

## Boundary

This acceptance is deterministic software-contract validation. It does not claim live physical USB Bluetooth operation. Dedicated Bluetooth endpoint contexts/rings, DMA-backed control-OUT command transfer, interrupt-IN event reception, bulk ACL execution, hardware Reset/Read Local Version, teardown/recovery, and physical qualification remain open.

## Next

Bind the accepted transfer plans to dedicated xHCI rings and DMA ownership, route transfer completions into the HCI transaction state, and execute the initial controller Reset / Read Local Version sequence when a real Bluetooth USB interface is present.
