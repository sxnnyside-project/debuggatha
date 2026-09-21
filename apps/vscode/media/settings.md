# Configuring Debuggatha

Debuggatha needs no configuration for a standard project: it detects your stack and applies the
matching Review Packs.

## Settings

- **Review Depth** (`debuggatha.reviewDepth`): `quick` runs only the built-in detectors; `full` (default) also runs the analyzers installed on your machine; `architectural` also reviews the whole repository, architecture included.
- **Review on Save** (`debuggatha.reviewOnSave`): review a file when you save it, quietly, in the status bar.
- **Minimum Severity** (`debuggatha.minimumSeverity`): the lowest severity shown as underlines and in Problems. The Findings view lists everything.
- **Extra Review Packs** (`debuggatha.extraReviewPacks`): packs applied on top of the ones detected for your stack. Listed packs win when rules conflict.
- **Enabled Analyzers** (`debuggatha.enabledAnalyzers`): analyzers that run project code or use the network. Off until you list them here.
- **Log Level** (`debuggatha.logLevel`): what the Output channel records. It never contains code.
