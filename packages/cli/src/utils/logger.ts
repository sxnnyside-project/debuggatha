import pc from "picocolors";

export interface LoggerOptions {
  json: boolean;
  verbose: boolean;
  quiet: boolean;
}

export class CliLogger {
  constructor(private options: LoggerOptions) {}

  public get isJson() {
    return this.options.json;
  }

  public get isVerbose() {
    return this.options.verbose;
  }

  log(message: string) {
    if (this.options.json || this.options.quiet) return;
    console.log(message);
  }

  success(message: string) {
    if (this.options.json || this.options.quiet) return;
    console.log(pc.green(`✔ ${message}`));
  }

  warn(message: string) {
    if (this.options.json || this.options.quiet) return;
    console.warn(pc.yellow(`⚠ ${message}`));
  }

  error(message: string, error?: unknown) {
    if (this.options.json) return;
    console.error(pc.red(`✖ ${message}`));
    if (error && (this.options.verbose || !this.options.json)) {
      if (error instanceof Error) {
        console.error(this.options.verbose ? error.stack : pc.red(error.message));
      } else {
        console.error(error);
      }
    }
  }

  info(message: string) {
    if (this.options.json || this.options.quiet) return;
    console.info(pc.blue(`ℹ ${message}`));
  }

  debug(message: string) {
    if (this.options.json || this.options.quiet || !this.options.verbose) return;
    console.debug(pc.gray(`[debug] ${message}`));
  }

  json(data: unknown) {
    if (!this.options.json) return;
    console.log(JSON.stringify(data, null, 2));
  }
}
