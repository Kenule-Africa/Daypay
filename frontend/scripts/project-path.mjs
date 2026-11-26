// On Windows, a shell can report the working directory with a lowercase drive
// letter (VS Code terminals do: "c:\..."). Next.js then registers app modules
// under two spellings of the same path, and rendering fails with "Could not find
// the module ... in the React Client Manifest". Starting from one canonical
// spelling avoids it. Other platforms are case-sensitive and left unchanged.
export function canonicalProjectDir(dir, platform = process.platform) {
  return platform === "win32" ? dir.replace(/^[a-z](?=:)/, drive => drive.toUpperCase()) : dir;
}
