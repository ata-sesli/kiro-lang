'use strict';

const vscode = require('vscode');
const { keywordDocs, moduleDocs } = require('./docs/hoverDocs');

function docToMarkdown(label, doc) {
  const md = new vscode.MarkdownString();
  md.isTrusted = false;
  md.appendMarkdown(`**${label}**\n\n`);
  if (doc.signature) {
    md.appendCodeblock(doc.signature, 'kiro');
  }
  if (doc.detail) {
    md.appendMarkdown(`${doc.detail}\n\n`);
  }
  if (doc.example) {
    md.appendMarkdown('_Example:_\n');
    md.appendCodeblock(doc.example, 'kiro');
  }
  return md;
}

function resolveModuleContext(document, position) {
  const range = new vscode.Range(
    new vscode.Position(position.line, 0),
    position
  );
  const before = document.getText(range);
  const match = before.match(/([a-z_][a-zA-Z0-9_]*)\s*\.\s*[a-z_][a-zA-Z0-9_]*\s*$/);
  return match ? match[1] : null;
}

function provideKiroHover(document, position) {
  const wordRange = document.getWordRangeAtPosition(position, /[a-z_][a-zA-Z0-9_]*/);
  if (!wordRange) {
    return undefined;
  }

  const word = document.getText(wordRange);
  const moduleName = resolveModuleContext(document, position);

  if (moduleName && moduleDocs[moduleName] && moduleDocs[moduleName][word]) {
    return new vscode.Hover(docToMarkdown(`${moduleName}.${word}`, moduleDocs[moduleName][word]), wordRange);
  }

  if (keywordDocs[word]) {
    return new vscode.Hover(docToMarkdown(word, keywordDocs[word]), wordRange);
  }

  return undefined;
}

function activate(context) {
  const provider = vscode.languages.registerHoverProvider(
    { language: 'kiro', scheme: 'file' },
    {
      provideHover(document, position) {
        return provideKiroHover(document, position);
      }
    }
  );

  context.subscriptions.push(provider);
}

function deactivate() {}

module.exports = {
  activate,
  deactivate
};
