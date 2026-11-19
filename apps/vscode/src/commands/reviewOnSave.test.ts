import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterAll, afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { loadLedger, makeRepo, removeRepos } from '../test/fixtures.js';
import { harness } from '../test/harness.js';
import { mock, resetMock, Uri } from '../test/vscode-mock.js';
import { registerReviewOnSave, SAVE_DEBOUNCE_MS } from './reviewOnSave.js';

afterAll(removeRepos);

async function setup() {
  const { root, badFile } = makeRepo();
  const { services } = harness(root);
  registerReviewOnSave({ subscriptions: [] } as never, services);
  const save = (path: string, scheme = 'file') => {
    for (const listener of mock.saveListeners) listener({ uri: new Uri(scheme, path) });
  };
  return { root, badFile, services, save };
}

const settle = () => vi.advanceTimersByTimeAsync(SAVE_DEBOUNCE_MS + 1_000);
const reviewed = (root: string) => loadLedger(root).entries.length > 0;
/** A review runs in another process: real time passes while the fake clock stands still. */
const untilReviewed = (root: string) =>
  vi.waitFor(() => expect(reviewed(root)).toBe(true), { timeout: 20_000, interval: 50 });

describe('review on save', () => {
  beforeEach(() => {
    resetMock();
    vi.useFakeTimers();
  });
  afterEach(() => vi.useRealTimers());

  it('does nothing unless the setting is on', async () => {
    const { root, badFile, save } = await setup();
    save(badFile);
    await settle();
    expect(reviewed(root)).toBe(false);
  });

  it('reviews the saved file, quietly, once the editor has been idle a moment', async () => {
    const { root, badFile, save } = await setup();
    mock.settings['debuggatha.reviewOnSave'] = true;

    save(badFile);
    await vi.advanceTimersByTimeAsync(SAVE_DEBOUNCE_MS - 50);
    expect(reviewed(root)).toBe(false); // still waiting: a burst of saves is one review
    await settle();
    await untilReviewed(root);

    expect(mock.infos).toEqual([]);
    expect(mock.errors).toEqual([]);
    expect(mock.progressLocations).toEqual([10]); // the status bar, not a notification
  });

  it('a burst of saves is a single review', async () => {
    const { badFile, save } = await setup();
    mock.settings['debuggatha.reviewOnSave'] = true;
    for (let i = 0; i < 5; i++) {
      save(badFile);
      await vi.advanceTimersByTimeAsync(100);
    }
    await settle();
    expect(mock.progressLocations).toHaveLength(1);
  });

  it('only ever reviews the file that was saved, whatever the depth', async () => {
    const { root, badFile, save } = await setup();
    mock.settings['debuggatha.reviewOnSave'] = true;
    mock.settings['debuggatha.reviewDepth'] = 'architectural';
    save(badFile);
    await settle();
    await untilReviewed(root);
    const files = new Set(loadLedger(root).entries.map((entry) => entry.fingerprint.file));
    expect(files).toEqual(new Set(['src/bad.ts']));
  });

  it('does not review what is outside the workspace or is not a file on disk', async () => {
    const { root, save } = await setup();
    mock.settings['debuggatha.reviewOnSave'] = true;
    save('/somewhere/else/outside.ts');
    save(join(root, 'src', 'bad.ts'), 'untitled');
    await settle();
    expect(mock.progressLocations).toEqual([]);
  });

  it("leaves the project's own code to the daemon: what it does not own yields no findings", async () => {
    const { root, services, save } = await setup();
    mock.settings['debuggatha.reviewOnSave'] = true;
    mkdirSync(join(root, 'node_modules', 'dep'), { recursive: true });
    writeFileSync(join(root, 'node_modules', 'dep', 'index.js'), 'eval("1");\n');
    save(join(root, 'node_modules', 'dep', 'index.js'));
    await settle();
    await vi.waitFor(() => expect(services.coordinator.running).toBeUndefined(), { interval: 50 });
    expect(reviewed(root)).toBe(false);
  });

  it('does nothing without a workspace', async () => {
    const { save } = await setup();
    mock.workspaceFolders = undefined;
    mock.settings['debuggatha.reviewOnSave'] = true;
    save('/repo/a.ts');
    await settle();
    expect(mock.progressLocations).toEqual([]);
  });
});
