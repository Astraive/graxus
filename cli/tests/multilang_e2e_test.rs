//! End-to-end multi-language test suite for Graxus powered by Ripex.
//!
//! Exercises scanning, indexing, Ripex fact extraction, SQLite persistence,
//! and CLI commands across all 10 supported code languages + Markdown.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn bin() -> String {
    env!("CARGO_BIN_EXE_graxus").to_string()
}

fn run(args: &[&str]) -> String {
    let output = Command::new(bin())
        .args(args)
        .output()
        .expect("failed to execute graxus binary");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        output.status.success(),
        "graxus {:?} failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
        args,
        output.status,
        stdout,
        stderr
    );
    stdout
}

fn create_multilang_fixtures(root: &Path) {
    fs::create_dir_all(root.join("src")).unwrap();

    // 1. Rust
    fs::write(
        root.join("src/lib.rs"),
        r#"/// Adds two integers.
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#,
    )
    .unwrap();

    // 2. Python
    fs::write(
        root.join("src/app.py"),
        r#"def calculate_total(price: float, tax: float) -> float:
    """Compute total price including tax."""
    return price * (1.0 + tax)
"#,
    )
    .unwrap();

    // 3. Go
    fs::write(
        root.join("src/main.go"),
        r#"package main

import "fmt"

func ProcessOrder(id int) string {
    return fmt.Sprintf("Order %d", id)
}
"#,
    )
    .unwrap();

    // 4. TypeScript
    fs::write(
        root.join("src/service.ts"),
        r#"export async function fetchUserData(userId: string): Promise<object> {
    return { id: userId, active: true };
}
"#,
    )
    .unwrap();

    // 5. C
    fs::write(
        root.join("src/utils.c"),
        r#"#include <stdio.h>

int format_buffer(char *buf, int len) {
    return len;
}
"#,
    )
    .unwrap();

    // 6. C++
    fs::write(
        root.join("src/engine.cpp"),
        r#"#include <iostream>

class Engine {
public:
    void start() {
        std::cout << "Engine starting..." << std::endl;
    }
};
"#,
    )
    .unwrap();

    // 7. C#
    fs::write(
        root.join("src/Program.cs"),
        r#"namespace App {
    public class Processor {
        public static int Process(int input) {
            return input * 2;
        }
    }
}
"#,
    )
    .unwrap();

    // 8. Java
    fs::write(
        root.join("src/App.java"),
        r#"package com.example;

public class App {
    public void run() {
        System.out.println("Java app running");
    }
}
"#,
    )
    .unwrap();

    // 9. Kotlin
    fs::write(
        root.join("src/App.kt"),
        r#"package com.example

fun execute(): Int {
    return 42
}
"#,
    )
    .unwrap();

    // 10. Swift
    fs::write(
        root.join("src/main.swift"),
        r#"import Foundation

public func runSwift() {
    print("Swift app")
}
"#,
    )
    .unwrap();

    // 11. Markdown
    fs::write(
        root.join("README.md"),
        r#"---
title: MultiLang Architecture
tags: [architecture, test]
---

# MultiLang Project

Integrates [[add]] and [[calculate_total]].
"#,
    )
    .unwrap();
}

#[test]
fn test_end_to_end_multilang_ripex_pipeline() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    let root_str = root.to_str().unwrap();

    // 1. Initialize project
    run(&["init", root_str]);
    assert!(root.join("graxus.yaml").exists());

    create_multilang_fixtures(root);

    // 2. Index project with Ripex backend
    let index_out = run(&["--root", root_str, "index", "--codemap-backend", "ripex"]);
    assert!(
        index_out.contains("Symbols:") || index_out.contains("Indexed"),
        "index output:\n{}",
        index_out
    );
    // 3. Status check
    let status_out = run(&["--root", root_str, "status", "--json"]);
    assert!(status_out.contains("\"name\"") && status_out.contains("subdirs"));

    // 4. Codemap show check
    let codemap_out = run(&["--root", root_str, "codemap", "show"]);
    assert!(
        codemap_out.contains("Symbols:") || codemap_out.contains("Files:") || codemap_out.contains("Codemap"),
        "codemap output:\n{}",
        codemap_out
    );

    // 5. Doctor check
    let doctor_out = run(&["--root", root_str, "doctor"]);
    assert!(
        doctor_out.contains("Diagnostics complete"),
        "doctor output:\n{}",
        doctor_out
    );
}
