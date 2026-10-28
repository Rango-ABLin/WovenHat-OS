# WovenHat system-app packaging

`scripts/package-system-apps.py` is the reproducible release boundary for the
accepted Stage 19 applications. It builds each app as a freestanding
`x86_64-unknown-none` ELF, signs it with the existing WAPP packager, validates
the envelope, and emits:

- `packages/<id>.wapp`, the signed install payload;
- `catalog/<id>.app`, the bounded WovenStart `Name=`/`Exec=` descriptor; and
- `manifest.json`, containing install paths, byte counts, and SHA-256 hashes.

The Stage 28 `woven-installer` entry is published at the stable
`/bin/wovenhat-installer` path with `capabilities: []`, `privileged: false`,
and `interface: display-only`. It is presentation-only and receives no
installation authority.

Run it from the repository root:

```text
python scripts/package-system-apps.py --version 0.8.0
```

The default output is `target/system-apps/0.8.0`. Build directories and output
are ignored artifacts; the manifest is the release evidence that binds the
source applications to their signed package and catalog bytes.

The current release image consumes this manifest through the feature-gated
`system-apps` boot path, which installs accepted packages in RAM after
signature and loader validation and rolls back entries created by a failed
transaction. Persistent-media installation, power-loss recovery, key
rotation/revocation, and update policy remain separate requirements; the Stage
16.1 acceptance still exercises one package at a time through
`WOVENHAT_APP_IMAGE`.
