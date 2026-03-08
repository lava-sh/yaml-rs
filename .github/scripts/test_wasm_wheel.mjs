import { readFile } from "node:fs/promises";
import { basename } from "node:path";

const { WHEEL_PATH, PYODIDE_VERSION } = process.env;

if (!WHEEL_PATH) {
  throw new Error("WHEEL_PATH is required");
}

if (!PYODIDE_VERSION) {
  throw new Error("PYODIDE_VERSION is required");
}

try {
  const { loadPyodide } = await import("pyodide");
  const pyodide = await loadPyodide();
  await pyodide.loadPackage("micropip");

  const wheelBytes = new Uint8Array(await readFile(WHEEL_PATH));
  const wheelName = basename(WHEEL_PATH);
  const wheelInFs = `/tmp/${wheelName}`;
  pyodide.FS.writeFile(wheelInFs, wheelBytes);

  const installed = await pyodide.runPythonAsync(`
import micropip

await micropip.install("emfs:${wheelInFs}")

import yaml_rs
yaml_rs.__version__
`);

  if (!installed) {
    throw new Error(`yaml_rs.__version__ is empty for pyodide ${PYODIDE_VERSION}`);
  }

  console.log(`ok pyodide=${PYODIDE_VERSION} yaml_rs=${installed}`);
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`fail pyodide=${PYODIDE_VERSION} message=${message}`);
  process.exit(1);
}
