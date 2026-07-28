//! Minimal LSP (Language Server Protocol) server for graxus.
//!
//! Provides hover, go-to-definition, references, document symbols, and
//! completion using the project's codemap and docgraph.
//!
//! # Security Model
//!
//! This LSP server is designed for **local editor integration only**.
//! It communicates over stdio and has no built-in authentication.
//! Do not expose this server to untrusted networks. If remote access
//! is needed, use an authenticated transport layer (e.g., SSH tunnel).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::path::{Path, PathBuf};

use graxus_codemap::CodeGraph;
use graxus_docgraph::graph::DocGraph;

// ── LSP Types ──────────────────────────────────────────────────────────────

/// A position in a text document (0-based line and character).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// A range in a text document (start inclusive, end exclusive).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

/// A location inside a resource (file path + range).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

/// Text document identifier with position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDocumentPositionParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

/// Text document identifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDocumentIdentifier {
    pub uri: String,
}

/// Parameters for a references request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
    #[serde(default)]
    pub context: Option<ReferenceContext>,
}

/// Context for a references request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceContext {
    #[serde(rename = "includeDeclaration", default)]
    pub include_declaration: bool,
}

/// Parameters for a document symbols request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSymbolParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocumentIdentifier,
}

/// Parameters for a completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

/// LSP initialize params (minimal).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    #[serde(rename = "processId")]
    pub process_id: Option<u32>,
    #[serde(rename = "rootUri")]
    pub root_uri: Option<String>,
    #[serde(rename = "capabilities", default)]
    pub capabilities: serde_json::Value,
}

/// Server capabilities returned during initialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    pub capabilities: ServerCapabilities,
}

/// Capabilities the server advertises.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    #[serde(rename = "hoverProvider", skip_serializing_if = "Option::is_none")]
    pub hover_provider: Option<bool>,
    #[serde(rename = "definitionProvider", skip_serializing_if = "Option::is_none")]
    pub definition_provider: Option<bool>,
    #[serde(rename = "referencesProvider", skip_serializing_if = "Option::is_none")]
    pub references_provider: Option<bool>,
    #[serde(
        rename = "documentSymbolProvider",
        skip_serializing_if = "Option::is_none"
    )]
    pub document_symbol_provider: Option<bool>,
    #[serde(rename = "completionProvider", skip_serializing_if = "Option::is_none")]
    pub completion_provider: Option<CompletionOptions>,
}

/// Completion server options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionOptions {
    #[serde(rename = "triggerCharacters", skip_serializing_if = "Option::is_none")]
    pub trigger_characters: Option<Vec<String>>,
}

/// Hover response content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hover {
    pub contents: HoverContents,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
}

/// Content of a hover response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HoverContents {
    Scalar(MarkedString),
    Array(Vec<MarkedString>),
}

/// A marked string (plain text or language-tagged code).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MarkedString {
    String(String),
    LanguageString { language: String, value: String },
}

/// A completion item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    pub label: String,
    #[serde(rename = "kind", skip_serializing_if = "Option::is_none")]
    pub kind: Option<CompletionItemKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
}

/// LSP completion item kind.
#[derive(Debug, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq, Eq)]
#[repr(u8)]
pub enum CompletionItemKind {
    Function = 3,
    Method = 2,
    Constructor = 4,
    Field = 5,
    Variable = 6,
    Class = 7,
    Interface = 8,
    Module = 9,
    Property = 10,
    Enum = 13,
    Keyword = 14,
    Snippet = 15,
    Constant = 21,
    EnumMember = 22,
    Struct = 23,
    TypeParameter = 26,
}

/// Symbol information for document symbols.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInformation {
    pub name: String,
    pub kind: LspSymbolKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    #[serde(rename = "containerName", skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
}

/// LSP symbol kinds.
#[derive(Debug, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq, Eq)]
#[repr(u8)]
pub enum LspSymbolKind {
    File = 1,
    Module = 2,
    Namespace = 3,
    Package = 4,
    Class = 5,
    Method = 6,
    Property = 7,
    Field = 8,
    Constructor = 9,
    Enum = 10,
    Interface = 11,
    Function = 12,
    Variable = 13,
    Constant = 14,
    String = 15,
    Number = 16,
    Boolean = 17,
    Array = 18,
    Object = 19,
    Key = 20,
    Null = 21,
    EnumMember = 22,
    Struct = 23,
    Event = 24,
    Operator = 25,
    TypeParameter = 26,
}

