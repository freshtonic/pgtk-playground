//! A tree of an AST value for the AST panel.
//!
//! recursa has no reflection API, and its `TotalVisitor` sees nodes but not field names,
//! `Option`s, lists or primitive values. So the tree comes from the AST's pretty `Debug` output,
//! which has all of them.

use serde::Serialize;

/// One value of the AST.
#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Node {
    /// `Name { field: value, .. }`, or a tuple `Name(a, b)` with the fields `0`, `1`, ...
    Struct { name: String, fields: Vec<Field> },
    /// `Name(value)` or a bare `Name`: an enum variant, or a newtype or unit struct.
    Enum { variant: Variant },
    /// `None` or `Some(value)`.
    Option { value: Option<Box<Node>> },
    /// `[a, b, ..]`.
    List { items: Vec<Node> },
    /// A string, a number, a bool or other text.
    Primitive { value: Primitive },
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub _type: Node,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Variant {
    pub name: String,
    /// The value in the variant. `None` for a unit variant.
    #[serde(rename = "type")]
    pub _type: Option<Box<Node>>,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum Primitive {
    String(String),
    Bool(bool),
    Number(String),
    Other(String),
}

impl Node {
    /// Parses the output of `{:#?}`. Text that is not in the `Debug` form becomes
    /// [`Primitive::Other`].
    pub fn from_debug(text: &str) -> Node {
        let mut parser = Parser { s: text, i: 0 };
        parser.value()
    }
}

struct Parser<'a> {
    s: &'a str,
    i: usize,
}

