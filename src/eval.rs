use crate::parser::{Node, Span, Spanned};
use std::collections::{HashMap, HashSet};
use std::io;
use std::str::Bytes;
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

    // Helps look up line numbers for Spans. Optional to make it easier to write
    // tests as not all nodes care about this.
    line_index: Option<LineIndex>,

    // A mapping from line number to a literal (int/float/str/adverb).
    // Used for ValueRef evaluation.
    line_to_literal: HashMap<usize, Node>,
}

static BUILT_INS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    let mut set = HashSet::new();
    set.insert("humanely");
    set.insert("multiply");
    set.insert("parsingly");
    set.insert("additionally");
    set.insert("visibly");
    // TODO: Add more.
    set
});

#[derive(Debug, PartialEq)]
pub struct EvalError {
    pub message: String,
    pub span: Span,
}

impl Evaluator {
    pub fn new(nodes: Vec<Spanned<Node>>, line_index: Option<LineIndex>) -> Self {
        Self {
            line_to_literal: collect_values(&nodes, &line_index),
            stack: Vec::new(),
            nodes: nodes,
            idents: HashMap::new(),
            line_index: line_index,
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
                    let ident = normalize_ident(name);
                    self.check_ident_clash(&ident, span)?;
                    self.idents.insert(ident.to_string(), i);
                }
                (Node::Chapter(name, _children), span) => {
                    let ident = normalize_ident(name);
                    self.check_ident_clash(&ident, span)?;
                    self.idents.insert(ident.to_string(), i);
                }
                (Node::IntLiteral(_), _span) => (),
                (Node::FloatLiteral(_), _span) => (),
                (Node::Adverb(_), _span) => (),
                (Node::Word, _span) => (),
                _ => {
                    unimplemented!("TODO {:?}", n);
                }
            }
        }

        // Now start execution starting with children of `story`.
        match story {
            Some(i) => match self.nodes[i].clone() {
                // TODO: Remove clone.
                (Node::Story(_, children), _) => {
                    for child in children.iter().rev() {
                        let was_return = self.eval_node(&child)?;
                        if was_return {
                            break;
                        }
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

    // Returns `true` when `FuncReturn` was evaluated. Caller should use this as a signal
    // appropriately.
    fn eval_node(&mut self, node: &Spanned<Node>) -> Result<bool, EvalError> {
        match node {
            (Node::FuncCall(ident, word_count), span) => {
                self.eval_func_call(ident, *word_count, span)?;
                Ok(false)
            }
            (Node::ValueRef(lines_below), span) => {
                self.eval_value_ref(*lines_below, span)?;
                Ok(false)
            }
            (Node::FuncReturn, _) => Ok(true),
            (Node::Word, _) => Ok(false),
            (Node::Adverb(_), _) => Ok(false),
            _ => {
                unimplemented!("TODO {:?}", node);
            }
        }
    }

    fn eval_func_call(
        &mut self,
        raw_ident: &String,
        word_count: usize,
        span: &Span,
    ) -> Result<(), EvalError> {
        // Process the ident to remove punctuation.
        let ident = normalize_ident(raw_ident);

        // Verify that this is a valid function call.
        let is_correct_words_long = ident.len() == word_count as usize;

        let focus_var_value = self.get_focus_var();
        let focus_letter_count = count_letters_in(focus_var_value, &ident);
        let is_correct_focus_count = word_count == focus_letter_count;
        if !is_correct_words_long && !is_correct_focus_count {
            return Err(EvalError {
                message: "Invalid func call, incorrect number of words given".to_string(),
                span: span.clone(),
            });
        }

        // Find the ident. Check built-ins first.
        for built_in in BUILT_INS.iter() {
            if is_anagram(&ident, built_in) {
                // We cannot allow the names to match.
                if ident == *built_in {
                    return Err(EvalError {
                        message: format!("Invalid func call, need an anagram of {}", ident),
                        span: span.clone(),
                    });
                }

                match *built_in {
                    "visibly" => {
                        let val = self.pop();
                        match val {
                            Some(Value::Float(v)) => println!("{}", v),
                            Some(Value::Integer(v)) => println!("{}", v),
                            Some(Value::Text(v)) => println!("{}", v),
                            None => {
                                return Err(EvalError {
                                    message: "Need value on stack for `visibly`".to_string(),
                                    span: span.clone(),
                                });
                            }
                        }
                        return Ok(());
                    }
                    "humanely" => {
                        let mut input = String::new();
                        io::stdin()
                            .read_line(&mut input)
                            .expect("Failed to read line");
                        self.push(Value::Text(input.trim_end().to_string()));
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
                            (Some(Value::Float(a_val)), Some(Value::Integer(b_val))) => {
                                self.push(Value::Float(a_val * b_val as f64));
                            }
                            (Some(Value::Integer(a_val)), Some(Value::Float(b_val))) => {
                                self.push(Value::Float(a_val as f64 * b_val));
                            }
                            (Some(Value::Float(a_val)), Some(Value::Float(b_val))) => {
                                self.push(Value::Float(a_val * b_val));
                            }
                            _ => {
                                unimplemented!("TODO");
                            }
                        }
                        return Ok(());
                    }
                    "additionally" => {
                        let a = self.pop();
                        let b = self.pop();
                        if a.is_none() || b.is_none() {
                            return Err(EvalError {
                                message: "Need two values on stack for `additionally`".to_string(),
                                span: span.clone(),
                            });
                        }

                        match (a, b) {
                            (Some(Value::Integer(a_val)), Some(Value::Integer(b_val))) => {
                                self.push(Value::Integer(a_val + b_val));
                            }
                            (Some(Value::Float(a_val)), Some(Value::Integer(b_val))) => {
                                self.push(Value::Float(a_val + b_val as f64));
                            }
                            (Some(Value::Integer(a_val)), Some(Value::Float(b_val))) => {
                                self.push(Value::Float(a_val as f64 + b_val));
                            }
                            (Some(Value::Float(a_val)), Some(Value::Float(b_val))) => {
                                self.push(Value::Float(a_val + b_val));
                            }
                            _ => {
                                unimplemented!("TODO");
                            }
                        }
                        return Ok(());
                    }
                    "parsingly" => {
                        let number_value = self.pop();

                        match number_value {
                            Some(Value::Text(str)) => {
                                let value = if let Ok(int_val) = str.parse::<i64>() {
                                    Value::Integer(int_val)
                                } else if let Ok(float_val) = str.parse::<f64>() {
                                    Value::Float(float_val)
                                } else {
                                    return Err(EvalError {
                                        message: format!("Couldn't parse '{}'", str),
                                        span: span.clone(),
                                    });
                                };

                                self.push(value);
                            }
                            Some(Value::Integer(_)) => {
                                self.push(number_value.unwrap());
                            }
                            Some(Value::Float(_)) => {
                                self.push(number_value.unwrap());
                            }
                            None => {
                                return Err(EvalError {
                                    message: "Need value on stack for `parsingly`".to_string(),
                                    span: span.clone(),
                                });
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
        for (ch_ident, index) in self.idents.clone() {
            if let (Node::Chapter(_, children), _) = self.nodes[index].clone() {
                if !is_anagram(&ident, &ch_ident) {
                    continue;
                }

                // We cannot allow the names to match.
                if ident == ch_ident {
                    return Err(EvalError {
                        message: format!("Invalid func call, need an anagram of {}", ident),
                        span: span.clone(),
                    });
                }

                // Loop through nodes, back to front.
                for child in children.iter().rev() {
                    let was_return = self.eval_node(&child)?;
                    if was_return {
                        break;
                    }
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

    fn eval_value_ref(&mut self, lines_below: usize, span: &Span) -> Result<(), EvalError> {
        let our_line = self
            .line_index
            .as_ref()
            .expect("Evaluator needs LineIndex")
            .get_line(span.start);
        let wanted_line = our_line + lines_below;
        let wanted_node = self.line_to_literal.get(&wanted_line);

        match wanted_node {
            Some(node) => {
                // Push onto the stack.
                let value = match node {
                    Node::FloatLiteral(f) => Value::Float(*f),
                    Node::IntLiteral(i) => Value::Integer(*i),
                    Node::Adverb(a) => {
                        // We look up the current focus variable, whatever char it is we count in the
                        // adverb. Then push that as the value.
                        let focus_var_value = self.get_focus_var();
                        let value = count_letters_in(focus_var_value, a) as i64;
                        Value::Integer(value)
                    }
                    _ => {
                        panic!("Unsupported Node being pushed onto stack");
                    }
                };
                self.stack.push(value);
                Ok(())
            }
            None => Err(EvalError {
                message: format!("No value {} lines below", lines_below),
                span: span.clone(),
            }),
        }
    }

    fn get_focus_var(&self) -> u8 {
        b'r' // TODO: Implement variables.
    }
}

// A LineIndex which ignores empty lines. Used for ValueRef evaluation.
pub struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(src: &str) -> Self {
        let mut line_starts = vec![0]; // First line begins at offset 0.

        let mut last_was_newline = false;
        for (i, c) in src.bytes().enumerate() {
            if c == b'\n' {
                if !last_was_newline {
                    line_starts.push(i + 1);
                    last_was_newline = true;
                }
            } else {
                last_was_newline = false;
            }
        }

        Self { line_starts }
    }

    fn get_line(&self, byte_offset: usize) -> usize {
        match self.line_starts.binary_search(&byte_offset) {
            Ok(line) => line + 1,
            Err(line) => line,
        }
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

fn count_letters_in(letter: u8, word: &str) -> usize {
    word.bytes().filter(|x| *x == letter).count()
}

fn collect_values(
    nodes: &Vec<Spanned<Node>>,
    line_index: &Option<LineIndex>,
) -> HashMap<usize, Node> {
    let mut line_to_literal = HashMap::new();
    for node in nodes {
        match node {
            (Node::FloatLiteral(f), span) => {
                let line = line_index
                    .as_ref()
                    .expect("Evaluator needs LineIndex")
                    .get_line(span.start);
                line_to_literal.insert(line, Node::FloatLiteral(*f));
            }
            (Node::IntLiteral(i), span) => {
                let line = line_index
                    .as_ref()
                    .expect("Evaluator needs LineIndex")
                    .get_line(span.start);
                line_to_literal.insert(line, Node::IntLiteral(*i));
            }
            (Node::Adverb(a), span) => {
                let line = line_index
                    .as_ref()
                    .expect("Evaluator needs LineIndex")
                    .get_line(span.start);
                line_to_literal.insert(line, Node::Adverb(a.to_string()));
            }
            (Node::Story(_, children), _) => {
                line_to_literal.extend(collect_values(children, line_index));
            }
            (Node::Chapter(_, children), _) => {
                line_to_literal.extend(collect_values(children, line_index));
            }
            (Node::FuncCall(_, _), _) => (),
            (Node::FuncReturn, _) => (),
            (Node::ValueRef(_), _) => (),
            (Node::Word, _) => (),
        }
    }

    return line_to_literal;
}

fn normalize_ident(ident: &str) -> String {
    ident
        .trim_matches(|c: char| c.is_ascii_punctuation())
        .to_ascii_lowercase()
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

        let mut evaluator = Evaluator::new(nodes, None /* LineIndex */);
        evaluator.push(Value::Integer(5));
        evaluator.push(Value::Integer(5));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(25)]));
    }

    #[test]
    fn test_builtin_func_call_must_be_anagram() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::FuncCall("multiply".to_string(), 0).spanned(0..0)],
            )
            .spanned(0..0),
        ];

        let mut evaluator = Evaluator::new(nodes, None /* LineIndex */);
        evaluator.push(Value::Integer(5));
        evaluator.push(Value::Integer(5));
        let res = evaluator.eval_script();
        assert_eq!(
            res,
            Err(EvalError {
                message: "Invalid func call, need an anagram of multiply".to_string(),
                span: 0..0
            })
        );
    }

    #[test]
    fn test_custom_func_call() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::FuncCall("styleit".to_string(), 0).spanned(0..0)],
            )
            .spanned(0..0),
            Node::Chapter(
                "testily".to_string(),
                vec![Node::FuncCall("miltypul".to_string(), 0).spanned(0..0)],
            )
            .spanned(0..0),
        ];

        let mut evaluator = Evaluator::new(nodes, None /* LineIndex */);
        evaluator.push(Value::Integer(5));
        evaluator.push(Value::Integer(5));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(25)]));
    }

    #[test]
    fn test_custom_func_call_must_be_anagram() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::FuncCall("testily".to_string(), 0).spanned(0..0)],
            )
            .spanned(0..0),
            Node::Chapter("testily".to_string(), vec![]).spanned(0..0),
        ];

        let mut evaluator = Evaluator::new(nodes, None /* LineIndex */);
        let res = evaluator.eval_script();
        assert_eq!(
            res,
            Err(EvalError {
                message: "Invalid func call, need an anagram of testily".to_string(),
                span: 0..0
            })
        );
    }

    // Verifies that whatever is on the stack at the end of function
    // evaluation is cleared, with only the top of the stock preserved.
    #[test]
    fn test_func_call_only_one_value_after_return() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::FuncCall("tesitly".to_string(), 0).spanned(0..0)],
            )
            .spanned(0..0),
            Node::Chapter("testily".to_string(), vec![]).spanned(0..0),
        ];

        let mut evaluator = Evaluator::new(nodes, None /* LineIndex */);
        evaluator.push(Value::Integer(5));
        evaluator.push(Value::Integer(10));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(10)]));
    }

    #[test]
    fn test_value_push() {
        let src = "This story starts testly.\nThere is something with a value 2 lines below.\nLine one\nMy secret value is 42";
        let index = LineIndex::new(src);
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::ValueRef(2).spanned(26..72)],
            )
            .spanned(0..25),
            Node::IntLiteral(42).spanned(101..103),
        ];

        let mut evaluator = Evaluator::new(nodes, Some(index));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(42)]));
    }

    #[test]
    fn test_value_push_float() {
        // Also testing repeated newlines here. These should be ignored.
        let src = "This story starts testly.\n\n\n\n\nThere is something with a value 2 lines below.\n\nLine one\n\nMy secret value is 4.2";
        let index = LineIndex::new(src);
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::ValueRef(2).spanned(30..76)],
            )
            .spanned(0..25),
            Node::FloatLiteral(4.2).spanned(107..109),
        ];

        let mut evaluator = Evaluator::new(nodes, Some(index));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Float(4.2)]));
    }

    #[test]
    fn test_value_push_adverb() {
        let src = "This story starts testly.\nThere is something with a value 1 line below.\nThe story was wirrrrrrinringly bad";
        let index = LineIndex::new(src);
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::ValueRef(1).spanned(26..71)],
            )
            .spanned(0..25),
            Node::Adverb("wirrrrrrinringly".to_string()).spanned(86..102),
        ];

        let mut evaluator = Evaluator::new(nodes, Some(index));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(7)]));
    }

    #[test]
    fn test_func_return() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::FuncCall("tesitly".to_string(), 0).spanned(0..0)],
            )
            .spanned(0..0),
            Node::Chapter("testily".to_string(), vec![Node::FuncReturn.spanned(0..0)])
                .spanned(0..0),
        ];

        let mut evaluator = Evaluator::new(nodes, None /* LineIndex */);
        evaluator.push(Value::Integer(1));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(1)]));
    }

    #[test]
    fn test_func_call_word_count() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::FuncCall("frigidly".to_string(), 15).spanned(0..0)],
            )
            .spanned(0..0),
            Node::Chapter(
                "frigidly".to_string(),
                vec![Node::FuncCall("miltypul".to_string(), 0).spanned(0..0)],
            )
            .spanned(0..0),
        ];

        let mut evaluator = Evaluator::new(nodes, None /* LineIndex */);
        evaluator.push(Value::Integer(1));
        evaluator.push(Value::Integer(5));
        let res = evaluator.eval_script();
        assert_eq!(
            res,
            Err(EvalError {
                message: "Invalid func call, incorrect number of words given".to_string(),
                span: 0..0
            })
        );
    }
}
