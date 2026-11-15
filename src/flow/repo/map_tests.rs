use super::*;

#[test]
fn test_rust_symbol_scanner() {
    let content = r#"
// This is a comment: fn ignored_fn() {}
/* block comment: struct IgnoredStruct; */
fn regular_fn() {}
pub fn public_fn(x: i32) -> bool { true }
pub(crate) async fn crate_async_fn() {}
pub unsafe fn danger() {}
pub const fn const_calc() -> u32 { 42 }
struct LocalStruct;
pub struct PubStruct {
    pub field: u32,
}
enum LocalEnum { A, B }
pub enum PubEnum { X, Y }
trait LocalTrait {}
pub trait PubTrait {
    fn trait_fn(&self);
}
impl LocalStruct {}
impl<T> PubTrait for PubStruct {}
"#;

    let symbols = scan_file_symbols("src/main.rs", content);
    assert!(symbols.iter().any(|s| s.contains("fn regular_fn()")));
    assert!(symbols
        .iter()
        .any(|s| s.contains("pub fn public_fn(x: i32) -> bool")));
    assert!(symbols
        .iter()
        .any(|s| s.contains("pub(crate) async fn crate_async_fn()")));
    assert!(symbols.iter().any(|s| s.contains("pub unsafe fn danger()")));
    assert!(symbols
        .iter()
        .any(|s| s.contains("pub const fn const_calc() -> u32")));
    assert!(symbols.iter().any(|s| s.contains("struct LocalStruct")));
    assert!(symbols.iter().any(|s| s.contains("pub struct PubStruct")));
    assert!(symbols.iter().any(|s| s.contains("enum LocalEnum")));
    assert!(symbols.iter().any(|s| s.contains("pub enum PubEnum")));
    assert!(symbols.iter().any(|s| s.contains("trait LocalTrait")));
    assert!(symbols.iter().any(|s| s.contains("pub trait PubTrait")));
    assert!(symbols.iter().any(|s| s.contains("impl LocalStruct")));
    assert!(symbols
        .iter()
        .any(|s| s.contains("impl<T> PubTrait for PubStruct")));

    assert!(!symbols.iter().any(|s| s.contains("ignored_fn")));
    assert!(!symbols.iter().any(|s| s.contains("IgnoredStruct")));
}

#[test]
fn test_python_symbol_scanner() {
    let content = r#"
# Comment: def ignored(): pass
def compute(x, y):
    return x + y

async def fetch_data(url: str):
    pass

class BaseWorker:
    def worker_method(self):
        pass

class AdvancedWorker(BaseWorker):
    pass
"#;

    let symbols = scan_file_symbols("service.py", content);
    assert!(symbols.iter().any(|s| s.contains("def compute(x, y):")));
    assert!(symbols
        .iter()
        .any(|s| s.contains("async def fetch_data(url: str):")));
    assert!(symbols.iter().any(|s| s.contains("class BaseWorker:")));
    assert!(symbols
        .iter()
        .any(|s| s.contains("class AdvancedWorker(BaseWorker):")));
    assert!(symbols
        .iter()
        .any(|s| s.contains("def worker_method(self):")));
    assert!(!symbols.iter().any(|s| s.contains("ignored")));
}

#[test]
fn test_go_symbol_scanner() {
    let content = r#"
// Comment: func ignoredFunc() {}
package main

func Standalone(x int) (int, error) {
    return x, nil
}

func (s *Server) Start() error {
    return nil
}

type Config struct {
    Port int
}

type Runner interface {
    Run() error
}

type UserID string
"#;

    let symbols = scan_file_symbols("server.go", content);
    assert!(symbols
        .iter()
        .any(|s| s.contains("func Standalone(x int) (int, error)")));
    assert!(symbols
        .iter()
        .any(|s| s.contains("func (s *Server) Start() error")));
    assert!(symbols.iter().any(|s| s.contains("type Config struct")));
    assert!(symbols.iter().any(|s| s.contains("type Runner interface")));
    assert!(symbols.iter().any(|s| s.contains("type UserID string")));
    assert!(!symbols.iter().any(|s| s.contains("ignoredFunc")));
}

#[test]
fn test_typescript_symbol_scanner() {
    let content = r#"
// Comment: function ignored() {}
// export const ignoredConst = 1;
function localHelper(): void {}
export function calculateTotal(items: number[]): number {
    return 0;
}
export async function loadConfig(): Promise<void> {}

class LocalClass {}
export class ApiClient {}
export abstract class BaseService {}

interface LocalOptions {}
export interface ClientOptions {
    timeout: number;
}

export const API_URL = "https://api.example.com";
export const maxRetries = 3;
const unexportedConst = 100;
"#;

    let symbols = scan_file_symbols("app.ts", content);
    assert!(symbols
        .iter()
        .any(|s| s.contains("function localHelper(): void")));
    assert!(symbols
        .iter()
        .any(|s| s.contains("export function calculateTotal(items: number[]): number")));
    assert!(symbols
        .iter()
        .any(|s| s.contains("export async function loadConfig(): Promise<void>")));
    assert!(symbols.iter().any(|s| s.contains("class LocalClass")));
    assert!(symbols.iter().any(|s| s.contains("export class ApiClient")));
    assert!(symbols
        .iter()
        .any(|s| s.contains("export abstract class BaseService")));
    assert!(symbols.iter().any(|s| s.contains("interface LocalOptions")));
    assert!(symbols
        .iter()
        .any(|s| s.contains("export interface ClientOptions")));
    assert!(symbols.iter().any(|s| s.contains("export const API_URL")));
    assert!(symbols
        .iter()
        .any(|s| s.contains("export const maxRetries")));

    // Unexported const should NOT be included
    assert!(!symbols.iter().any(|s| s.contains("unexportedConst")));
    assert!(!symbols.iter().any(|s| s.contains("ignored")));
}
