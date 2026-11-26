// Lists the commands whose result is an array (read from the desktop's ipc/commands.ts), so the
// mocked backend can answer [] to any of them that fixtures.ts leaves out.
import { readFileSync, writeFileSync } from "node:fs";
const [commands, out] = process.argv.slice(2);
const src = readFileSync(commands, "utf8");
const names = [...src.matchAll(/invoke<([^(]*?)>\(\s*"([a-z0-9_:|-]+)"/g)]
  .filter((m) => /\[\]\s*$/.test(m[1].trim()))
  .map((m) => m[2]);
writeFileSync(out, JSON.stringify([...new Set(names)].sort(), null, 1) + "\n");
