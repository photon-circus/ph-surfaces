//! Text normalization and syntax-aware Rust source policy checks.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use proc_macro2::{TokenStream, TokenTree};
use syn::visit::{self, Visit};
use syn::{
    Attribute, ExprUnsafe, Item, ItemForeignMod, ItemImpl, ItemTrait, Lit, LitFloat, Macro,
    Signature,
};
use walkdir::WalkDir;

use crate::config::SourcePolicy;

pub fn read_text(path: &Path) -> io::Result<String> {
    Ok(String::from_utf8_lossy(&fs::read(path)?).replace('\r', ""))
}

pub fn rust_sources(dir: &Path) -> io::Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut found = Vec::new();
    for entry in WalkDir::new(dir).follow_links(false) {
        let entry = entry.map_err(io::Error::other)?;
        if entry.file_type().is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "rs")
        {
            found.push(entry.into_path());
        }
    }
    found.sort();
    Ok(found)
}

/// Count physical lines of implementation in a Rust source file.
///
/// A trailing `#[cfg(test)]` region is excluded, matching the integer-only
/// scanner: test items live at the file tail, so the first file-level
/// `#[cfg(test)]` starts the excluded suffix. Nested test attributes inside
/// already-excluded modules do not get a second look.
pub fn implementation_line_count(relative: &str, source: &str) -> Result<usize, String> {
    let file = syn::parse_file(source).map_err(|error| error.to_string())?;
    let mut findings = Vec::new();
    validate_test_tail(relative, &file.items, &mut findings);
    if !findings.is_empty() {
        return Err(findings.join("\n"));
    }
    let total = source.lines().count();
    match file_level_cfg_test_line(&file) {
        Some(tail) => Ok(tail.saturating_sub(1)),
        None => Ok(total),
    }
}

/// 1-based source line where the first file-level `#[cfg(test)]` item starts,
/// read from the parsed tree: a raw text scan can match the attribute text
/// inside a multiline string literal, or miss a differently spaced spelling.
fn file_level_cfg_test_line(file: &syn::File) -> Option<usize> {
    use syn::spanned::Spanned as _;
    file.items
        .iter()
        .filter(|item| has_cfg_test(item_attrs(item)))
        .map(|item| item.span().start().line)
        .min()
}

#[derive(Clone, Copy)]
pub enum Scan {
    AllCode,
    Runtime,
    Examples,
    FeatureCfg,
}

pub fn source_findings(
    root: &Path,
    dirs: &[String],
    policy: &SourcePolicy,
    scan: Scan,
) -> Result<Vec<String>, String> {
    let mut findings = Vec::new();
    for dir in dirs {
        for path in rust_sources(&root.join(dir)).map_err(|error| error.to_string())? {
            let source = read_text(&path).map_err(|error| error.to_string())?;
            let mut file = syn::parse_file(&source)
                .map_err(|error| format!("{} is not valid Rust: {error}", path.display()))?;
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");

            if matches!(scan, Scan::Runtime) {
                validate_test_tail(&relative, &file.items, &mut findings);
                file.items.retain(|item| !has_cfg_test(item_attrs(item)));
            }

            let allow_i64 = relative == policy.arithmetic_kernel;
            let mut analyzer = Analyzer {
                relative: &relative,
                policy,
                scan,
                allow_i64,
                findings: &mut findings,
            };
            analyzer.visit_file(&file);
        }
    }
    findings.sort();
    findings.dedup();
    Ok(findings)
}

fn validate_test_tail(relative: &str, items: &[Item], findings: &mut Vec<String>) {
    let mut in_tail = false;
    for item in items {
        let is_test = has_cfg_test(item_attrs(item));
        if is_test {
            in_tail = true;
        } else if in_tail {
            findings.push(format!(
                "{relative}: runtime item follows a #[cfg(test)] item; keep tests at the file tail"
            ));
            return;
        }
    }
}

fn item_attrs(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        Item::Verbatim(_) => &[],
        _ => &[],
    }
}

