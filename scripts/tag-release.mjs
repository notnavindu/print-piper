// Create and push the release tag for the current package.json version — but only
// if it doesn't already exist. Runs as the changesets "publish" step.
//
// Why this exists: `changeset tag` skips packages marked `private` (ours is, since
// it's an app, not an npm package), so it never creates a tag and the release
// workflow never fires. We tag it ourselves instead. The push uses the checkout's
// RELEASE_PAT credentials, which is what lets the pushed tag trigger release.yml
// (a tag pushed with the default GITHUB_TOKEN would not).
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

const { version } = JSON.parse(readFileSync("package.json", "utf8"));
const tag = `print-piper@${version}`;

const existing = execFileSync("git", [
  "ls-remote",
  "--tags",
  "origin",
  `refs/tags/${tag}`,
])
  .toString()
  .trim();

if (existing) {
  console.log(`${tag} already exists on origin — nothing to release`);
  process.exit(0);
}

execFileSync("git", ["tag", tag]);
execFileSync("git", ["push", "origin", `refs/tags/${tag}`], { stdio: "inherit" });
console.log(`created and pushed ${tag}`);
