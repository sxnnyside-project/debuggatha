/**
 * Which `debuggatha` binaries this extension works with. The handshake enforces the protocol; this
 * names the side to update when the versions are apart. They are released in lockstep, so the
 * range is the minor line the extension was built against.
 */
export const SUPPORTED_CLI = { min: [0, 3, 0], below: [0, 4, 0] } as const;

function parse(version: string): [number, number, number] | undefined {
  const match = /^(\d+)\.(\d+)\.(\d+)/.exec(version);
  return match ? [Number(match[1]), Number(match[2]), Number(match[3])] : undefined;
}

function compare(a: readonly number[], b: readonly number[]): number {
  for (let i = 0; i < 3; i++) {
    const difference = (a[i] ?? 0) - (b[i] ?? 0);
    if (difference !== 0) return difference;
  }
  return 0;
}

/**
 * Whether the binary can be used with this extension, and what to tell the person when it cannot.
 */
export type Compatibility = { ok: true } | { ok: false; message: string };

/** Checks `version`, as the binary reports it, against the range this extension was built for. */
export function checkCliVersion(version: string): Compatibility {
  const found = parse(version);
  const [min, below] = [SUPPORTED_CLI.min.join('.'), SUPPORTED_CLI.below.join('.')];
  if (!found) {
    return {
      ok: false,
      message: `Could not read the version of the Debuggatha CLI ("${version}").`,
    };
  }
  if (compare(found, SUPPORTED_CLI.min) < 0) {
    return {
      ok: false,
      message: `The Debuggatha CLI ${version} is older than this extension supports (${min} up to, not including, ${below}). Update the CLI.`,
    };
  }
  if (compare(found, SUPPORTED_CLI.below) >= 0) {
    return {
      ok: false,
      message: `The Debuggatha CLI ${version} is newer than this extension supports (${min} up to, not including, ${below}). Update the extension.`,
    };
  }
  return { ok: true };
}
