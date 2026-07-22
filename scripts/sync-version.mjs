// Propagate the package.json version (the single source of truth, bumped by
// changesets) into the Rust/Tauri side so all four version sites agree.
// tauri.conf.json already reads its version from package.json, so only Cargo.toml
// and Cargo.lock need syncing. Pure string edits — no cargo required, so this runs
// fine on the CI versioning runner.
import { readFileSync, writeFileSync } from "node:fs";

const { version } = JSON.parse(readFileSync("package.json", "utf8"));
if (!version) {
  console.error("no version field in package.json");
  process.exit(1);
}

// Cargo.toml: the [package] version is the only line starting with `version = "`
// at column 0 (dependency versions are inline `{ version = ... }`).
const tomlPath = "src-tauri/Cargo.toml";
const toml = readFileSync(tomlPath, "utf8");
const nextToml = toml.replace(/^version = "[^"]*"/m, `version = "${version}"`);
if (nextToml === toml && !toml.includes(`version = "${version}"`)) {
  console.error("failed to update version in Cargo.toml");
  process.exit(1);
}
writeFileSync(tomlPath, nextToml);

// Cargo.lock: the print-piper package entry.
const lockPath = "src-tauri/Cargo.lock";
const lock = readFileSync(lockPath, "utf8");
const nextLock = lock.replace(
  /(name = "print-piper"\nversion = )"[^"]*"/,
  `$1"${version}"`
);
if (nextLock === lock && !lock.includes(`name = "print-piper"\nversion = "${version}"`)) {
  console.error("failed to update print-piper version in Cargo.lock");
  process.exit(1);
}
writeFileSync(lockPath, nextLock);

console.log(`synced version ${version} → Cargo.toml, Cargo.lock`);
