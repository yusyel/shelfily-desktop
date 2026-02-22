# shelfily-desktop

Desktop client for Shelfily.

## Icon

![Shelfily Desktop Icon](./icon.png)

## Screenshot

![Shelfily Desktop Screenshot](./sc.png)

## Features

- Native GTK4/libadwaita desktop experience for Linux
- Connect and sign in to your Audiobookshelf server
- Browse libraries and view audiobook/podcast details
- Stream and control playback from the desktop app
- Flatpak packaging and automated CI/CD workflows

## Flatpak CI/CD

This repository includes two GitHub Actions workflows:

1. `.github/workflows/flatpak-ci.yml`
- Runs on every push to `main` and on pull requests.
- Builds a Flatpak bundle from `flatpak/io.github.yusyel.ShelfilyDesktop.Devel.json`.
- Uploads the generated `.flatpak` file as a workflow artifact.

2. `.github/workflows/flatpak-release.yml`
- Runs when you push a tag like `v0.1.0`.
- Builds a release `.flatpak` bundle.
- Uploads the bundle as an asset to the matching GitHub Release.

3. `.github/workflows/flathub-publish.yml`
- Runs on tags like `v0.1.0` (and manually via workflow dispatch).
- Syncs `flatpak/io.github.yusyel.ShelfilyDesktop.json` and `flatpak/cargo-sources.json`
  into your Flathub fork branch.
- Opens (or reuses) a PR to `flathub/flathub`.

## Flathub publishing flow

GitHub CI/CD builds and releases Flatpak artifacts, but Flathub publishing is done through the Flathub packaging repository and PR review process.

1. Tag and push a release:
```bash
git tag v0.1.0
git push origin v0.1.0
```
2. Wait for `Flatpak Release` workflow to produce the `.flatpak` release asset.
3. Submit/update your app manifest in Flathub (new app request or update PR).
4. After Flathub review/merge, the app is published on Flathub.

### Required repository settings for Flathub automation

Configure these in this GitHub repository:

- Secret: `FLATHUB_GITHUB_TOKEN`
  - GitHub token that can push to your fork of `flathub/flathub` and create PRs.
- Variable: `FLATHUB_FORK_OWNER`
  - Your GitHub username/org that owns the fork.
- Optional variable: `FLATHUB_BASE_BRANCH`
  - Defaults to `new-pr`.

## Flathub manifest prep in this repo

The Flathub-ready manifest and vendored Rust dependency source list live under:

- `flatpak/io.github.yusyel.ShelfilyDesktop.json`
- `flatpak/cargo-sources.json`

Regenerate `flatpak/cargo-sources.json` whenever `Cargo.lock` changes:

```bash
./flatpak/update-cargo-sources.sh
```
