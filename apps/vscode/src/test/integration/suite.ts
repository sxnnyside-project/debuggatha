import { resolve } from "node:path";
import Mocha from "mocha";

/** Entry point VS Code calls inside the extension host (`extensionTestsPath`). */
export async function run(): Promise<void> {
  const mocha = new Mocha({ ui: "tdd", color: true, timeout: 90_000 });
  mocha.addFile(resolve(__dirname, "extension.itest.js"));
  // Run only tests whose name matches, while working on one: `DEBUGGATHA_TEST_GREP="quick fix"`.
  if (process.env.DEBUGGATHA_TEST_GREP) mocha.grep(process.env.DEBUGGATHA_TEST_GREP);

  await new Promise<void>((done, fail) => {
    mocha.run((failures) =>
      failures > 0 ? fail(new Error(`${failures} integration tests failed.`)) : done(),
    );
  });
}
