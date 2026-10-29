// Random runs in its own worker with its own wasm instance. A sample that makes the sampler
// fail traps that instance only; the page then starts a new worker.
import init, { random } from "./pkg/pgtk_playground_wasm.js";

const ready = init();

self.onmessage = async () => {
  await ready;
  const attempts = 20;
  const entropy = new Uint8Array(256 * attempts);
  crypto.getRandomValues(entropy);
  self.postMessage(random(entropy, attempts) ?? null);
};
