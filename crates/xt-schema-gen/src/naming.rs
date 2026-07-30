use std::collections::BTreeMap;

pub(crate) fn rust_identifier(name: &str) -> String {
    let mut output = String::new();
    for (index, ch) in name.chars().enumerate() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            if ch.is_ascii_uppercase() {
                if index > 0 && !output.ends_with('_') {
                    output.push('_');
                }
                output.push(ch.to_ascii_lowercase());
            } else {
                output.push(ch);
            }
        } else if !output.ends_with('_') {
            output.push('_');
        }
    }
    let output = output.trim_matches('_').to_owned();
    let mut output = if output.is_empty() {
        "unnamed".to_owned()
    } else {
        output
    };
    if output
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_digit())
    {
        output.insert(0, '_');
    }
    if is_keyword(&output) {
        format!("r#{output}")
    } else {
        output
    }
}

pub(crate) fn module_identifier(name: &str) -> String {
    let mut output = String::new();
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
        } else if !output.ends_with('_') {
            output.push('_');
        }
    }
    let mut output = output.trim_matches('_').to_owned();
    if output.is_empty() {
        output.push_str("unnamed");
    }
    if output
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_digit())
    {
        output.insert(0, '_');
    }
    output
}

pub(crate) fn pascal_case(name: &str) -> String {
    let mut output = String::new();
    for part in name
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
    {
        let mut characters = part.chars();
        if let Some(first) = characters.next() {
            output.push(first.to_ascii_uppercase());
            for character in characters {
                output.push(character.to_ascii_lowercase());
            }
        }
    }
    if output.is_empty() {
        output.push_str("Unnamed");
    }
    if output
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_digit())
    {
        output.insert_str(0, "Schema");
    }
    output
}

pub(crate) fn field_identifiers<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    names
        .into_iter()
        .map(|name| {
            let base = rust_identifier(name);
            let count = counts.entry(base.clone()).or_default();
            *count += 1;
            if *count == 1 {
                base
            } else {
                format!("{base}_{}", *count)
            }
        })
        .collect()
}

fn is_keyword(value: &str) -> bool {
    matches!(
        value,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "dyn"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_stable_rust_names() {
        assert_eq!(rust_identifier("boundarySurface"), "boundary_surface");
        assert_eq!(rust_identifier("type"), "r#type");
        assert_eq!(pascal_case("BSPLINE_CURVE"), "BsplineCurve");
        assert_eq!(
            module_identifier("attributes_features"),
            "attributes_features"
        );
    }

    #[test]
    fn disambiguates_repeated_field_names() {
        assert_eq!(
            field_identifiers(["value", "value", "type"]),
            ["value", "value_2", "r#type"]
        );
    }
}
