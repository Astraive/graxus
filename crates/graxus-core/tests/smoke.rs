use graxus_core::{file_types, Language};
use std::path::Path;

#[test]
fn detects_c_family_languages_in_public_api() {
    assert_eq!(
        file_types::detect_language(Path::new("main.c")),
        Language::C
    );
    assert_eq!(
        file_types::detect_language(Path::new("main.cpp")),
        Language::Cpp
    );
    assert_eq!(
        file_types::detect_language(Path::new("service.cs")),
        Language::CSharp
    );
}
