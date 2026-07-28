use super::{framework_resolver, FrameworkResolver};

framework_resolver!(AxumResolver, "axum", "rust");

pub fn resolver() -> impl FrameworkResolver {
    AxumResolver
}
