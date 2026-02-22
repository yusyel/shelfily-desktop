# Flathub Submission Checklist

This repo now contains the files needed to open a Flathub packaging PR.

## Files to submit

- `flatpak/io.github.yusyel.ShelfilyDesktop.json`
- `flatpak/cargo-sources.json`

## Pre-submit checks

1. Update Rust vendored source list after any dependency change:
   - `./flatpak/update-cargo-sources.sh`
2. Ensure the manifest source commit points to the release commit in upstream:
   - `flatpak/io.github.yusyel.ShelfilyDesktop.json` -> `modules[0].sources[0].commit`
3. Ensure metainfo release data is current:
   - `data/io.github.yusyel.ShelfilyDesktop.metainfo.xml.in`
4. Validate JSON syntax:
   - `python3 -m json.tool flatpak/io.github.yusyel.ShelfilyDesktop.json >/dev/null`
   - `python3 -m json.tool flatpak/cargo-sources.json >/dev/null`

## Local build prerequisites

Install runtime/sdk used by this manifest:

```bash
flatpak install -y flathub org.gnome.Platform//48 org.gnome.Sdk//48 org.freedesktop.Sdk.Extension.rust-stable//24.08
```

Install builder:

```bash
flatpak install -y flathub org.flatpak.Builder
```

Build test:

```bash
flatpak run --filesystem=$(pwd) --filesystem=/tmp org.flatpak.Builder \
  --force-clean .flatpak-build flatpak/io.github.yusyel.ShelfilyDesktop.json
```

## Flathub PR flow

1. Fork `https://github.com/flathub/flathub`.
2. Add app directory `io.github.yusyel.ShelfilyDesktop/` in your fork.
3. Copy the two files above into that directory.
4. Open PR to `flathub/flathub`.
5. Address bot/reviewer feedback until merged.

## Automated PR flow from this repository

Workflow: `.github/workflows/flathub-publish.yml`

- Trigger: push tags like `v0.1.0` or manual dispatch.
- Requires:
  - Secret `FLATHUB_GITHUB_TOKEN`
  - Variable `FLATHUB_FORK_OWNER`
  - Optional variable `FLATHUB_BASE_BRANCH` (default `master`)
