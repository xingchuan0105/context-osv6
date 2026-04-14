#!/usr/bin/env node

const [major] = process.versions.node.split(".").map((v) => Number(v));

if (Number.isNaN(major) || major < 20 || major >= 25) {
  console.error(
    `Unsupported Node.js ${process.versions.node}. ` +
      "Use Node.js 20.x, 22.x, or 24.x for this frontend."
  );
  process.exit(1);
}