// ── JSON-RPC framing ───────────────────────────────────────────────────────

/// A JSON-RPC message for LSP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspMessage {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<LspError>,
}

/// An LSP error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspError {
    pub code: i32,
    pub message: String,
}

impl LspMessage {
    /// Create a success response.
    fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            method: None,
            params: None,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response.
    fn error(id: Option<serde_json::Value>, code: i32, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            method: None,
            params: None,
            result: None,
            error: Some(LspError {
                code,
                message: message.to_string(),
            }),
        }
    }
}

// ── LSP Server ─────────────────────────────────────────────────────────────

/// LSP server that uses the loaded codemap and docgraph to answer queries.
pub struct LspServer {
    pub root: PathBuf,
    pub code_graph: Option<CodeGraph>,
    pub doc_graph: Option<DocGraph>,
}

impl LspServer {
    /// Create a new LspServer by loading graphs from the project root.
    pub fn load(root: PathBuf) -> Result<Self> {
        use graxus_core::workspace;

        let docs_dir = workspace::docs_dir(&root);
        let doc_graph = if docs_dir.join("graph.json").exists() {
            Some(DocGraph::load(&docs_dir)?)
        } else {
            None
        };

        let code_dir = workspace::code_dir(&root);
        let code_graph = if code_dir.join("codemap.json").exists() {
            let content = std::fs::read_to_string(code_dir.join("codemap.json"))?;
            Some(serde_json::from_str(&content)?)
        } else {
            None
        };

        tracing::info!("LSP server state loaded from {}", root.display());
        Ok(Self {
            root,
            code_graph,
            doc_graph,
        })
    }

