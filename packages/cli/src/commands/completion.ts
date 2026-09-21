import { Argument, type Command } from "commander";

const SHELLS = ["bash", "zsh", "fish"] as const;
type Shell = (typeof SHELLS)[number];

interface Node {
  name: string;
  description: string;
  flags: string[];
  children: Node[];
}

const flagsOf = (command: Command): string[] =>
  command.options
    .flatMap((option) => [option.long, option.short])
    .filter((flag): flag is string => !!flag);

function describe(command: Command): Node {
  return {
    name: command.name(),
    description: command.description().replaceAll("'", ""),
    flags: flagsOf(command),
    children: command.commands.filter((sub) => sub.name() !== "help").map(describe),
  };
}

const bash = (root: Node) => `_debuggatha() {
  local cur="\${COMP_WORDS[COMP_CWORD]}" cmd="\${COMP_WORDS[1]}"
  if [ "$COMP_CWORD" -eq 1 ]; then
    COMPREPLY=($(compgen -W "${root.children.map((c) => c.name).join(" ")} ${root.flags.join(" ")}" -- "$cur"))
    return
  fi
  case "$cmd" in
${root.children
  .map(
    (c) =>
      `    ${c.name}) COMPREPLY=($(compgen -W "${[...c.children.map((s) => s.name), ...c.flags].join(" ")}" -- "$cur")) ;;`,
  )
  .join("\n")}
  esac
}
complete -F _debuggatha debuggatha
`;

const zsh = (root: Node) => `#compdef debuggatha
_debuggatha() {
  local -a commands
  commands=(
${root.children.map((c) => `    '${c.name}:${c.description}'`).join("\n")}
  )
  if (( CURRENT == 2 )); then
    _describe 'command' commands
    return
  fi
  case "$words[2]" in
${root.children
  .map(
    (c) =>
      `    ${c.name}) _values 'option' ${[...c.children.map((s) => s.name), ...c.flags].join(" ")} ;;`,
  )
  .join("\n")}
  esac
}
compdef _debuggatha debuggatha
`;

const fish = (root: Node) =>
  [
    ...root.children.map(
      (c) =>
        `complete -c debuggatha -n '__fish_use_subcommand' -a '${c.name}' -d '${c.description}'`,
    ),
    ...root.children.flatMap((c) =>
      [...c.children.map((s) => s.name), ...c.flags].map((word) =>
        word.startsWith("--")
          ? `complete -c debuggatha -n '__fish_seen_subcommand_from ${c.name}' -l '${word.slice(2)}'`
          : `complete -c debuggatha -n '__fish_seen_subcommand_from ${c.name}' -a '${word}'`,
      ),
    ),
  ].join("\n") + "\n";

const GENERATORS: Record<Shell, (root: Node) => string> = { bash, zsh, fish };

export function registerCompletionCommand(program: Command) {
  program
    .command("completion")
    .description("Print a shell completion script")
    .addArgument(new Argument("<shell>", "Target shell").choices(SHELLS))
    .addHelpText("after", "\nExample:\n  debuggatha completion zsh > ~/.zfunc/_debuggatha")
    .action((shell: Shell) => {
      process.stdout.write(GENERATORS[shell](describe(program)));
    });
}