fn has_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| match &attr.meta {
        syn::Meta::List(list) if attr.path().is_ident("cfg") => {
            let mut tokens = list.tokens.clone().into_iter();
            matches!(tokens.next(), Some(TokenTree::Ident(ident)) if ident == "test")
                && tokens.next().is_none()
        }
        _ => false,
    })
}

fn meta_tokens(attr: &Attribute) -> TokenStream {
    match &attr.meta {
        syn::Meta::Path(path) => path
            .segments
            .iter()
            .map(|segment| TokenTree::Ident(segment.ident.clone()))
            .collect(),
        syn::Meta::List(list) => list.tokens.clone(),
        syn::Meta::NameValue(_) => TokenStream::new(),
    }
}

fn token_has_ident(tokens: TokenStream, wanted: &str) -> bool {
    tokens.into_iter().any(|token| match token {
        TokenTree::Ident(ident) => ident == wanted,
        TokenTree::Group(group) => token_has_ident(group.stream(), wanted),
        _ => false,
    })
}

struct Analyzer<'a> {
    relative: &'a str,
    policy: &'a SourcePolicy,
    scan: Scan,
    allow_i64: bool,
    findings: &'a mut Vec<String>,
}

impl Analyzer<'_> {
    fn finding(&mut self, message: impl AsRef<str>) {
        if self.findings.len() < 20 {
            self.findings
                .push(format!("{}: {}", self.relative, message.as_ref()));
        }
    }

    fn inspect_ident(&mut self, name: &str) {
        match self.scan {
            Scan::AllCode => {
                if matches!(name, "f32" | "f64") {
                    self.finding("code names a floating-point type");
                }
                if name == "ph_curves" {
                    self.finding("code references ph-curves");
                }
            }
            Scan::Runtime => {
                if matches!(name, "alloc" | "std") {
                    self.finding("runtime code reaches for alloc or std");
                }
                if matches!(name, "i128" | "u128") {
                    self.finding("runtime code uses a 128-bit integer");
                }
                if !self.allow_i64 && matches!(name, "i64" | "u64") {
                    self.finding("64-bit arithmetic appears outside the configured kernel");
                }
            }
            Scan::Examples => {
                if matches!(name, "alloc" | "std")
                    || self
                        .policy
                        .forbidden_example_types
                        .iter()
                        .any(|item| item == name)
                {
                    self.finding("example uses a host or allocating path/type");
                }
            }
            Scan::FeatureCfg => {}
        }
    }

    fn inspect_tokens(&mut self, tokens: TokenStream) {
        for token in tokens {
            match token {
                TokenTree::Ident(ident) => self.inspect_ident(&ident.to_string()),
                TokenTree::Literal(literal) => {
                    if let Ok(literal) = syn::parse_str::<Lit>(&literal.to_string()) {
                        match literal {
                            Lit::Float(_) if matches!(self.scan, Scan::AllCode) => {
                                self.finding("code contains a floating-point literal");
                            }
                            Lit::Int(integer) if matches!(self.scan, Scan::Runtime) => {
                                self.inspect_ident(integer.suffix());
                            }
                            _ => {}
                        }
                    }
                }
                TokenTree::Group(group) => self.inspect_tokens(group.stream()),
                _ => {}
            }
        }
    }
}

