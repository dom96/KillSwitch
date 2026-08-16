use crate::parser::{Node, Span, Spanned};
use std::collections::{HashMap, HashSet};
use std::io;
use std::rc::Rc;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Float(f64),
    Text(String),
}

pub struct Evaluator {
    // Used during evaluation.
    stack: Vec<Value>,

    // The nodes that this evaluator is evaluating.
    nodes: Vec<Spanned<Node>>,

    // A list of named nodes: chapters, stories, etc.
    // Because of Rust's lifetimes, we use an index to refer to the Node,
    // rather than a reference to the Node.
    // TODO: Right now this only allows us to refer to a top-level node.
    idents: HashMap<String, usize>,
}

static BUILT_INS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    let mut set = HashSet::new();
    set.insert("humanely");
    set.insert("multiply");
    // TODO: Add more.
    set
});

#[derive(Debug, PartialEq)]
pub struct EvalError {
    message: String,
    span: Span,
}

impl Evaluator {
    pub fn new(nodes: Vec<Spanned<Node>>) -> Self {
        Self {
            stack: Vec::new(),
            nodes: nodes,
            idents: HashMap::new(),
        }
    }

    fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    fn pop(&mut self) -> Option<Value> {
        self.stack.pop()
    }

    pub fn eval_script(&mut self) -> Result<Vec<Value>, EvalError> {
        // Verification to ensure we aren't accidentally calling this function twice.
        assert_eq!(self.idents.len(), 0);

        // Pre-process nodes to find referencable elements. Like chapters, stories, values.
        //
        // An (index) reference to the Node containing the entry point of the
        // script.
        let mut story: Option<usize> = None;

        for (i, n) in self.nodes.iter().enumerate() {
            match n {
                (Node::Story(name, _children), span) => {
                    story = Some(i);
                    self.check_ident_clash(name, span)?;
                    self.idents.insert(name.clone(), i);
                }
                (Node::Chapter(name, _children), span) => {
                    self.check_ident_clash(name, span)?;
                    self.idents.insert(name.clone(), i);
                }
                (Node::IntLiteral(_), _span) => (),
                _ => {
                    unimplemented!("TODO");
                }
            }
        }

        // Now start execution starting with children of `story`.
        match story {
            Some(i) => match self.nodes[i].clone() {
                // TODO: Remove clone.
                (Node::Story(_, children), _) => {
                    for child in children {
                        self.eval_node(&child)?
                    }
                }
                _ => panic!("We should never not get a Story here."),
            },
            _ => {
                return Err(EvalError {
                    message: "Evaluated script needs an entrypoint".to_string(),
                    span: (0..0),
                });
            }
        }

        return Ok(self.stack.clone());
    }

    fn eval_node(&mut self, node: &Spanned<Node>) -> Result<(), EvalError> {
        match node {
            (Node::FuncCall(ident, _word_count), span) => self.eval_func_call(ident, span),
            (Node::ValueRef(lines_below), span) => self.eval_value_ref(*lines_below),
            _ => {
                unimplemented!("TODO");
            }
        }
    }

    fn eval_func_call(&mut self, ident: &String, span: &Span) -> Result<(), EvalError> {
        // Find the ident. Check built-ins first.
        for built_in in BUILT_INS.iter() {
            if is_anagram(ident, built_in) {
                match *built_in {
                    "humanely" => {
                        let mut input = String::new();
                        io::stdin()
                            .read_line(&mut input)
                            .expect("Failed to read line");

                        self.push(Value::Text(input));
                        return Ok(());
                    }
                    "multiply" => {
                        let a = self.pop();
                        let b = self.pop();
                        if a.is_none() || b.is_none() {
                            return Err(EvalError {
                                message: "Need two values on stack for `multiply`".to_string(),
                                span: span.clone(),
                            });
                        }

                        match (a, b) {
                            (Some(Value::Integer(a_val)), Some(Value::Integer(b_val))) => {
                                self.push(Value::Integer(a_val * b_val));
                            }
                            _ => {
                                unimplemented!("TODO");
                            }
                        }
                        return Ok(());
                    }
                    &_ => unimplemented!(),
                }
            }
        }

        // If none of the built-ins match, then find a chapter with the name.
        //
        // TODO: Avoid the `clone` calls.
        for (_, index) in self.idents.clone() {
            if let (Node::Chapter(ch_ident, children), _) = self.nodes[index].clone() {
                if !is_anagram(ident, &ch_ident) {
                    continue;
                }

                // Loop through nodes, back to front.
                for child in children.iter().rev() {
                    self.eval_node(&child)?;
                }

                // Keep only the top value on the stack.
                let top = self.stack.pop();
                self.stack.clear();
                if let Some(t) = top {
                    self.stack.push(t);
                }

                return Ok(());
            }
        }

        return Err(EvalError {
            message: format!("Could not find chapter {}", ident),
            span: span.clone(),
        });
    }

    fn check_ident_clash(&self, name: &str, span: &Span) -> Result<(), EvalError> {
        if BUILT_INS.contains(name) {
            return Err(EvalError {
                message: format!("Duplicate identifier {}", &name),
                span: span.clone(),
            });
        }

        if self.idents.contains_key(name) {
            return Err(EvalError {
                message: format!("Duplicate identifier {}", &name),
                span: span.clone(),
            });
        }

        return Ok(());
    }

    fn eval_value_ref(&self, lines_below: usize) -> Result<(), EvalError> {
        // TODO
        return Ok(());
    }
}

// Determines whether `a` is an anagram of `b`.
// is_anagram("hunyamel", "humanely")
fn is_anagram(a: &str, b: &str) -> bool {
    let mut counts_a: HashMap<char, i32> = HashMap::new();
    let mut counts_b: HashMap<char, i32> = HashMap::new();

    for c in a.chars() {
        *counts_a.entry(c).or_insert(0) += 1;
    }

    for c in b.chars() {
        *counts_b.entry(c).or_insert(0) += 1;
    }

    return counts_a == counts_b;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_func_call() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::FuncCall("miltypul".to_string(), 0).spanned(0..0)],
            )
            .spanned(0..0),
        ];

        let mut evaluator = Evaluator::new(nodes);
        evaluator.push(Value::Integer(5));
        evaluator.push(Value::Integer(5));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(25)]));
    }

    #[test]
    fn test_custom_func_call() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::FuncCall("testily".to_string(), 0).spanned(0..0)],
            )
            .spanned(0..0),
            Node::Chapter(
                "testily".to_string(),
                vec![Node::FuncCall("miltypul".to_string(), 0).spanned(0..0)],
            )
            .spanned(0..0),
        ];

        let mut evaluator = Evaluator::new(nodes);
        evaluator.push(Value::Integer(5));
        evaluator.push(Value::Integer(5));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(25)]));
    }

    // Verifies that whatever is on the stack at the end of function
    // evaluation is cleared, with only the top of the stock preserved.
    #[test]
    fn test_func_call_only_one_value_after_return() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::FuncCall("testily".to_string(), 0).spanned(0..0)],
            )
            .spanned(0..0),
            Node::Chapter("testily".to_string(), vec![]).spanned(0..0),
        ];

        let mut evaluator = Evaluator::new(nodes);
        evaluator.push(Value::Integer(5));
        evaluator.push(Value::Integer(10));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(10)]));
    }

    #[test]
    fn test_value_push() {
        let nodes = vec![
            Node::Story("testly".to_string(), vec![Node::ValueRef(2).spanned(0..0)]).spanned(0..0),
            Node::IntLiteral(42).spanned(0..0),
        ];

        let mut evaluator = Evaluator::new(nodes);
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(42)]));
    }
}