    /// Get the word prefix at the given position for completion filtering.
    fn get_line_content(&self, file_path: &str, line: usize, character: usize) -> Option<String> {
        let full_path = self.root.join(file_path);
        let content = std::fs::read_to_string(&full_path).ok()?;
        let line_text = content.lines().nth(line - 1)?;
        // Extract the word prefix: walk backwards from cursor to find word start
        let chars: Vec<char> = line_text.chars().take(character).collect();
        let mut start = chars.len();
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }
        Some(chars[start..].iter().collect())
    }

    /// Dispatch an LSP message to the appropriate handler.
    pub fn handle(&self, msg: &LspMessage) -> LspMessage {
        let method = match msg.method.as_deref() {
            Some(m) => m,
            None => {
                // This is a response/notification we don't handle
                return LspMessage::success(msg.id.clone(), serde_json::Value::Null);
            }
        };

        match method {
            "initialize" => self.handle_initialize(msg),
            "initialized" => LspMessage::success(msg.id.clone(), serde_json::Value::Null),
            "shutdown" => LspMessage::success(msg.id.clone(), serde_json::Value::Null),
            "textDocument/hover" => self.handle_hover(msg),
            "textDocument/definition" => self.handle_definition(msg),
            "textDocument/references" => self.handle_references(msg),
            "textDocument/documentSymbol" => self.handle_document_symbol(msg),
            "textDocument/completion" => self.handle_completion(msg),
            _ => LspMessage::error(
                msg.id.clone(),
                -32601,
                &format!("Method not found: {}", method),
            ),
        }
    }

    fn handle_initialize(&self, msg: &LspMessage) -> LspMessage {
        let result = InitializeResult {
            capabilities: ServerCapabilities {
                hover_provider: Some(true),
                definition_provider: Some(true),
                references_provider: Some(true),
                document_symbol_provider: Some(true),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".into()]),
                }),
            },
        };
        match serde_json::to_value(&result) {
            Ok(v) => LspMessage::success(msg.id.clone(), v),
            Err(e) => LspMessage::error(
                msg.id.clone(),
                -32603,
                &format!("Serialization error: {}", e),
            ),
        }
    }

    fn handle_hover(&self, msg: &LspMessage) -> LspMessage {
        let params: TextDocumentPositionParams = match parse_params(msg) {
            Ok(p) => p,
            Err(e) => return LspMessage::error(msg.id.clone(), -32602, &e),
        };

        let code_graph = match &self.code_graph {
            Some(g) => g,
            None => return LspMessage::success(msg.id.clone(), serde_json::Value::Null),
        };

        let file_path = uri_to_path(&params.text_document.uri);
        let line = params.position.line as usize + 1; // LSP 0-based -> 1-based

        // Find the symbol at this line in this file
        let symbol = code_graph
            .symbols_in_file(&file_path)
            .into_iter()
            .find(|s| s.line_start <= line && line <= s.line_end);

        let Some(sym) = symbol else {
            return LspMessage::success(msg.id.clone(), serde_json::Value::Null);
        };

        let mut contents = vec![MarkedString::LanguageString {
            language: sym.language.clone(),
            value: if sym.signature.is_empty() {
                format!("{} {}", sym.kind, sym.name)
            } else {
                sym.signature.clone()
            },
        }];
        if let Some(ref doc) = sym.doc_string {
            contents.push(MarkedString::String(doc.clone()));
        }

        // Add docgraph info if available
        if let Some(ref dg) = self.doc_graph {
            if let Some(node) = dg.nodes.iter().find(|n| n.path == file_path) {
                if !node.title.is_empty() && node.title != sym.name {
                    contents.push(MarkedString::String(format!("File: {}", node.title)));
                }
            }
        }
        contents.push(MarkedString::String(format!(
            "{} | {} | lines {}-{}",
            sym.kind, file_path, sym.line_start, sym.line_end
        )));

        let hover = Hover {
            contents: HoverContents::Array(contents),
            range: Some(Range {
                start: Position {
                    line: sym.line_start as u32 - 1,
                    character: 0,
                },
                end: Position {
                    line: sym.line_end as u32 - 1,
                    character: 0,
                },
            }),
        };

        match serde_json::to_value(&hover) {
            Ok(v) => LspMessage::success(msg.id.clone(), v),
            Err(e) => LspMessage::error(
                msg.id.clone(),
                -32603,
                &format!("Serialization error: {}", e),
            ),
        }
    }

    fn handle_definition(&self, msg: &LspMessage) -> LspMessage {
        let params: TextDocumentPositionParams = match parse_params(msg) {
            Ok(p) => p,
            Err(e) => return LspMessage::error(msg.id.clone(), -32602, &e),
        };

        let code_graph = match &self.code_graph {
            Some(g) => g,
            None => return LspMessage::success(msg.id.clone(), serde_json::Value::Null),
        };

        let file_path = uri_to_path(&params.text_document.uri);
        let line = params.position.line as usize + 1;

        // Find symbol at position
        let symbol = code_graph
            .symbols_in_file(&file_path)
            .into_iter()
            .find(|s| s.line_start <= line && line <= s.line_end);

        let Some(sym) = symbol else {
            return LspMessage::success(msg.id.clone(), serde_json::Value::Null);
        };

        // Return location of the definition
        let uri = if self.root.join(&sym.file).exists() {
            path_to_uri(&self.root.join(&sym.file))
        } else {
            params.text_document.uri.clone()
        };

        let location = Location {
            uri,
            range: Range {
                start: Position {
                    line: sym.line_start as u32 - 1,
                    character: 0,
                },
                end: Position {
                    line: sym.line_start as u32 - 1,
                    character: 0,
                },
            },
        };

        match serde_json::to_value(&location) {
            Ok(v) => LspMessage::success(msg.id.clone(), v),
            Err(e) => LspMessage::error(
                msg.id.clone(),
                -32603,
                &format!("Serialization error: {}", e),
            ),
        }
    }

    fn handle_references(&self, msg: &LspMessage) -> LspMessage {
        let params: ReferenceParams = match parse_params(msg) {
            Ok(p) => p,
            Err(e) => return LspMessage::error(msg.id.clone(), -32602, &e),
        };

        let code_graph = match &self.code_graph {
            Some(g) => g,
            None => return LspMessage::success(msg.id.clone(), serde_json::json!([])),
        };

        let file_path = uri_to_path(&params.text_document.uri);
        let line = params.position.line as usize + 1;

        // Find the symbol name at this position
        let symbol = code_graph
            .symbols_in_file(&file_path)
            .into_iter()
            .find(|s| s.line_start <= line && line <= s.line_end);

        let Some(sym) = symbol else {
            return LspMessage::success(msg.id.clone(), serde_json::json!([]));
        };

        let symbol_id = format!("{}::{}", sym.file, sym.name);

        // Find all call sites referencing this symbol
        let mut locations: Vec<Location> = Vec::new();

        // Add the definition itself
        locations.push(Location {
            uri: path_to_uri(&self.root.join(&sym.file)),
            range: Range {
                start: Position {
                    line: sym.line_start as u32 - 1,
                    character: 0,
                },
                end: Position {
                    line: sym.line_end as u32 - 1,
                    character: 0,
                },
            },
        });

        // Add call sites
        for call in code_graph.calls_to_symbol(&symbol_id) {
            locations.push(Location {
                uri: path_to_uri(&self.root.join(&call.file)),
                range: Range {
                    start: Position {
                        line: call.line as u32 - 1,
                        character: call.column as u32,
                    },
                    end: Position {
                        line: call.line as u32 - 1,
                        character: call.column as u32,
                    },
                },
            });
        }

        // Also find symbols with the same name in other files (cross-file references)
        for other in code_graph.find_symbols(&sym.name) {
            if other.id != sym.id {
                locations.push(Location {
                    uri: path_to_uri(&self.root.join(&other.file)),
                    range: Range {
                        start: Position {
                            line: other.line_start as u32 - 1,
                            character: 0,
                        },
                        end: Position {
                            line: other.line_end as u32 - 1,
                            character: 0,
                        },
                    },
                });
            }
        }

        match serde_json::to_value(&locations) {
            Ok(v) => LspMessage::success(msg.id.clone(), v),
            Err(e) => LspMessage::error(
                msg.id.clone(),
                -32603,
                &format!("Serialization error: {}", e),
            ),
        }
    }

    fn handle_document_symbol(&self, msg: &LspMessage) -> LspMessage {
        let params: DocumentSymbolParams = match parse_params(msg) {
            Ok(p) => p,
            Err(e) => return LspMessage::error(msg.id.clone(), -32602, &e),
        };

        let code_graph = match &self.code_graph {
            Some(g) => g,
            None => return LspMessage::success(msg.id.clone(), serde_json::json!([])),
        };

        let file_path = uri_to_path(&params.text_document.uri);
        let symbols = code_graph.symbols_in_file(&file_path);

        let infos: Vec<SymbolInformation> = symbols
            .into_iter()
            .map(|s| SymbolInformation {
                name: s.name.clone(),
                kind: map_symbol_kind(s.kind),
                location: Some(Location {
                    uri: params.text_document.uri.clone(),
                    range: Range {
                        start: Position {
                            line: s.line_start as u32 - 1,
                            character: 0,
                        },
                        end: Position {
                            line: s.line_end as u32 - 1,
                            character: 0,
                        },
                    },
                }),
                container_name: None,
            })
            .collect();

        match serde_json::to_value(&infos) {
            Ok(v) => LspMessage::success(msg.id.clone(), v),
            Err(e) => LspMessage::error(
                msg.id.clone(),
                -32603,
                &format!("Serialization error: {}", e),
            ),
        }
    }

    fn handle_completion(&self, msg: &LspMessage) -> LspMessage {
        let params: CompletionParams = match parse_params(msg) {
            Ok(p) => p,
            Err(e) => return LspMessage::error(msg.id.clone(), -32602, &e),
        };

        let code_graph = match &self.code_graph {
            Some(g) => g,
            None => return LspMessage::success(msg.id.clone(), serde_json::json!([])),
        };

        let file_path = uri_to_path(&params.text_document.uri);
        let line = params.position.line as usize + 1;
        let character = params.position.character as usize;

        // Get the current line text to determine the prefix for filtering
        let prefix = self
            .get_line_content(&file_path, line, character)
            .unwrap_or_default();

        // Collect all unique symbol names as completion items, filtered by prefix
        let mut seen = std::collections::HashSet::new();
        let items: Vec<CompletionItem> = code_graph
            .symbols
            .iter()
            .filter_map(|s| {
                if !seen.insert(s.name.clone()) {
                    return None;
                }
                // Filter by prefix if we have one
                if !prefix.is_empty() && !s.name.to_lowercase().starts_with(&prefix.to_lowercase())
                {
                    return None;
                }
                Some(CompletionItem {
                    label: s.name.clone(),
                    kind: Some(map_completion_kind(s.kind)),
                    detail: Some(format!("{} {}", s.kind, s.file)),
                    documentation: if s.signature.is_empty() {
                        None
                    } else {
                        Some(s.signature.clone())
                    },
                })
            })
            .collect();

        match serde_json::to_value(&items) {
            Ok(v) => LspMessage::success(msg.id.clone(), v),
            Err(e) => LspMessage::error(
                msg.id.clone(),
                -32603,
                &format!("Serialization error: {}", e),
            ),
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn parse_params<T: serde::de::DeserializeOwned>(msg: &LspMessage) -> Result<T, String> {
    let params = msg.params.as_ref().ok_or("Missing params")?;
    serde_json::from_value(params.clone()).map_err(|e| format!("Invalid params: {}", e))
}

fn uri_to_path(uri: &str) -> String {
    // Strip file:// prefix
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    // On Windows, file:///C:/path -> /C:/path, strip leading /
    #[cfg(windows)]
    let path = path.strip_prefix('/').unwrap_or(path);
    // URL-decode the path (handles %20, etc.)
    let decoded = url_decode(path);
    decoded.replace('/', "\\").to_string()
}

/// Simple URL decoding for percent-encoded characters.
fn url_decode(input: &str) -> String {
    let mut result = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                result.push(byte);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            result.push(b' ');
        } else {
            result.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8(result).unwrap_or_else(|_| input.to_string())
}

fn path_to_uri(path: &Path) -> String {
    let s = path.to_string_lossy().to_string();
    // On Windows, C:\foo -> file:///C:/foo (prepend / for drive letter)
    #[cfg(windows)]
    let s = format!("/{}", s.replace('\\', "/"));
    #[cfg(not(windows))]
    let s = s.replace('\\', "/");
    format!("file://{}", s)
}

fn map_symbol_kind(kind: graxus_codemap::SymbolKind) -> LspSymbolKind {
    match kind {
        graxus_codemap::SymbolKind::Function => LspSymbolKind::Function,
        graxus_codemap::SymbolKind::Class => LspSymbolKind::Class,
        graxus_codemap::SymbolKind::Struct => LspSymbolKind::Struct,
        graxus_codemap::SymbolKind::Trait => LspSymbolKind::Interface,
        graxus_codemap::SymbolKind::Interface => LspSymbolKind::Interface,
        graxus_codemap::SymbolKind::Method => LspSymbolKind::Method,
        graxus_codemap::SymbolKind::Module => LspSymbolKind::Module,
        graxus_codemap::SymbolKind::Constant => LspSymbolKind::Constant,
        graxus_codemap::SymbolKind::Enum => LspSymbolKind::Enum,
        graxus_codemap::SymbolKind::Type => LspSymbolKind::TypeParameter,
        graxus_codemap::SymbolKind::Variable => LspSymbolKind::Variable,
        graxus_codemap::SymbolKind::Constructor => LspSymbolKind::Constructor,
        graxus_codemap::SymbolKind::Destructor => LspSymbolKind::Method,
        graxus_codemap::SymbolKind::Getter
        | graxus_codemap::SymbolKind::Setter
        | graxus_codemap::SymbolKind::Property => LspSymbolKind::Property,
        graxus_codemap::SymbolKind::Event => LspSymbolKind::Event,
        graxus_codemap::SymbolKind::Delegate => LspSymbolKind::Function,
        graxus_codemap::SymbolKind::Namespace => LspSymbolKind::Namespace,
    }
}

fn map_completion_kind(kind: graxus_codemap::SymbolKind) -> CompletionItemKind {
    match kind {
        graxus_codemap::SymbolKind::Function => CompletionItemKind::Function,
        graxus_codemap::SymbolKind::Class => CompletionItemKind::Class,
        graxus_codemap::SymbolKind::Struct => CompletionItemKind::Struct,
        graxus_codemap::SymbolKind::Trait => CompletionItemKind::Interface,
        graxus_codemap::SymbolKind::Interface => CompletionItemKind::Interface,
        graxus_codemap::SymbolKind::Method => CompletionItemKind::Method,
        graxus_codemap::SymbolKind::Module => CompletionItemKind::Module,
        graxus_codemap::SymbolKind::Constant => CompletionItemKind::Variable,
        graxus_codemap::SymbolKind::Enum => CompletionItemKind::Enum,
        graxus_codemap::SymbolKind::Type => CompletionItemKind::TypeParameter,
        graxus_codemap::SymbolKind::Variable => CompletionItemKind::Variable,
        graxus_codemap::SymbolKind::Constructor => CompletionItemKind::Constructor,
        graxus_codemap::SymbolKind::Destructor => CompletionItemKind::Method,
        graxus_codemap::SymbolKind::Getter
        | graxus_codemap::SymbolKind::Setter
        | graxus_codemap::SymbolKind::Property => CompletionItemKind::Property,
        graxus_codemap::SymbolKind::Event => CompletionItemKind::Property,
        graxus_codemap::SymbolKind::Delegate => CompletionItemKind::Function,
        graxus_codemap::SymbolKind::Namespace => CompletionItemKind::Module,
    }
}

/// Parse an LSP message from a raw JSON string.
pub fn parse_lsp_message(raw: &str) -> Result<LspMessage> {
    Ok(serde_json::from_str(raw)?)
}

/// Serialize an LSP message to a JSON string with Content-Length header.
pub fn serialize_lsp_message(msg: &LspMessage) -> Result<String> {
    let body = serde_json::to_string(msg)?;
    Ok(format!("Content-Length: {}\r\n\r\n{}", body.len(), body))
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_initialize_request() {
        let raw = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":1234,"rootUri":"file:///project"}}"#;
        let msg = parse_lsp_message(raw).unwrap();
        assert_eq!(msg.method.as_deref(), Some("initialize"));
        assert_eq!(msg.id, Some(serde_json::json!(1)));

        let params: InitializeParams = serde_json::from_value(msg.params.unwrap()).unwrap();
        assert_eq!(params.process_id, Some(1234));
        assert_eq!(params.root_uri, Some("file:///project".to_string()));
    }

    #[test]
    fn serialize_initialize_response() {
        let msg = LspMessage::success(
            Some(serde_json::json!(1)),
            serde_json::to_value(&InitializeResult {
                capabilities: ServerCapabilities {
                    hover_provider: Some(true),
                    definition_provider: Some(true),
                    references_provider: Some(true),
                    document_symbol_provider: Some(true),
                    completion_provider: Some(CompletionOptions {
                        trigger_characters: Some(vec![".".into()]),
                    }),
                },
            })
            .unwrap(),
        );
        let serialized = serialize_lsp_message(&msg).unwrap();
        assert!(serialized.contains("Content-Length:"));
        assert!(serialized.contains("hoverProvider"));
        assert!(serialized.contains("definitionProvider"));
    }

    #[test]
    fn parse_hover_request() {
        let raw = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///src/main.rs"},"position":{"line":10,"character":5}}}"#;
        let msg = parse_lsp_message(raw).unwrap();
        let params: TextDocumentPositionParams =
            serde_json::from_value(msg.params.unwrap()).unwrap();
        assert_eq!(params.text_document.uri, "file:///src/main.rs");
        assert_eq!(params.position.line, 10);
        assert_eq!(params.position.character, 5);
    }

    #[test]
    fn parse_definition_request() {
        let raw = r#"{"jsonrpc":"2.0","id":3,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///src/lib.rs"},"position":{"line":5,"character":12}}}"#;
        let msg = parse_lsp_message(raw).unwrap();
        let params: TextDocumentPositionParams =
            serde_json::from_value(msg.params.unwrap()).unwrap();
        assert_eq!(params.text_document.uri, "file:///src/lib.rs");
    }

    #[test]
    fn parse_references_request() {
        let raw = r#"{"jsonrpc":"2.0","id":4,"method":"textDocument/references","params":{"textDocument":{"uri":"file:///src/lib.rs"},"position":{"line":5,"character":12},"context":{"includeDeclaration":true}}}"#;
        let msg = parse_lsp_message(raw).unwrap();
        let params: ReferenceParams = serde_json::from_value(msg.params.unwrap()).unwrap();
        assert!(params.context.unwrap().include_declaration);
    }

    #[test]
    fn parse_document_symbol_request() {
        let raw = r#"{"jsonrpc":"2.0","id":5,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":"file:///src/lib.rs"}}}"#;
        let msg = parse_lsp_message(raw).unwrap();
        let params: DocumentSymbolParams = serde_json::from_value(msg.params.unwrap()).unwrap();
        assert_eq!(params.text_document.uri, "file:///src/lib.rs");
    }

    #[test]
    fn parse_completion_request() {
        let raw = r#"{"jsonrpc":"2.0","id":6,"method":"textDocument/completion","params":{"textDocument":{"uri":"file:///src/lib.rs"},"position":{"line":10,"character":5}}}"#;
        let msg = parse_lsp_message(raw).unwrap();
        let params: CompletionParams = serde_json::from_value(msg.params.unwrap()).unwrap();
        assert_eq!(params.position.line, 10);
    }

    #[test]
    fn hover_response_format() {
        let hover = Hover {
            contents: HoverContents::Array(vec![
                MarkedString::LanguageString {
                    language: "rust".into(),
                    value: "fn main()".into(),
                },
                MarkedString::String("The entry point".into()),
            ]),
            range: Some(Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 10,
                },
            }),
        };
        let json = serde_json::to_value(&hover).unwrap();
        assert!(json["contents"].is_array());
        assert_eq!(json["contents"][0]["language"], "rust");
        assert_eq!(json["contents"][0]["value"], "fn main()");
        assert_eq!(json["contents"][1], "The entry point");
    }

    #[test]
    fn symbol_information_format() {
        let info = SymbolInformation {
            name: "my_func".into(),
            kind: LspSymbolKind::Function,
            location: Some(Location {
                uri: "file:///src/main.rs".into(),
                range: Range {
                    start: Position {
                        line: 5,
                        character: 0,
                    },
                    end: Position {
                        line: 10,
                        character: 1,
                    },
                },
            }),
            container_name: None,
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["name"], "my_func");
        assert_eq!(json["kind"], 12); // Function
        assert_eq!(json["location"]["range"]["start"]["line"], 5);
    }

    #[test]
    fn completion_item_format() {
        let item = CompletionItem {
            label: "my_func".into(),
            kind: Some(CompletionItemKind::Function),
            detail: Some("function src/main.rs".into()),
            documentation: Some("Does something".into()),
        };
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["label"], "my_func");
        assert_eq!(json["kind"], 3); // Function
    }

    #[test]
    fn uri_path_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("src").join("main.rs");
        let uri = path_to_uri(&file);
        assert!(uri.starts_with("file://"));
        let back = uri_to_path(&uri);
        assert!(back.contains("src"));
        assert!(back.contains("main.rs"));
    }

    #[test]
    fn unknown_method_returns_error() {
        let server = LspServer {
            root: std::env::temp_dir(),
            code_graph: None,
            doc_graph: None,
        };
        let msg = LspMessage {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: Some("textDocument/foldingRange".into()),
            params: None,
            result: None,
            error: None,
        };
        let resp = server.handle(&msg);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[test]
    fn handle_initialize_returns_capabilities() {
        let server = LspServer {
            root: std::env::temp_dir(),
            code_graph: None,
            doc_graph: None,
        };
        let msg = LspMessage {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: Some("initialize".into()),
            params: Some(serde_json::json!({})),
            result: None,
            error: None,
        };
        let resp = server.handle(&msg);
        assert!(resp.error.is_none());
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(result["capabilities"]["hoverProvider"].as_bool().unwrap());
        assert!(result["capabilities"]["definitionProvider"]
            .as_bool()
            .unwrap());
    }

    #[test]
    fn lsp_symbol_kind_mapping() {
        assert_eq!(
            map_symbol_kind(graxus_codemap::SymbolKind::Function),
            LspSymbolKind::Function
        );
        assert_eq!(
            map_symbol_kind(graxus_codemap::SymbolKind::Struct),
            LspSymbolKind::Struct
        );
        assert_eq!(
            map_symbol_kind(graxus_codemap::SymbolKind::Enum),
            LspSymbolKind::Enum
        );
        assert_eq!(
            map_symbol_kind(graxus_codemap::SymbolKind::Trait),
            LspSymbolKind::Interface
        );
    }

    #[test]
    fn content_length_header_format() {
        let msg = LspMessage::success(Some(serde_json::json!(1)), serde_json::json!("ok"));
        let serialized = serialize_lsp_message(&msg).unwrap();
        assert!(serialized.starts_with("Content-Length: "));
        assert!(serialized.contains("\r\n\r\n"));
        // The body after the header should be valid JSON
        let body_start = serialized.find("\r\n\r\n").unwrap() + 4;
        let body = &serialized[body_start..];
        let parsed: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(parsed["result"], "ok");
    }
}
