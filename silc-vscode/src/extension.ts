import * as cp from 'child_process';
import * as path from 'path';
import * as vscode from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;
let salida: vscode.OutputChannel;

function archivoActual(): string | undefined {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== 'sil') {
        void vscode.window.showErrorMessage('Abre un archivo SIL (.sil) primero.');
        return undefined;
    }
    return editor.document.uri.fsPath;
}

function ejecutar(argumentos: (archivo: string) => string[]): void {
    const archivo = archivoActual();
    if (!archivo) {
        return;
    }
    const args = argumentos(archivo);
    salida.show(true);
    salida.appendLine(`$ silc ${args.join(' ')}`);
    const proceso = cp.spawn('silc', args, { shell: false });
    proceso.stdout.on('data', (datos: Buffer) => salida.append(datos.toString()));
    proceso.stderr.on('data', (datos: Buffer) => salida.append(datos.toString()));
    proceso.on('error', (error: Error) =>
        salida.appendLine(`No se pudo ejecutar silc: ${error.message}`),
    );
    proceso.on('close', (codigo: number | null) =>
        salida.appendLine(`silc terminó con código ${codigo ?? '?'}`),
    );
}

export function activate(context: vscode.ExtensionContext): void {
    salida = vscode.window.createOutputChannel('SIL');
    context.subscriptions.push(salida);

    context.subscriptions.push(
        vscode.commands.registerCommand('sil.compile', () =>
            ejecutar((archivo) => [
                'build',
                archivo,
                '--target',
                'c99',
                '-o',
                path.join(path.dirname(archivo), `${path.basename(archivo, '.sil')}.c`),
            ]),
        ),
        vscode.commands.registerCommand('sil.verify', () =>
            ejecutar((archivo) => ['check', archivo]),
        ),
    );

    const serverOptions: ServerOptions = {
        run: { command: 'silc', args: ['lsp'], transport: TransportKind.stdio },
        debug: { command: 'silc', args: ['lsp'], transport: TransportKind.stdio },
    };
    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'sil' }],
    };
    client = new LanguageClient(
        'silc',
        'SIL Language Server',
        serverOptions,
        clientOptions,
    );
    void client.start();
}

export function deactivate(): Thenable<void> | undefined {
    return client?.stop();
}
