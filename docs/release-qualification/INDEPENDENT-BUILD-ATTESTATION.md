# Independent organization build-attestation protocol

This gate cannot be satisfied by WovenHat repository agents, the project
owner, an alternate local checkout, another local agent, or GitHub Actions.
It requires an organization that is legally and operationally independent of
the WovenHat maintainer and that controls its own build host and Ed25519 key.

## Inputs the independent organization must deliver

1. The exact release source commit and source archive from
   `git archive --format=tar <commit>`.
2. A clean-room rebuild of the accepted locked/offline command recorded in the
   local `wovenhat.clean-room-build/v1` report.
3. The resulting source/archive/artifact SHA-256 values.
4. The compiler and operating-system identity.
5. A public Ed25519 key owned and controlled by the independent organization.
6. A detached Ed25519 signature over the attestation JSON.
7. A legally attributable organization identity and operator contact.

The independent organization can create the unsigned template from its own
clean-room report:

```powershell
python .\scripts\create-independent-build-attestation.py `
  --build-report target\clean-room-build.json `
  --output independent-attestation.json `
  --organization "Independent Build Laboratory" `
  --organization-legal-name "Independent Build Laboratory Ltd." `
  --organization-contact "release@example.test" `
  --attestor-name "Authorized named operator" `
  --build-host "organization-controlled-host-id" `
  --build-location "documented independent environment" `
  --artifact "release-artifact-name=<sha256>"
```

The attestation JSON schema is
`wovenhat.independent-build-attestation/v1` and must contain:

```json
{
  "schema": "wovenhat.independent-build-attestation/v1",
  "organization": "Example Independent Build Laboratory",
  "organization_legal_name": "Example Independent Build Laboratory Ltd.",
  "organization_contact": "release-attestation@example.test",
  "attestor_name": "Named authorized operator",
  "build_host": "independent-host-id",
  "build_location": "organization-owned host or documented isolated cloud tenant",
  "attested_at_utc": "YYYY-MM-DDTHH:MM:SSZ",
  "independent": true,
  "source_obtained_from_wovenhat_checkout": false,
  "repository_accessed_during_build": false,
  "source_commit": "<40+ hexadecimal characters>",
  "archive_sha256": "<64 hexadecimal characters>",
  "command": ["cargo", "test", "-p", "libwoven", "--lib", "--locked", "--offline"],
  "offline_locked": true,
  "returncode": 0,
  "rustc": "<exact rustc -Vv output>",
  "artifacts": [
    {"path": "result-description", "sha256": "<64 hexadecimal characters>"}
  ]
}
```

The detached signature uses the existing
`wovenhat.sdk-provenance-signature/v1` envelope and must bind the exact
attestation bytes.

## Verification

Supply the external attestor's pinned public key through review, not from the
attestation itself:

```powershell
$env:WOVENHAT_INDEPENDENT_ATTESTOR_PUBLIC_KEY_HEX = "<externally verified 64-hex key>"
python .\scripts\verify-independent-build-attestation.py `
  --attestation independent-attestation.json `
  --signature independent-attestation.signature.json `
  --local-report target\clean-room-build.json `
  --output target\independent-build-verification.json
```

The verifier fails closed on:

- a WovenHat/owner/agent identity;
- a declaration that is not independent;
- access to the WovenHat checkout during the build;
- a different commit, archive hash, compiler or command;
- a failed or online/unlocked build;
- an attestation key not pinned independently by the release operator;
- payload-hash, trusted-key or Ed25519 signature mismatch;
- absent legal identity/contact or empty artifact evidence.

## Release promotion

The release remains blocked until the verifier returns `valid: true` and the
independent organization attests, outside this repository, that it owns the
signing key and build host. Preserve the attestation, signature, verifier
result, public key provenance and the independent review disposition with the
release evidence. Development keys, repository agents, CI reruns and alternate
local directories cannot satisfy this gate.
