import type { ReviewPack } from '@debuggatha/core';

export const testingPack: ReviewPack = {
  id: 'debuggatha/testing',
  version: '1.0.0',
  kind: 'concern',
  displayName: 'Test Suite Quality',
  dependsOn: [],
  rules: [
    {
      id: 'no-focused-tests',
      packId: 'debuggatha/testing',
      statement:
        'Do not commit focused tests (such as `it.only`, `test.only`, `describe.only`, `fit`, or `fdescribe`). Focused tests silence the rest of the test suite and can hide regressions.',
      category: 'reliability',
      appliesTo: { kind: 'always' },
      defaultSeverity: 'critical',
      knowledgeRefs: ['know-testing-no-focused-tests'],
      contradicts: undefined,
    },
    {
      id: 'no-empty-tests',
      packId: 'debuggatha/testing',
      statement:
        'Avoid empty test blocks or tests containing only comments. Empty tests give a false sense of test coverage without actually verifying any behavior.',
      category: 'maintainability',
      appliesTo: { kind: 'always' },
      defaultSeverity: 'high',
      knowledgeRefs: ['know-testing-no-empty-tests'],
      contradicts: undefined,
    },
    {
      id: 'no-assertionless-tests',
      packId: 'debuggatha/testing',
      statement:
        'Every test should contain at least one verifiable assertion (e.g. `expect`, `assert`, or verified thrown exception) to confirm the expected behavior.',
      category: 'reliability',
      appliesTo: { kind: 'always' },
      defaultSeverity: 'medium',
      knowledgeRefs: ['know-testing-no-assertionless-tests'],
      contradicts: undefined,
    },
    {
      id: 'unhandled-async-test',
      packId: 'debuggatha/testing',
      statement:
        'Ensure asynchronous operations inside tests are awaited or returned as Promises. Floating async operations can cause tests to complete prematurely or fail silently.',
      category: 'reliability',
      appliesTo: { kind: 'always' },
      defaultSeverity: 'high',
      knowledgeRefs: ['know-testing-unhandled-async-test'],
      contradicts: undefined,
    },
    {
      id: 'leak-mock-restore',
      packId: 'debuggatha/testing',
      statement:
        'Restore or clear spies and mocked globals (such as `vi.spyOn`, `jest.spyOn`, `sinon.spy`, or environment overrides) in `afterEach` blocks or using `mockRestore` to prevent test state leakage.',
      category: 'reliability',
      appliesTo: { kind: 'always' },
      defaultSeverity: 'medium',
      knowledgeRefs: ['know-testing-leak-mock-restore'],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: 'know-testing-no-focused-tests',
      title: 'Focused Tests in Version Control',
      body: 'Focused tests (it.only, test.only, describe.only) are useful during local test-driven development to run a single test, but committing them skips all other tests in CI, allowing serious regressions to pass unnoticed.',
      externalRefs: [
        'https://vitest.dev/api/#test-only',
        'https://jestjs.io/docs/api#testonlyname-fn-timeout',
      ],
      limitations: [
        'Test runners that execute quarantined or debug-only suites via explicit environment flags or separate entrypoint configurations may intentionally isolate tests without affecting mainline CI.',
      ],
    },
    {
      id: 'know-testing-no-empty-tests',
      title: 'Empty Test Blocks',
      body: 'Tests without an implementation body pass trivially. They artificially inflate test count and reported coverage metrics while exercising no code pathways.',
      externalRefs: ['https://martinfowler.com/bliki/TestPyramid.html'],
      limitations: [
        'Placeholder tests explicitly marked with todo methods (such as it.todo or test.todo) convey intent for future implementation and are exempt when supported by the runner.',
      ],
    },
    {
      id: 'know-testing-no-assertionless-tests',
      title: 'Assertionless Tests',
      body: 'A test that executes code without checking return values, side effects, or assertions only verifies that the code does not throw an unhandled exception. While useful as a basic smoke test, explicit assertions are needed to verify correctness.',
      externalRefs: ['https://kentcdodds.com/blog/common-testing-mistakes'],
      limitations: [
        'Pure smoke tests that deliberately assert only that an invocation completes without crashing (such as rendering a component tree without throwing) may intentionally omit explicit assertions.',
      ],
    },
    {
      id: 'know-testing-unhandled-async-test',
      title: 'Floating Promises in Async Tests',
      body: 'When a test launches an asynchronous promise without awaiting it or returning it to the test runner, the test framework marks the test as passed immediately. Any subsequent assertion failures or unhandled rejections happen after the test finishes, leading to unhandled promise rejections or flaky test runs.',
      externalRefs: ['https://nodejs.org/api/errors.html#unhandled-promise-rejections'],
      limitations: [
        'Fire-and-forget background workers or event listeners whose completion is verified indirectly via polling or countdown latches may not directly await every intermediate Promise.',
      ],
    },
    {
      id: 'know-testing-leak-mock-restore',
      title: 'Mock and Spy Isolation',
      body: 'Modifying global objects, prototypes, process.env, or standard library APIs inside tests without restoring them pollutes subsequent tests running in the same process, causing hard-to-debug order-dependent test failures.',
      externalRefs: ['https://vitest.dev/api/mock.html#mockrestore'],
      limitations: [
        'Projects with isolated worker threads per test file or test frameworks configured with automatic mock restoration (e.g. restoreMocks: true) do not require manual per-test restoration.',
      ],
    },
  ],
};
