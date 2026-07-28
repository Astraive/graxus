use super::{framework_resolver, FrameworkResolver};

framework_resolver!(AspNetResolver, "aspnet", "csharp");

pub fn resolver() -> impl FrameworkResolver {
    AspNetResolver
}
