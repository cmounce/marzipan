use std::{fs, path::Path};

use anyhow::{anyhow, bail, Result};

use super::{
    parse::{parse, Expr},
    scan::scan,
};

pub struct Context {
    file_loader: Box<dyn FileLoaderTrait>,
}

trait FileLoaderTrait {
    fn load(&self, path: &Path) -> Result<String>;
}

struct FileLoader;
struct MockFileLoader {
    content: String,
}

impl FileLoaderTrait for FileLoader {
    fn load(&self, path: &Path) -> Result<String> {
        fs::read_to_string(path).map_err(|e| anyhow!("Couldn't load file: {}", e))
    }
}

impl FileLoaderTrait for MockFileLoader {
    fn load(&self, _path: &Path) -> Result<String> {
        Ok(self.content.clone())
    }
}

impl Context {
    pub fn eval_program(&self, input: &str) -> Result<String> {
        let tokens = scan(input).0;
        let exprs = parse(tokens)?;
        let mut result: Vec<String> = vec![];
        for expr in exprs {
            match expr {
                Expr::ZztOop(s) => result.push(s),
                Expr::Macro(name, args) => match name.as_str() {
                    "include" => {
                        if args.len() != 1 {
                            bail!("wrong number of args for %include");
                        }
                        let filename = if let Expr::String(s) = args[0].as_ref() {
                            s
                        } else {
                            bail!("%include filename must be a string")
                        };

                        let mut content = self.file_loader.load(Path::new(filename))?;
                        content = content.replace("\r\n", "\n");
                        if content.ends_with("\n") {
                            content.pop();
                        }
                        result.push(content)
                    }
                    _ => bail!("Unknown macro: {:?}", name),
                },
                _ => {
                    bail!("Unexpected expr: {:?}", expr);
                }
            }
        }
        Ok(result.join(""))
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_debug_snapshot;

    use super::*;

    fn make_context(data: String) -> Context {
        Context {
            file_loader: Box::new(MockFileLoader { content: data }),
        }
    }

    #[test]
    fn include() {
        let program = format!("foo\n%include \"bb.txt\"\nquux");
        let file = "bar\nbaz\n";
        assert_debug_snapshot!(make_context(file.into()).eval_program(&program), @r###"
        Ok(
            "foo\nbar\nbaz\nquux",
        )
        "###);
    }

    #[test]
    fn include_windows() {
        let program = format!("%include \"foo.txt\"");
        let file = "foo\r\nbar";
        assert_debug_snapshot!(make_context(file.into()).eval_program(&program), @r###"
        Ok(
            "foo\nbar",
        )
        "###);
    }

    #[test]
    fn unknown_macro() {
        make_context("".into())
            .eval_program("%foo")
            .expect_err("Expected error: unknown macro");
        assert_debug_snapshot!(make_context("".into())
            .eval_program("%foo")
            .expect_err("Expected error: unknown macro"), @r###""Unknown macro: \"foo\"""###);
    }
}
