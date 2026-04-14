import fs from "node:fs/promises";
import path from "node:path";

const sourceDir = new URL("../node_modules/@chenglou/pretext/dist/", import.meta.url);
const pkgDir = new URL("../pkg/", import.meta.url);
const vendorDir = new URL("../pkg/vendor/", import.meta.url);
const pkgPackage = new URL("../pkg/package.json", import.meta.url);
const vendorPackage = new URL("../pkg/vendor/package.json", import.meta.url);
const pretextTarget = new URL("../pkg/vendor/pretext.js", import.meta.url);

await fs.rm(vendorDir, { recursive: true, force: true });
await fs.mkdir(pkgDir, { recursive: true });
await fs.mkdir(vendorDir, { recursive: true });
await fs.writeFile(pkgPackage, JSON.stringify({ type: "module" }, null, 2) + "\n");
await fs.writeFile(vendorPackage, JSON.stringify({ type: "module" }, null, 2) + "\n");

for (const entry of await fs.readdir(sourceDir, { withFileTypes: true })) {
  if (!entry.isFile() || !entry.name.endsWith(".js")) {
    continue;
  }
  const source = new URL(entry.name, sourceDir);
  const destination = new URL(entry.name, vendorDir);
  await fs.copyFile(source, destination);
  if (entry.name === "layout.js") {
    await fs.copyFile(source, pretextTarget);
  }
}

console.log(`copied ${path.basename(new URL("layout.js", sourceDir).pathname)} -> ${path.basename(pretextTarget.pathname)}`);
