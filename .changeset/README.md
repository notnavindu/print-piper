# Changesets

This folder is managed by [changesets](https://github.com/changesets/changesets).
It's how versioning and the changelog are automated.

## Adding a change

When you make a user-facing change, record it:

```sh
npx changeset
```

Pick the bump type — **patch** (fix), **minor** (feature), or **major** (breaking)
— and write a one-line summary. This creates a small markdown file in this folder;
commit it with your PR.

## How a release happens

1. PRs land on `main`, each carrying its changeset file(s).
2. The `version` workflow opens a **"version packages"** PR that consumes the
   changesets, bumps the version (in `package.json`, then synced into
   `tauri.conf.json` and `Cargo.toml`), and updates `CHANGELOG.md`.
3. Merging that PR pushes a release tag, which triggers the `release` workflow to
   build the macOS and Windows installers and attach them to a GitHub Release.

You never edit version numbers by hand.
