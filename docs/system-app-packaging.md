# WovenHat system-app packaging

`scripts/package-system-apps.py` is the reproducible release boundary for the
accepted Stage 19 applications. It builds each app as a freestanding
`x86_64-unknown-none` ELF, signs it with the existing WAPP packager, validates
the envelope, and emits:

- `packages/<id>.wapp`, the signed install payload;
- `catalog/<id>.app`, the bounded WovenStart `Name=`/`Exec=` descriptor; and
- `manifest.json`, containing install paths, byte counts, and SHA-256 hashes.

Run it from the repository root:

```text
python scripts/package-system-apps.py --version 0.8.0
```

The default output is `target/system-apps/0.8.0`. Build directories and output
are ignored artifacts; the manifest is the release evidence that binds the
source applications to their signed package and catalog bytes.

This does not claim multi-application boot installation. The current kernel
acceptance ABI embeds one `WOVENHAT_APP_IMAGE` package per Stage 16.1 boot and
installs it at a test path. A later installer/image integration must consume
this manifest, install each package at its declared path, verify each WAPP
signature before publication, and atomically publish the catalog. Until that
consumer exists, the system-app packaging boundary is production-tested but
the desktop distribution path remains open.
