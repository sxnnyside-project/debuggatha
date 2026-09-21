// An in-memory stand-in for the slice of the `vscode` API the extension uses.
// Unit tests alias `vscode` to this module (see vitest.config.ts); behavior that
// depends on the real Extension Host is covered by the integration suite instead.
import { readFileSync } from "node:fs";

export class EventEmitter<T> {
  private handlers = new Set<(event: T) => void>();

  event = (handler: (event: T) => void) => {
    this.handlers.add(handler);
    return { dispose: () => this.handlers.delete(handler) };
  };

  fire(event?: T): void {
    for (const handler of this.handlers) handler(event as T);
  }
}

export class Uri {
  constructor(
    readonly scheme: string,
    readonly path: string,
  ) {}

  static file(path: string): Uri {
    return new Uri("file", path);
  }

  static parse(value: string): Uri {
    const separator = value.indexOf(":");
    return new Uri(value.slice(0, separator), value.slice(separator + 1));
  }

  get fsPath(): string {
    return this.path;
  }

  toString(): string {
    return `${this.scheme}:${this.path}`;
  }
}

export enum TreeItemCollapsibleState {
  None = 0,
  Collapsed = 1,
  Expanded = 2,
}

export class TreeItem {
  contextValue?: string;
  description?: string;
  tooltip?: string;
  iconPath?: unknown;
  command?: { command: string; title: string; arguments?: unknown[] };

  constructor(
    readonly label: string,
    readonly collapsibleState?: TreeItemCollapsibleState,
  ) {}
}

export class ThemeIcon {
  constructor(readonly id: string) {}
}

export class Position {
  constructor(
    readonly line: number,
    readonly character: number,
  ) {}
}

export class Range {
  readonly start: Position;
  readonly end: Position;

  constructor(
    readonly startLine: number,
    readonly startCharacter: number,
    readonly endLine: number,
    readonly endCharacter: number,
  ) {
    this.start = new Position(startLine, startCharacter);
    this.end = new Position(endLine, endCharacter);
  }
}

export enum DiagnosticSeverity {
  Error = 0,
  Warning = 1,
  Information = 2,
  Hint = 3,
}

export class Diagnostic {
  source?: string;
  code?: string | number | { value: string | number; target: Uri };

  constructor(
    readonly range: Range,
    readonly message: string,
    readonly severity: DiagnosticSeverity,
  ) {}
}

export class MarkdownString {
  isTrusted?: boolean | { enabledCommands: string[] };

  constructor(readonly value: string) {}
}

export class Hover {
  constructor(readonly contents: MarkdownString | MarkdownString[]) {}
}

export const CodeActionKind = { QuickFix: "quickfix" };

export class WorkspaceEdit {
  readonly replacements: { uri: Uri; range: Range; text: string }[] = [];
  readonly insertions: { uri: Uri; position: Position; text: string }[] = [];

  replace(uri: Uri, range: Range, text: string): void {
    this.replacements.push({ uri, range, text });
  }

  insert(uri: Uri, position: Position, text: string): void {
    this.insertions.push({ uri, position, text });
  }
}

export class CodeAction {
  diagnostics?: Diagnostic[];
  isPreferred?: boolean;
  edit?: WorkspaceEdit;
  command?: { command: string; title: string; arguments?: unknown[] };

  constructor(
    readonly title: string,
    readonly kind?: string,
  ) {}
}

export enum StatusBarAlignment {
  Left = 1,
  Right = 2,
}

export enum ProgressLocation {
  Window = 10,
  Notification = 15,
}

export enum ViewColumn {
  Two = 2,
}

class DiagnosticCollection {
  readonly entries = new Map<string, Diagnostic[]>();

  set(uri: Uri, diagnostics: Diagnostic[]): void {
    this.entries.set(uri.fsPath, diagnostics);
  }

  clear(): void {
    this.entries.clear();
  }

  dispose(): void {}
}

export interface StatusItem {
  text: string;
  tooltip: string;
  command: string | undefined;
  shown: boolean;
  show(): void;
  dispose(): void;
}

/** Everything a test can drive or inspect; `resetMock` returns it to a blank window. */
export const mock = {
  workspaceFolders: undefined as { uri: Uri }[] | undefined,
  settings: {} as Record<string, unknown>,
  trusted: true,
  activeTextEditor: undefined as { document: { uri: Uri } } | undefined,
  errors: [] as string[],
  infos: [] as string[],
  errorChoice: undefined as string | undefined,
  inputAnswer: undefined as string | undefined,
  commands: new Map<string, (...args: never[]) => unknown>(),
  executed: [] as { command: string; args: unknown[] }[],
  collections: new Map<string, DiagnosticCollection>(),
  statusBarItems: [] as StatusItem[],
  configListeners: [] as ((event: { affectsConfiguration: (s: string) => boolean }) => void)[],
  saveListeners: [] as ((document: { uri: Uri }) => void)[],
  contentProviders: new Map<string, unknown>(),
  hoverProviders: [] as unknown[],
  codeActionProviders: [] as unknown[],
  outputLines: [] as string[],
  progressLocations: [] as number[],
  applied: [] as WorkspaceEdit[],
};

