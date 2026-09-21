/**
 * Measures a model on the labeled set: `just semantic-eval` (Ollama), or
 * `bun packages/engine/src/semantic/eval/run.ts --provider lmstudio --model <name>`.
 * Prints per-category precision and recall, and each verdict a person may want to read.
 */
import { createLocalProvider, LOCAL_PROVIDERS, type LocalProviderName } from "../providers.js";
import { evaluateDetection, evaluateVerification, formatReport } from "./evaluate.js";

const args = process.argv.slice(2);
const option = (name: string) => {
  const index = args.indexOf(`--${name}`);
  return index >= 0 ? args[index + 1] : undefined;
};

const name = (option("provider") ?? "ollama") as LocalProviderName;
if (!LOCAL_PROVIDERS.includes(name)) {
  console.error(`Unknown provider "${name}". Use one of: ${LOCAL_PROVIDERS.join(", ")}.`);
  process.exit(1);
}

const model = option("model");
const baseUrl = option("url");
const provider = await createLocalProvider(name, {
  ...(model ? { model } : {}),
  ...(baseUrl ? { baseUrl } : {}),
});
console.error(`Evaluating ${provider.name} / ${provider.model} ...`);

const verification = await evaluateVerification(provider);
const detection = await evaluateDetection(provider);

console.log(formatReport(`${provider.name} / ${provider.model}`, verification, detection));
if (args.includes("--verbose")) {
  console.log("\nVerdicts:");
  for (const o of verification) {
    const right = (o.truth === "real") === (o.verdict === "confirmed") ? "ok " : "-- ";
    console.log(
      `  ${right}${o.id.padEnd(34)} truth=${o.truth.padEnd(14)} verdict=${o.verdict.padEnd(9)} ${o.reason}`,
    );
  }
}
