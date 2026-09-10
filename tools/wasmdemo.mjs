// Run the browser-target module and print its checksums.
//
// This is the JS half of M6's browser gate. `wasm32-wasip1` proves the parser
// works when there is a platform underneath it; a browser is not that, so the
// claim has to be made on `wasm32-unknown-unknown` -- no filesystem, no clock,
// no environment, no WASI syscalls -- which is the target a page actually
// loads. Node is used as the JS host because it runs the same V8 a browser
// does and needs no display; nothing here depends on Node beyond reading the
// file, and the same four calls work unchanged from a `<script type=module>`.
//
//   node tools/wasmdemo.mjs [path-to.wasm]
//
// Output is one line per document plus an ALL line, in a format
// `tests/checksum_matches.rs` parses.

import { readFile } from "node:fs/promises";

const path =
  process.argv[2] ??
  "target/wasm32-unknown-unknown/release/rusty_json_turbo_wasmdemo.wasm";

const bytes = await readFile(path);
const mod = await WebAssembly.compile(bytes);

// A Rust cdylib for this target should import nothing at all. If it ever does,
// say exactly what -- an unexplained import is a platform dependency that
// crept into a build claiming not to have one, which is the whole thing this
// gate exists to catch.
const needed = WebAssembly.Module.imports(mod);
const importObject = {};
if (needed.length > 0) {
  console.error(
    `note: module imports ${needed.length} item(s); stubbing them out:`,
  );
  for (const { module, name, kind } of needed) {
    console.error(`  ${module}.${name} (${kind})`);
    importObject[module] ??= {};
    if (kind === "function") {
      importObject[module][name] = () => {
        throw new Error(`the module called host function ${module}.${name}`);
      };
    }
  }
}

const { exports } = await WebAssembly.instantiate(mod, importObject);

// i64 crosses the boundary as a BigInt, which is exact -- a Number could not
// hold a 64-bit checksum without losing the low bits, and a checksum that
// loses bits compares equal when it should not.
const hex = (v) => "0x" + BigInt.asUintN(64, v).toString(16).padStart(16, "0");

const n = exports.rjt_doc_count();
for (let i = 0; i < n; i++) {
  const len = exports.rjt_output_len(i);
  const sum = exports.rjt_checksum(i);
  console.log(`doc ${i} len=${len} checksum=${hex(sum)}`);
}
console.log(`ALL checksum=${hex(exports.rjt_checksum_all())}`);
