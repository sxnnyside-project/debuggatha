import * as vscode from 'vscode';
import * as path from 'path';
import { FileContext } from '../types';

/**
 * Service for picking and reading workspace files.
 * All file selection is workspace-scoped — no OS dialogs.
 */
export class WorkspaceFileService {
    private selectedFiles: vscode.Uri[] = [];

    /**
     * Show VS Code QuickPick with workspace files for multi-select.
     * Excludes common non-source directories.
     */
    public async pickFiles(): Promise<vscode.Uri[]> {
        const config = vscode.workspace.getConfiguration('debuggatha');
        const maxFiles = config.get<number>('maxFilesPerRequest', 10);

        const files = await vscode.workspace.findFiles(
            '**/*',
            '{**/node_modules/**,**/.git/**,**/dist/**,**/build/**,**/.next/**,**/coverage/**,**/*.min.js,**/*.map,**/*.lock,**/package-lock.json,**/.vscode/**,**/.idea/**,**/venv/**,**/__pycache__/**,**/.env,**/.env.*,**/env/**,**/*.pyc,**/.DS_Store,**/Thumbs.db,**/.cache/**,**/tmp/**,**/temp/**}',
            500
        );

        if (files.length === 0) {
            vscode.window.showInformationMessage('No files found in workspace.');
            return this.selectedFiles;
        }

        // Sort by path for easier navigation
        files.sort((a, b) => a.fsPath.localeCompare(b.fsPath));

        const items = files.map(file => {
            const rel = vscode.workspace.asRelativePath(file);
            return {
                label: path.basename(rel),
                description: path.dirname(rel) === '.' ? '' : path.dirname(rel),
                uri: file,
                picked: this.selectedFiles.some(sf => sf.fsPath === file.fsPath),
            };
        });

        const selected = await vscode.window.showQuickPick(items, {
            canPickMany: true,
            placeHolder: `Select files to analyze (max ${maxFiles})`,
            matchOnDescription: true,
        });

        if (selected) {
            this.selectedFiles = selected.slice(0, maxFiles).map(i => i.uri);

            if (selected.length > maxFiles) {
                vscode.window.showWarningMessage(
                    `Selection trimmed to ${maxFiles} files (debuggatha.maxFilesPerRequest).`
                );
            }
        }

        return this.selectedFiles;
    }

    /**
     * Remove a file from the current selection by index.
     */
    public removeFile(index: number): vscode.Uri[] {
        if (index >= 0 && index < this.selectedFiles.length) {
            this.selectedFiles.splice(index, 1);
        }
        return this.selectedFiles;
    }

    /**
     * Return the relative paths of the currently selected files.
     */
    public getSelectedRelativePaths(): string[] {
        return this.selectedFiles.map(f => vscode.workspace.asRelativePath(f));
    }

    /**
     * Return the current selection as URIs.
     */
    public getSelectedUris(): vscode.Uri[] {
        return [...this.selectedFiles];
    }

    /**
     * Clear the current file selection.
     */
    public clearSelection(): void {
        this.selectedFiles = [];
    }

    /**
     * Read file contents for all selected files, respecting size limits.
     * Returns structured FileContext objects ready for prompt injection.
     */
    public async readSelectedFiles(): Promise<FileContext[]> {
        const config = vscode.workspace.getConfiguration('debuggatha');
        const maxFileSize = config.get<number>('maxFileSize', 100000);
        const results: FileContext[] = [];

        for (const uri of this.selectedFiles) {
            try {
                const raw = await vscode.workspace.fs.readFile(uri);
                const size = raw.length;

                if (size > maxFileSize) {
                    results.push({
                        path: uri.fsPath,
                        relativePath: vscode.workspace.asRelativePath(uri),
                        content: `[File too large: ${(size / 1024).toFixed(1)} KB — limit is ${(maxFileSize / 1024).toFixed(1)} KB]`,
                        language: langFromPath(uri.fsPath),
                        size,
                    });
                } else {
                    results.push({
                        path: uri.fsPath,
                        relativePath: vscode.workspace.asRelativePath(uri),
                        content: Buffer.from(raw).toString('utf8'),
                        language: langFromPath(uri.fsPath),
                        size,
                    });
                }
            } catch (err) {
                results.push({
                    path: uri.fsPath,
                    relativePath: vscode.workspace.asRelativePath(uri),
                    content: `[Error reading file: ${err}]`,
                    language: 'text',
                    size: 0,
                });
            }
        }

        return results;
    }
}

/** Map file extension to language identifier */
function langFromPath(filePath: string): string {
    const ext = path.extname(filePath).toLowerCase();
    const map: Record<string, string> = {
        '.ts': 'typescript', '.tsx': 'typescript', '.js': 'javascript', '.jsx': 'javascript',
        '.py': 'python', '.java': 'java', '.cpp': 'cpp', '.c': 'c', '.cs': 'csharp',
        '.go': 'go', '.rs': 'rust', '.rb': 'ruby', '.php': 'php', '.swift': 'swift',
        '.kt': 'kotlin', '.dart': 'dart', '.html': 'html', '.css': 'css', '.scss': 'scss',
        '.json': 'json', '.xml': 'xml', '.yaml': 'yaml', '.yml': 'yaml',
        '.md': 'markdown', '.sql': 'sql', '.sh': 'bash', '.ps1': 'powershell',
        '.vue': 'vue', '.svelte': 'svelte',
    };
    return map[ext] || 'text';
}
