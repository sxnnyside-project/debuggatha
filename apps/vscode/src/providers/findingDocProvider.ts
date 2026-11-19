/**
 * Virtual documents that show one finding in full.
 *
 * @packageDocumentation
 */

import * as vscode from 'vscode';
import type { Finding } from '../contracts/index.js';

/** Shows a finding in a read-only document. */
export class FindingDocProvider implements vscode.TextDocumentContentProvider {
  /** The URI scheme of those documents. */
  static scheme = 'debuggatha-finding';

  private _onDidChange = new vscode.EventEmitter<vscode.Uri>();
  /** Fires when the content of a document changed. */
  readonly onDidChange = this._onDidChange.event;

  private findingsMap = new Map<string, Finding>();

  /** Remembers the finding that `uri` shows. */
  public registerFinding(finding: Finding): vscode.Uri {
    this.findingsMap.set(finding.id, finding);
    return vscode.Uri.parse(`${FindingDocProvider.scheme}:${finding.id}.md`);
  }

  /** The Markdown of the finding the document shows. */
  provideTextDocumentContent(uri: vscode.Uri): string {
    const id = uri.path.replace('.md', '');
    const finding = this.findingsMap.get(id);

    if (!finding) {
      return '# Finding Not Found';
    }

    let md = `# [${finding.severity.toUpperCase()}] ${finding.title}\n\n`;

    if (finding.appliedPolicy) {
      md += `**Policy Reference**: \`${finding.appliedPolicy.id}\`\n\n`;
    }

    if (finding.locations && finding.locations.length > 0) {
      const loc = finding.locations[0];
      if (loc?.file) {
        md += `**Location**: \`${loc.file}${loc.lines ? `:${loc.lines.start}` : ''}\`\n\n`;
      }
    }

    if (finding.evidence) {
      md += `## Evidence\n\n`;
      for (const e of finding.evidence) {
        if ('excerpt' in e && e.excerpt) {
          md += `- ${e.excerpt}\n`;
        } else if ('detail' in e) {
          md += `- ${e.detail}\n`;
        } else if ('source' in e) {
          md += `- ${e.source}\n`;
        } else {
          md += `- ${e.kind}\n`;
        }
      }
      md += '\n';
    }

    if (finding.recommendations && finding.recommendations.length > 0) {
      const rec = finding.recommendations[0];
      if (rec) {
        md += `## Recommendation (${rec.action})\n\n${rec.summary}\n`;
        if (rec.rationale) {
          md += `\n**Rationale**: ${rec.rationale}\n`;
        }
      }
    }

    return md;
  }
}
