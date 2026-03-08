import { readFile } from "node:fs/promises";
import { loadPyodide } from "pyodide";

const { WHEEL_PATH, PYODIDE_VERSION } = process.env;

if (!WHEEL_PATH) {
  throw new Error("WHEEL_PATH is required");
}

if (!PYODIDE_VERSION) {
  throw new Error("PYODIDE_VERSION is required");
}

const pyodide = await loadPyodide();
await pyodide.loadPackage("micropip");

const wheelBytes = new Uint8Array(await readFile(WHEEL_PATH));
pyodide.FS.writeFile("/tmp/yaml_rs.whl", wheelBytes);

const installed = await pyodide.runPythonAsync(`
import micropip

await micropip.install("emfs:/tmp/yaml_rs.whl")

import yaml_rs
yaml_rs.__version__
`);

if (!installed) {
  throw new Error(`yaml_rs.__version__ is empty for pyodide ${PYODIDE_VERSION}`);
}

console.log(`ok pyodide=${PYODIDE_VERSION} yaml_rs=${installed}`);
