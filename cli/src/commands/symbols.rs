use anyhow::Result;

use crate::context::CliContext;

pub fn run(ctx: &CliContext, file: Option<&str>, json: bool) -> Result<()> {
    if json {
        return crate::commands::codemap::run(ctx, true);
    }
    crate::commands::codemap::run_symbols(
        ctx,
        &crate::commands::codemap::SymbolFilter {
            file: file.map(String::from),
            kind: None,
            lang: None,
            exported: false,
            include_tests: true,
            _min_confidence: 0.0,
            limit: 100,
            json: false,
        },
    )
}