impl Parser<'_> {
    fn rest(&self) -> &str {
        &self.s[self.i..]
    }

    fn skip_ws(&mut self) {
        let trimmed = self.rest().trim_start();
        self.i = self.s.len() - trimmed.len();
    }

    fn eat(&mut self, c: char) -> bool {
        self.skip_ws();
        if self.rest().starts_with(c) {
            self.i += c.len_utf8();
            true
        } else {
            false
        }
    }

    fn value(&mut self) -> Node {
        self.skip_ws();
        let Some(c) = self.rest().chars().next() else {
            return Node::Primitive {
                value: Primitive::Other(String::new()),
            };
        };
        if c == '[' {
            self.i += 1;
            return Node::List {
                items: self.values(']'),
            };
        }
        if c == '"' || c == '\'' {
            return Node::Primitive {
                value: Primitive::String(self.quoted(c)),
            };
        }
        if c.is_ascii_digit() || c == '-' {
            let end = self
                .rest()
                .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
                .unwrap_or(self.rest().len());
            let number = self.rest()[..end].to_owned();
            self.i += end;
            return Node::Primitive {
                value: Primitive::Number(number),
            };
        }
        let name = self.name();
        if name.is_empty() {
            // Text that the parser does not know: keep it to the end of the line.
            let end = self
                .rest()
                .find(['\n', ','])
                .unwrap_or(self.rest().len())
                .max(1);
            let other = self.rest()[..end].trim().to_owned();
            self.i += end;
            return Node::Primitive {
                value: Primitive::Other(other),
            };
        }
        match name.as_str() {
            "true" => {
                return Node::Primitive {
                    value: Primitive::Bool(true),
                };
            }
            "false" => {
                return Node::Primitive {
                    value: Primitive::Bool(false),
                };
            }
            "None" => return Node::Option { value: None },
            _ => {}
        }
        self.skip_ws();
        if self.eat('{') {
            let mut fields = Vec::new();
            loop {
                if self.eat('}') {
                    break;
                }
                let field = self.name();
                if field.is_empty() || !self.eat(':') {
                    // `..` of a non-exhaustive Debug, or text the parser does not know.
                    self.skip_to_close('}');
                    break;
                }
                fields.push(Field {
                    name: field,
                    _type: self.value(),
                });
                self.eat(',');
            }
            return Node::Struct { name, fields };
        }
        if self.eat('(') {
            let mut values = self.values(')');
            if name == "Some" && values.len() == 1 {
                return Node::Option {
                    value: values.pop().map(Box::new),
                };
            }
            if values.len() == 1 {
                let value = values.pop().map(Box::new);
                return Node::Enum {
                    variant: Variant { name, _type: value },
                };
            }
            let fields = values
                .into_iter()
                .enumerate()
                .map(|(i, v)| Field {
                    name: i.to_string(),
                    _type: v,
                })
                .collect();
            return Node::Struct { name, fields };
        }
        Node::Enum {
            variant: Variant { name, _type: None },
        }
    }

    fn values(&mut self, close: char) -> Vec<Node> {
        let mut values = Vec::new();
        loop {
            if self.eat(close) || self.rest().is_empty() {
                return values;
            }
            let before = self.i;
            values.push(self.value());
            self.eat(',');
            if self.i == before {
                self.i += self.rest().chars().next().map_or(0, char::len_utf8);
            }
        }
    }

    /// A Rust path with generics, for example `Handle<Relation>` or `ast::Statement`.
    fn name(&mut self) -> String {
        self.skip_ws();
        let mut depth = 0usize;
        let mut end = 0;
        let rest = self.rest();
        for (at, c) in rest.char_indices() {
            if at < end {
                continue;
            }
            match c {
                '<' => depth += 1,
                '>' if depth > 0 => depth -= 1,
                // A path separator. A single `:` ends a field name.
                ':' if rest[at..].starts_with("::") => {
                    end = at + 2;
                    continue;
                }
                c if c.is_alphanumeric() || c == '_' => {}
                ' ' | ',' | ':' if depth > 0 => {}
                _ => break,
            }
            end = at + c.len_utf8();
        }
        let name = self.rest()[..end].to_owned();
        self.i += end;
        name
    }

    fn quoted(&mut self, quote: char) -> String {
        let mut out = String::new();
        let mut chars = self.rest().char_indices().skip(1);
        let mut end = self.rest().len();
        while let Some((at, c)) = chars.next() {
            match c {
                '\\' => {
                    if let Some((_, e)) = chars.next() {
                        out.push(match e {
                            'n' => '\n',
                            't' => '\t',
                            'r' => '\r',
                            other => other,
                        });
                    }
                }
                c if c == quote => {
                    end = at + 1;
                    break;
                }
                c => out.push(c),
            }
        }
        self.i += end;
        out
    }

    fn skip_to_close(&mut self, close: char) {
        let end = self
            .rest()
            .find(close)
            .map_or(self.rest().len(), |at| at + 1);
        self.i += end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_structs_enums_options_lists_and_primitives() {
        let tree = Node::from_debug(
            "Select {\n    distinct: false,\n    limit: Some(\n        Int(\n            10,\n        ),\n    ),\n    items: [\n        Star,\n        Name(\n            \"a\\\"b\",\n        ),\n    ],\n    into: None,\n}",
        );
        let Node::Struct { name, fields } = tree else {
            panic!("{tree:?}")
        };
        assert_eq!(name, "Select");
        assert_eq!(fields.len(), 4);
        assert_eq!(
            fields[0]._type,
            Node::Primitive {
                value: Primitive::Bool(false)
            }
        );
        assert!(matches!(&fields[1]._type, Node::Option { value: Some(_) }));
        let Node::List { items } = &fields[2]._type else {
            panic!()
        };
        assert_eq!(items.len(), 2);
        assert_eq!(fields[3]._type, Node::Option { value: None });
    }
}

#[cfg(test)]
mod real {
    #[test]
    fn a_real_statement_has_no_other_text() {
        let document = pgtk_sql::document::parse_sql_recovering(
            "SELECT d.name, count(*) FROM departments d WHERE d.id = 1 AND d.name <> 'x' GROUP BY 1",
        )
        .unwrap();
        let text = format!("{:#?}", document.statements()[0]);
        let tree = serde_json::to_string(&super::Node::from_debug(&text)).unwrap();
        println!("{}", &text[..text.len().min(1500)]);
        assert!(!tree.contains("\"other\""), "{tree}");
    }
}
