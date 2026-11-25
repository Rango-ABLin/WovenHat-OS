# Release signing key custody

## Decision

WovenHat production releases use three separate Ed25519 signing authorities:

| Purpose | Trust consumer | Environment prefix |
| --- | --- | --- |
| UEFI boot/kernel handoff | `kernel/src/secure_boot.rs` | `WOVENHAT_BOOT_` |
| WAPP/WPKG/WUPD applications and updates | `kernel/src/signed_app.rs` | `WOVENHAT_APP_` |
| SDK SBOM/provenance attestation | External verifier | `WOVENHAT_SDK_PRODUCTION_` |

Production private keys must be generated on a PKCS#11 token and must not be
exported. The repository may contain only public keys, signatures and custody
evidence. A software signing seed remains available for deterministic tests
and explicit one-time transitions, but release packaging rejects ambiguous
authority: a seed and an external signer cannot be set together.

## Required token properties

The selected token or HSM must support:

- Ed25519 PKCS#11 signing (`CKM_EDDSA`).
- On-token key generation and non-exportable private objects.
- PIN-controlled signing and a unique key ID for each purpose.
- A vendor-supported encrypted backup, replication or second protected token
  that does not export raw private material.
- Audit records for key generation, signer authorization and release signatures.

The release gate does not accept a token that can export any private key.
Document the token model, serial/label, firmware and approved backup process in
the signed custody evidence.

## One-time key ceremony

Run this ceremony with the device owner and a second witness. Use a
release-dedicated token, not a personal login token.

1. Configure the token's user PIN interactively. Do not put the PIN in a file,
   command line, environment variable, shell history or repository.
2. Verify `pkcs11-tool` is present and identify the PKCS#11 module and slot:

   ```powershell
   $env:WOVENHAT_PKCS11_MODULE = "C:\Program Files\OpenSC Project\OpenSC\pkcs11\opensc-pkcs11.dll"
   $env:WOVENHAT_PKCS11_SLOT = "<token-slot>"
   ```

3. Assign distinct key IDs:

   ```powershell
   $env:WOVENHAT_BOOT_PKCS11_KEY_ID = "01"
   $env:WOVENHAT_APP_PKCS11_KEY_ID = "02"
   $env:WOVENHAT_SDK_PKCS11_KEY_ID = "03"
   ```

4. Generate the three non-exportable keys and public-only evidence:

   ```powershell
   python .\scripts\provision-release-keys.py `
     --output-directory .\audit-artifacts\release-key-custody-YYYYMMDD `
     --token-label "<controlled hardware token label>"
   ```

   The command refuses to overwrite existing token objects or a non-empty
   evidence directory. It never writes private bytes.

5. Verify with the token vendor's tooling that each private object is
   non-extractable/non-exportable and that signing requires the expected
   authorization. Archive the result with the public-only custody manifest.
6. Verify the three `public_key_hex` values against an independent token read.
   Have both ceremony participants sign the evidence.
7. Execute the token vendor's approved redundant-custody procedure. If the
   token cannot be backed up or replicated without raw-key export, provide a
   documented second-token/key-release transition instead and record the
   remaining recovery risk.

## Release configuration

Set only the three trusted public keys and external signer paths. The token PIN
is entered by `pkcs11-tool` at each protected operation.

```powershell
$env:WOVENHAT_BOOT_TRUSTED_PUBLIC_KEY_HEX = "<boot public key>"
$env:WOVENHAT_BOOT_EXTERNAL_SIGNER = (Resolve-Path .\scripts\hardware-release-signer.py)

$env:WOVENHAT_APP_TRUSTED_PUBLIC_KEY_HEX = "<application public key>"
$env:WOVENHAT_APP_EXTERNAL_SIGNER = (Resolve-Path .\scripts\hardware-release-signer.py)

$env:WOVENHAT_SDK_PRODUCTION_TRUSTED_PUBLIC_KEY_HEX = "<SDK public key>"
$env:WOVENHAT_SDK_PRODUCTION_EXTERNAL_SIGNER = (Resolve-Path .\scripts\hardware-release-signer.py)
```

On a host where `.py` files are not directly executable, set
`WOVENHAT_SIGNER_PYTHON` to the controlled Python executable. The common signing
library invokes the external signer without shell interpolation, validates a
64-byte raw signature and verifies it against the pinned public key before an
artifact is accepted.

Do not set `WOVENHAT_*_SIGNING_SEED_HEX` in a hardware-custody production
environment.

## Verification

Before signing a release:

```powershell
python .\scripts\test-release-key-provisioning.py
python .\scripts\sign-sdk-provenance.py sign `
  <provenance.json> <provenance.signature.json> --production
python .\scripts\sign-sdk-provenance.py verify `
  <provenance.signature.json> <provenance.json> --production
```

Then run the accepted release build/package chain with the external signer.
Production signing is complete only when the boot image, all WAPP/WPKG/WUPD
artifacts and SDK provenance signature verify with the three pinned public keys.

## Rotation and compromise

The current boot and application verifiers each pin one public key. Rotation is
therefore a coordinated OS release transition, not an in-place runtime secret
swap. Preserve the previous key only while required to verify already shipped
artifacts; do not continue signing with it. A compromised key requires an
independently reviewed emergency transition with a newly built trust root, new
artifacts and an explicit support statement for old images.

## Current gate state

The repository-side custody boundary and hardware-signer adapter are complete.
The external release gate remains blocked until the physical token is present,
the ceremony above is witnessed, and the resulting public-only evidence plus
signatures are retained against an exact release commit.