impl<'ast> Visit<'ast> for Analyzer<'_> {
    fn visit_attribute(&mut self, attr: &'ast Attribute) {
        if matches!(self.scan, Scan::FeatureCfg)
            && (attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr"))
            && token_has_ident(meta_tokens(attr), "feature")
        {
            self.finding("a cfg names a feature");
        }
        visit::visit_attribute(self, attr);
    }

    fn visit_ident(&mut self, ident: &'ast proc_macro2::Ident) {
        self.inspect_ident(&ident.to_string());
    }

    fn visit_lit_float(&mut self, _: &'ast LitFloat) {
        if matches!(self.scan, Scan::AllCode) {
            self.finding("code contains a floating-point literal");
        }
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        if matches!(self.scan, Scan::Examples)
            && mac.path.segments.last().is_some_and(|segment| {
                self.policy
                    .forbidden_example_macros
                    .iter()
                    .any(|name| segment.ident == name)
            })
        {
            self.finding("example invokes a forbidden host/allocating macro");
        }
        self.inspect_tokens(mac.tokens.clone());
        visit::visit_macro(self, mac);
    }

    fn visit_expr_unsafe(&mut self, node: &'ast ExprUnsafe) {
        if matches!(self.scan, Scan::Examples | Scan::Runtime) {
            self.finding("code uses unsafe");
        }
        visit::visit_expr_unsafe(self, node);
    }

    fn visit_signature(&mut self, node: &'ast Signature) {
        if matches!(node.safety, syn::Safety::Unsafe(_))
            && matches!(self.scan, Scan::Examples | Scan::Runtime)
        {
            self.finding("code declares an unsafe function");
        }
        visit::visit_signature(self, node);
    }

    fn visit_item_foreign_mod(&mut self, node: &'ast ItemForeignMod) {
        if matches!(self.scan, Scan::Examples | Scan::Runtime) {
            self.finding("code declares an unsafe foreign block");
        }
        visit::visit_item_foreign_mod(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        if node.unsafety.is_some() && matches!(self.scan, Scan::Examples | Scan::Runtime) {
            self.finding("code declares an unsafe impl");
        }
        visit::visit_item_impl(self, node);
    }

    fn visit_item_trait(&mut self, node: &'ast ItemTrait) {
        if node.unsafety.is_some() && matches!(self.scan, Scan::Examples | Scan::Runtime) {
            self.finding("code declares an unsafe trait");
        }
        visit::visit_item_trait(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macro_token_scanning_distinguishes_float_and_integer_literals() {
        let file =
            syn::parse_file("fn f() { assert_eq!(1..2, 0xf32); assert_eq!(1.5, 2.); }").unwrap();
        let policy: SourcePolicy = ron::from_str(
            "(runtime_roots:[],oracle_roots:[],example_roots:[],arithmetic_kernel:\"x\",forbidden_example_types:[\"Vec\"],forbidden_example_macros:[\"println\"],dependency_manifests:[\"Cargo.toml\"])",
        )
        .unwrap();
        let mut findings = Vec::new();
        Analyzer {
            relative: "fixture.rs",
            policy: &policy,
            scan: Scan::AllCode,
            allow_i64: false,
            findings: &mut findings,
        }
        .visit_file(&file);
        assert_eq!(findings.len(), 2);
    }

    #[test]
    fn cfg_test_tails_are_excluded_from_the_implementation_count() {
        let source = "fn impl_fn() {}\n\n#[cfg(test)]\nmod tests {\n    fn t() {}\n}\n";
        assert_eq!(implementation_line_count("fixture.rs", source).unwrap(), 2);

        let no_tests = "fn only_impl() {}\n";
        assert_eq!(
            implementation_line_count("fixture.rs", no_tests).unwrap(),
            1
        );

        let indented = "fn impl_fn() {\n    #[cfg(test)]\n    fn nested() {}\n}\n";
        assert_eq!(
            implementation_line_count("fixture.rs", indented).unwrap(),
            4
        );
    }

    #[test]
    fn cfg_test_text_inside_a_string_literal_does_not_end_the_count() {
        let source = "const S: &str = \"\n#[cfg(test)]\nnot code\n\";\nfn impl_fn() {}\n\n#[cfg(test)]\nmod tests {}\n";
        assert_eq!(implementation_line_count("fixture.rs", source).unwrap(), 6);
    }

    #[test]
    fn a_spaced_cfg_test_attribute_still_ends_the_count() {
        let source = "fn impl_fn() {}\n\n#[cfg( test )]\nmod tests {}\n";
        assert_eq!(implementation_line_count("fixture.rs", source).unwrap(), 2);
    }

    #[test]
    fn a_runtime_item_after_cfg_test_fails_the_implementation_count() {
        let source = "#[cfg(test)]\nmod tests {}\nfn leaked() {}\n";
        let error = implementation_line_count("fixture.rs", source).unwrap_err();
        assert!(
            error.contains("runtime item follows a #[cfg(test)] item"),
            "{error}"
        );
    }
}
