# shelfily-desktop

Desktop client for Shelfily.

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
