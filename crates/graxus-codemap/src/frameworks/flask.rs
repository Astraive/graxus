use super::{framework_resolver, FrameworkResolver};

framework_resolver!(FlaskResolver, "flask", "python");

pub fn resolver() -> impl FrameworkResolver {
    FlaskResolver
}