export function resetMock(): void {
  mock.workspaceFolders = undefined;
  mock.settings = {};
  mock.trusted = true;
  mock.activeTextEditor = undefined;
  mock.errors = [];
  mock.infos = [];
  mock.errorChoice = undefined;
  mock.inputAnswer = undefined;
  mock.commands.clear();
  mock.executed = [];
  mock.collections.clear();
  mock.statusBarItems = [];
  mock.configListeners = [];
  mock.saveListeners = [];
  mock.contentProviders.clear();
  mock.hoverProviders = [];
  mock.codeActionProviders = [];
  mock.outputLines = [];
  mock.progressLocations = [];
  mock.applied = [];
}

export function openWorkspace(root: string): void {
  mock.workspaceFolders = [{ uri: Uri.file(root) }];
}

/** A text document backed by a real file, with the two methods the extension reads. */
export function documentFor(path: string) {
  const lines = readFileSync(path, "utf8").split("\n");
  return {
    uri: Uri.file(path),
    get lineCount() {
      return lines.length;
    },
    lineAt: (line: number) => ({ text: lines[line] ?? "" }),
  };
}

export const window = {
  get activeTextEditor() {
    return mock.activeTextEditor;
  },
  async showErrorMessage(message: string, ..._items: string[]) {
    mock.errors.push(message);
    return mock.errorChoice;
  },
  async showInformationMessage(message: string, ..._items: string[]) {
    mock.infos.push(message);
    return undefined;
  },
  async showInputBox() {
    return mock.inputAnswer;
  },
  withProgress<T>(
    options: { location: number },
    task: (
      progress: { report: (value: unknown) => void },
      token: { onCancellationRequested: (listener: () => void) => { dispose(): void } },
    ) => Promise<T>,
  ) {
    mock.progressLocations.push(options.location);
    return task({ report: () => {} }, { onCancellationRequested: () => ({ dispose() {} }) });
  },
  createStatusBarItem() {
    const item: StatusItem = {
      text: "",
      tooltip: "",
      command: undefined,
      shown: false,
      show() {
        item.shown = true;
      },
      dispose() {},
    };
    mock.statusBarItems.push(item);
    return item;
  },
  createOutputChannel() {
    return {
      appendLine: (line: string) => mock.outputLines.push(line),
      show() {},
      dispose() {},
    };
  },
  registerTreeDataProvider() {
    return { dispose() {} };
  },
  async showTextDocument() {
    return undefined;
  },
};

export const workspace = {
  get workspaceFolders() {
    return mock.workspaceFolders;
  },
  get isTrusted() {
    return mock.trusted;
  },
  getConfiguration(section: string) {
    return {
      get: <T>(key: string): T | undefined => mock.settings[`${section}.${key}`] as T | undefined,
    };
  },
  onDidChangeConfiguration(listener: (typeof mock.configListeners)[number]) {
    mock.configListeners.push(listener);
    return { dispose() {} };
  },
  onDidSaveTextDocument(listener: (typeof mock.saveListeners)[number]) {
    mock.saveListeners.push(listener);
    return { dispose() {} };
  },
  registerTextDocumentContentProvider(scheme: string, provider: unknown) {
    mock.contentProviders.set(scheme, provider);
    return { dispose() {} };
  },
  async openTextDocument(uri: Uri) {
    return uri.scheme === "file" ? documentFor(uri.fsPath) : { uri };
  },
  async applyEdit(edit: WorkspaceEdit) {
    mock.applied.push(edit);
    return true;
  },
};

export const commands = {
  registerCommand(id: string, handler: (...args: never[]) => unknown) {
    mock.commands.set(id, handler);
    return { dispose: () => mock.commands.delete(id) };
  },
  async executeCommand(id: string, ...args: unknown[]) {
    mock.executed.push({ command: id, args });
    const handler = mock.commands.get(id);
    return handler ? (handler as (...a: unknown[]) => unknown)(...args) : undefined;
  },
};

export const languages = {
  createDiagnosticCollection(name: string) {
    const collection = new DiagnosticCollection();
    mock.collections.set(name, collection);
    return collection;
  },
  registerHoverProvider(_selector: unknown, provider: unknown) {
    mock.hoverProviders.push(provider);
    return { dispose() {} };
  },
  registerCodeActionsProvider(_selector: unknown, provider: unknown) {
    mock.codeActionProviders.push(provider);
    return { dispose() {} };
  },
};
