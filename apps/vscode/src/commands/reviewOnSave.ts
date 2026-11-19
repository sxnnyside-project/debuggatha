import * as vscode from 'vscode';
import { getSettings } from '../config/settings.js';
import { toRepoRelative } from '../paths.js';
import type { Services } from '../services.js';
import { runReview } from './review.js';

/** How long to wait after a save before reviewing, so a burst of saves is one review. */
export const SAVE_DEBOUNCE_MS = 400;

/**
 * Reviews a file when it is saved, if `debuggatha.reviewOnSave` is on. It reviews only that file
 * (never widened, whatever the depth), stays silent, and steps aside for a review a person asked for.
 */
export function registerReviewOnSave(context: vscode.ExtensionContext, services: Services) {
  let timer: ReturnType<typeof setTimeout> | undefined;

  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument((document) => {
      const rootDir = services.rootDir;
      if (!rootDir || !getSettings().reviewOnSave) return;
      if (document.uri.scheme !== 'file') return;

      const relative = toRepoRelative(rootDir, document.uri.fsPath);
      // Outside the workspace (still absolute). Code the project does not own is the daemon's call.
      if (relative.startsWith('/') || /^[a-zA-Z]:/.test(relative)) return;

      if (timer) clearTimeout(timer);
      timer = setTimeout(() => {
        timer = undefined;
        void runReview(
          services,
          { kind: 'file', path: relative, owned: true },
          { origin: 'save', widen: false },
        );
      }, SAVE_DEBOUNCE_MS);
    }),
    { dispose: () => timer && clearTimeout(timer) },
  );
}
