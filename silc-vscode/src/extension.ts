import * as vscode from 'vscode';
import { LanguageClient, LanguageClientOptions, ServerOptions, TransportKind } from 'vscode-languageclient';

export function activate(context: vscode.ExtensionContext) {

	// Use the language client to connect to the SIL language server
	const serverOptions: ServerOptions = {
		run: {
			command: 'silc-lsp',
			args: [],
		},
		debug: {
			command: 'silc-lsp',
			args: [],
		},
	};

	const clientOptions: LanguageClientOptions = {
		// Don't automatically show diagnostics
		diagnosticCollectionName: 'silc-vscode',
		diagnosticCollection: undefined,
		workspaceFolders: false,
		// Emit telemetry events to VS Code
		telemetery: 'off',
		// Cancel any request when the client is cancelled
		cancelRequested: true,
		// Delegate tooltip / hover to the language server
		hover: true,
		// Delegate completion to the language server
		completion: true,
		// Delegate signature help to the language server
		signatureHelp: true,
	};

	// Create the language client
	const languageClient = new LanguageClient(
		'silc-lsp',
		'SIL Language Server',
		serverOptions,
		clientOptions
	);

	// Register the language client to be activated when a .sil file is opened
	const diagnosticCollection = vscode.diagnosticCollection.create('silc-vscode');
	context.subscriptions.push(diagnosticCollection);

	// Push the diagnostic collection to be disposed on extension deactivation
	context.subscriptions.push(diagnosticCollection);

	// Register the command to toggle diagnostics
	const toggleDiagnosticsDisposable = vscode.commands.registerCommand('silc-vscode.toggleDiagnostics', () => {
		const enabled = !diagnosticCollection.enabled;
		diagnosticCollection.enabled = enabled;
		vscode.window.showInformationMessage(`SIL diagnostics ${enabled ? 'enabled' : 'disabled'}`);
	});
	context.subscriptions.push(toggleDiagnosticsDisposable);

	// When the language client is started, connect it to the diagnostic collection
	languageClient.onDidChangeDiagnostics((change) => {
		diagnosticCollection.delete();
		for (const entry of change.items) {
			const diagnostic: vscode.Diagnostic = {
				range: entry.range,
				severity: entry.severity ? entry.severity : vscode.DiagnosticSeverity.Error,
				message: entry.message,
				code: entry.code,
			};
			diagnosticCollection.set(entry.range.start, diagnostic);
		}
	});

	// Start the language client
	const disposable = languageClient.start();
	context.subscriptions.push(disposable);

	// Add a listener for when a .sil file is opened
	const openListener = vscode.workspace.onDidOpenTextDocument((document) => {
		if (document.languageId === 'sil') {
			languageClient.prepareCallHierarchy();
		}
	});
	context.subscriptions.push(openListener);

	// Add a listener for when a .sil file is closed
	const closeListener = vscode.workspace.onDidCloseTextDocument((document) => {
		if (document.languageId === 'sil') {
			// Could cleanup here if needed
		}
	});
	context.subscriptions.push(closeListener);
}

export function deactivate() {
	// Language client will be stopped automatically
}