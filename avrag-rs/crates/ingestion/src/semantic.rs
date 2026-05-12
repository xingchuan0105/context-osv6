use syn::{Item, parse_str};

pub fn extract_rust_functions(code: &str) -> Vec<String> {
    let file = match parse_str::<syn::File>(code) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let mut functions = Vec::new();
    for item in file.items {
        if let Item::Fn(f) = item {
            functions.push(f.sig.ident.to_string());
        }
    }
    functions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_functions() {
        let code = r#"
            fn hello() {}
            pub async fn world() -> i32 { 42 }
        "#;
        let funcs = extract_rust_functions(code);
        assert_eq!(funcs, vec!["hello", "world"]);
    }
}
