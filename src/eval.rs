use crate::parser::{Node, Span, Spanned, Unspanned};
use ariadne::{Color, Label, Report, ReportKind, Source};
use rand::prelude::IndexedRandom;
use rand::rng;
use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Float(f64),
    Text(String),
}

#[derive(Debug)]
pub struct VariableStore {
    store: HashMap<String, (String, Value)>,
}

impl VariableStore {
    pub fn new() -> Self {
        Self {
            store: HashMap::new(),
        }
    }

    pub fn insert(&mut self, name: &str, value: Value) {
        self.store
            .insert(sort_ident(normalize_ident(name)), (name.to_owned(), value));
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.store
            .get(&sort_ident(normalize_ident(name)))
            .map(|v| &v.1)
    }
}

pub struct Evaluator<'a> {
    // Used during evaluation.
    stack: Vec<Value>,

    // The nodes that this evaluator is evaluating.
    nodes: Vec<Spanned<Node>>,

    // A list of named nodes: chapters, stories, etc.
    // Because of Rust's lifetimes, we use an index to refer to the Node,
    // rather than a reference to the Node.
    idents: HashMap<String, Spanned<Node>>,

    // Helps look up line numbers for Spans. Optional to make it easier to write
    // tests as not all nodes care about this.
    line_index: Option<LineIndex<'a>>,

    // A mapping from line number to a literal (int/float/str/adverb).
    // Used for ValueRef evaluation.
    line_to_literal: HashMap<usize, Vec<Node>>,

    // Stack traces! We track FuncCalls here so we can print a nice stack trace
    // if we have too many nested calls.
    nested_func_calls: Vec<(String, Span)>,
}

static BUILT_INS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    let mut set = HashSet::new();
    set.insert("humanely");
    set.insert("multiply");
    set.insert("parsingly");
    set.insert("additionally");
    set.insert("visibly");
    set.insert("deductively");
    set.insert("divisibly");
    set.insert("beyond");
    set.insert("debuggably");
    set.insert("equally");
    set.insert("alternatively");
    set.insert("conjointly");
    set.insert("focally");
    set.insert("modularly");
    // TODO: Add more.
    set
});

#[derive(Debug, PartialEq)]
pub struct EvalError {
    pub message: String,
    pub span: Span,
}

impl<'a> Evaluator<'a> {
    pub fn new(nodes: Vec<Spanned<Node>>, line_index: Option<LineIndex<'a>>) -> Self {
        Self {
            line_to_literal: collect_values(&nodes, &line_index),
            stack: Vec::new(),
            nodes: nodes,
            idents: HashMap::new(),
            line_index: line_index,
            nested_func_calls: vec![],
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
        let story: Option<usize> = collect_idents(&mut self.idents, &self.nodes)?;

        // Now start execution starting with children of `story`.
        match story {
            Some(i) => match self.nodes[i].clone() {
                // TODO: Remove clone.
                (Node::Story(_, children), _) => {
                    let mut variable_store = VariableStore::new();

                    for child in children.iter().rev() {
                        let was_return = self.eval_node(&child, &mut variable_store)?;
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
    fn eval_node(
        &mut self,
        node: &Spanned<Node>,
        variable_store: &mut VariableStore,
    ) -> Result<bool, EvalError> {
        match node {
            (Node::FuncCall(ident, nodes), span) => {
                self.eval_func_call(ident, count_words(nodes), variable_store, span)?;
                Ok(false)
            }
            (Node::ValueRef(lines_below), span) => {
                self.eval_value_ref(*lines_below, variable_store, span)?;
                Ok(false)
            }
            (Node::FuncReturn, _) => {
                let val = self.stack.pop();

                match val {
                    Some(Value::Integer(1)) => Ok(true),
                    _ => Ok(false),
                }
            }
            (Node::Word(_), _) => Ok(false),
            (Node::Adverb(_), _) => Ok(false),
            (Node::FloatLiteral(_), _) => Ok(false),
            (Node::IntLiteral(_), _) => Ok(false),
            (Node::StringLiteral(_), _) => Ok(false),
            (Node::VariableAssign(ident), span) => {
                self.eval_var_assign(ident, variable_store, span)?;
                Ok(false)
            }
            (Node::VariableRead(ident, _), span) => {
                self.eval_var_read(ident, variable_store, span)?;
                Ok(false)
            }
            (Node::Hack(a, b), span) => {
                self.eval_hack(a, b, variable_store, span)?;
                Ok(false)
            }
            (Node::Chapter(_, _), _) => {
                // Nested chapters don't need to be evaluated.
                Ok(false)
            }
            (Node::FocusChange(t), span) => {
                self.eval_focus(*t, variable_store, span)?;
                Ok(false)
            }
            _ => {
                unimplemented!("TODO {:?}", node);
            }
        }
    }

    fn eval_func_call(
        &mut self,
        raw_ident: &String,
        word_count: usize,
        variable_store: &VariableStore,
        span: &Span,
    ) -> Result<(), EvalError> {
        // Handle infinite loops.
        // For now just restrict nested loops.
        // TODO: In future we won't be able to do this, as it will limit loops too much. Print stack on Ctrl+C instead.
        // TODO: Also print the stacks here nicely.
        if self.nested_func_calls.len() > 100 {
            println!("{:?}", self.nested_func_calls);
            return Err(EvalError {
                message: "Infinite loop detected".to_string(),
                span: span.clone(),
            });
        }

        // Track the function calls for stack traces.
        self.nested_func_calls
            .push((raw_ident.to_owned(), span.clone()));
        let res = self.eval_func_call_impl(raw_ident, word_count, variable_store, span);
        self.nested_func_calls.pop();
        res
    }

    fn eval_func_call_impl(
        &mut self,
        raw_ident: &String,
        word_count: usize,
        variable_store: &VariableStore,
        span: &Span,
    ) -> Result<(), EvalError> {
        // TODO: We can make func lookup faster here, by using the same trick as
        // for variable reads.

        // Process the ident to remove punctuation.
        let ident = normalize_ident(raw_ident);

        // Verify that this is a valid function call.
        let is_correct_words_long = ident.len() == word_count as usize;

        let focus_var_value = self.get_focus_var(variable_store);
        let focus_letter_count = count_letters_in(focus_var_value, &ident);
        let is_correct_focus_count = word_count == focus_letter_count;
        let is_debuggably = ident == normalize_ident("debuggably");
        if !is_correct_words_long && !is_correct_focus_count && !is_debuggably {
            return Err(EvalError {
                message: format!(
                    "Invalid func call, incorrect number of words given for `{}`, got {} wanted {} or {}",
                    raw_ident,
                    word_count,
                    focus_letter_count,
                    ident.len()
                ),
                span: span.clone(),
            });
        }

        let is_flat_adverb = !raw_ident.ends_with("ly");

        // Find the ident. Check built-ins first.
        for built_in in BUILT_INS.iter() {
            if is_anagram(&ident, built_in) {
                // We cannot allow the names to match.
                if ident == *built_in && !is_flat_adverb && !is_debuggably {
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
                    "focally" => {
                        let val = self.pop();
                        match val {
                            Some(Value::Integer(v)) => {
                                if (0..127).contains(&v) {
                                    self.push(Value::Text((v as u8 as char).into()));
                                } else {
                                    return Err(EvalError {
                                        message: format!("Non-ASCII integer on stack, got {}", v),
                                        span: span.clone(),
                                    });
                                }
                            }
                            _ => {
                                return Err(EvalError {
                                    message: "Need integer value on stack for `focally`"
                                        .to_string(),
                                    span: span.clone(),
                                });
                            }
                        }
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
                    "deductively" => {
                        let b = self.pop();
                        let a = self.pop();
                        if a.is_none() || b.is_none() {
                            return Err(EvalError {
                                message: "Need two values on stack for `deductively`".to_string(),
                                span: span.clone(),
                            });
                        }

                        match (a, b) {
                            (Some(Value::Integer(a_val)), Some(Value::Integer(b_val))) => {
                                self.push(Value::Integer(a_val - b_val));
                            }
                            (Some(Value::Float(a_val)), Some(Value::Integer(b_val))) => {
                                self.push(Value::Float(a_val - b_val as f64));
                            }
                            (Some(Value::Integer(a_val)), Some(Value::Float(b_val))) => {
                                self.push(Value::Float(a_val as f64 - b_val));
                            }
                            (Some(Value::Float(a_val)), Some(Value::Float(b_val))) => {
                                self.push(Value::Float(a_val - b_val));
                            }
                            _ => {
                                unimplemented!("TODO");
                            }
                        }
                        return Ok(());
                    }
                    "divisibly" => {
                        let b = self.pop();
                        let a = self.pop();
                        if a.is_none() || b.is_none() {
                            return Err(EvalError {
                                message: "Need two values on stack for `divisibly`".to_string(),
                                span: span.clone(),
                            });
                        }

                        match (a, b) {
                            (Some(Value::Integer(a_val)), Some(Value::Integer(b_val))) => {
                                self.push(Value::Float(a_val as f64 / b_val as f64));
                            }
                            (Some(Value::Float(a_val)), Some(Value::Integer(b_val))) => {
                                self.push(Value::Float(a_val / b_val as f64));
                            }
                            (Some(Value::Integer(a_val)), Some(Value::Float(b_val))) => {
                                self.push(Value::Float(a_val as f64 / b_val));
                            }
                            (Some(Value::Float(a_val)), Some(Value::Float(b_val))) => {
                                self.push(Value::Float(a_val / b_val));
                            }
                            _ => {
                                unimplemented!("TODO");
                            }
                        }
                        return Ok(());
                    }
                    "beyond" => {
                        let a = self.pop();
                        let b = self.pop();
                        if a.is_none() || b.is_none() {
                            return Err(EvalError {
                                message: "Need two values on stack for `beyond`".to_string(),
                                span: span.clone(),
                            });
                        }

                        match (a, b) {
                            (Some(Value::Integer(a_val)), Some(Value::Integer(b_val))) => {
                                self.push(Value::Integer(if a_val > b_val { 0 } else { 1 }));
                            }
                            (Some(Value::Float(a_val)), Some(Value::Integer(b_val))) => {
                                self.push(Value::Integer(if a_val > b_val as f64 { 0 } else { 1 }));
                            }
                            (Some(Value::Integer(a_val)), Some(Value::Float(b_val))) => {
                                self.push(Value::Integer(if a_val as f64 > b_val { 0 } else { 1 }));
                            }
                            (Some(Value::Float(a_val)), Some(Value::Float(b_val))) => {
                                self.push(Value::Integer(if a_val > b_val { 0 } else { 1 }));
                            }
                            _ => {
                                return Err(EvalError {
                                    message: "Unsupported types for `beyond`".to_string(),
                                    span: span.clone(),
                                });
                            }
                        }
                        return Ok(());
                    }
                    "alternatively" => {
                        let a = self.pop();
                        let b = self.pop();
                        if a.is_none() || b.is_none() {
                            return Err(EvalError {
                                message: "Need two values on stack for `alternatively`".to_string(),
                                span: span.clone(),
                            });
                        }

                        match (a, b) {
                            (Some(Value::Integer(a_val)), Some(Value::Integer(b_val))) => {
                                self.push(Value::Integer(if a_val == 1 || b_val == 1 {
                                    1
                                } else {
                                    0
                                }));
                            }
                            _ => {
                                return Err(EvalError {
                                    message: "Unsupported types for `alternatively`".to_string(),
                                    span: span.clone(),
                                });
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
                    "equally" => {
                        let a = self.pop();
                        let b = self.pop();
                        if a.is_none() || b.is_none() {
                            return Err(EvalError {
                                message: "Need two values on stack for `equally`".to_string(),
                                span: span.clone(),
                            });
                        }

                        match (a, b) {
                            (Some(Value::Integer(a_val)), Some(Value::Integer(b_val))) => {
                                self.push(Value::Integer(if a_val == b_val { 1 } else { 0 }));
                            }
                            (Some(Value::Float(a_val)), Some(Value::Integer(b_val))) => {
                                self.push(Value::Integer(if a_val == b_val as f64 {
                                    1
                                } else {
                                    0
                                }));
                            }
                            (Some(Value::Integer(a_val)), Some(Value::Float(b_val))) => {
                                self.push(Value::Integer(if a_val as f64 == b_val {
                                    1
                                } else {
                                    0
                                }));
                            }
                            (Some(Value::Float(a_val)), Some(Value::Float(b_val))) => {
                                self.push(Value::Integer(if a_val == b_val { 1 } else { 0 }));
                            }
                            _ => {
                                unimplemented!("TODO");
                            }
                        }
                        return Ok(());
                    }
                    "conjointly" => {
                        let b = self.pop();
                        let a = self.pop();
                        if a.is_none() || b.is_none() {
                            return Err(EvalError {
                                message: "Need two values on stack for `conjointly`".to_string(),
                                span: span.clone(),
                            });
                        }

                        match (&a, &b) {
                            (Some(Value::Text(a_val)), Some(Value::Text(b_val))) => {
                                self.push(Value::Text(a_val.to_owned() + b_val));
                            }
                            _ => {
                                return Err(EvalError {
                                    message: format!(
                                        "Need two strings on stack for `conjointly`, got {:?} {:?}",
                                        b, a
                                    ),
                                    span: span.clone(),
                                });
                            }
                        }
                        return Ok(());
                    }
                    "modularly" => {
                        let b = self.pop();
                        let a = self.pop();
                        if a.is_none() || b.is_none() {
                            return Err(EvalError {
                                message: "Need two values on stack for `modularly`".to_string(),
                                span: span.clone(),
                            });
                        }

                        match (&a, &b) {
                            (Some(Value::Integer(a_val)), Some(Value::Integer(b_val))) => {
                                if *b_val == 0 {
                                    return Err(EvalError {
                                        message: "Received divisor of 0 in `modularly`".to_string(),
                                        span: span.clone(),
                                    });
                                }
                                self.push(Value::Integer(a_val % b_val));
                            }
                            _ => {
                                return Err(EvalError {
                                    message: format!(
                                        "Invalid types for `modularly`, got {:?} {:?}",
                                        a, b
                                    ),
                                    span: span.clone(),
                                });
                            }
                        }
                        return Ok(());
                    }
                    "debuggably" => {
                        let index = &self
                            .line_index
                            .as_ref()
                            .expect("Need line index for warnings");
                        let vars: String = variable_store
                            .store
                            .iter()
                            .map(|f| format!("{} -> {:?}, ", f.1.0, f.1.1))
                            .collect();
                        let _ = Report::build(ReportKind::Advice, (index.filename, span.clone()))
                            .with_code("W101")
                            .with_message("Debug")
                            .with_label(
                                Label::new((index.filename, span.clone()))
                                    .with_message(format!("Stack: {:?}", self.stack))
                                    .with_color(Color::Blue),
                            )
                            .with_label(
                                Label::new((index.filename, span.clone()))
                                    .with_message(format!("Vars: {}", vars))
                                    .with_color(Color::Blue),
                            )
                            .finish()
                            .eprint((index.filename, Source::from(index.src)));
                        return Ok(());
                    }
                    &_ => unimplemented!(),
                }
            }
        }

        // If none of the built-ins match, then find a chapter with the name.
        //
        // TODO: Avoid the clones here.
        for (ch_ident, n) in self.idents.clone() {
            if let (Node::Chapter(_, children), _) = n {
                if !is_anagram(&ident, &ch_ident) {
                    continue;
                }

                // We cannot allow the names to match.
                if ident == *ch_ident && !is_flat_adverb {
                    return Err(EvalError {
                        message: format!("Invalid func call, need an anagram of {}", ident),
                        span: span.clone(),
                    });
                }

                let mut variable_store = VariableStore::new();

                // Loop through nodes, back to front.
                for child in children.iter().rev() {
                    let was_return = self.eval_node(&child, &mut variable_store)?;
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

    fn print_warning(&self, title: &str, message: &str, code: &str, span: &Span) {
        let index = &self
            .line_index
            .as_ref()
            .expect("Need line index for warnings");
        let _ = Report::build(ReportKind::Warning, (index.filename, span.clone()))
            .with_code(code)
            .with_message(title)
            .with_label(
                Label::new((index.filename, span.clone()))
                    .with_message(message)
                    .with_color(Color::Yellow),
            )
            .finish()
            .eprint((index.filename, Source::from(index.src)));
    }

    fn eval_value_ref(
        &mut self,
        lines_below: isize,
        variable_store: &VariableStore,
        span: &Span,
    ) -> Result<(), EvalError> {
        let our_line = self
            .line_index
            .as_ref()
            .expect("Evaluator needs LineIndex")
            .get_line(span.start);
        let wanted_line = (our_line as isize + lines_below) as usize;
        let wanted_nodes = self.line_to_literal.get(&wanted_line);

        match wanted_nodes {
            Some(nodes) => {
                let mut my_rng = rng();
                let node = (*nodes).choose(&mut my_rng);

                // Push onto the stack.
                let value = match node {
                    Some(Node::FloatLiteral(f)) => Value::Float(*f),
                    Some(Node::IntLiteral(i)) => Value::Integer(*i),
                    Some(Node::StringLiteral(s)) => Value::Text(
                        s.strip_prefix('"')
                            .unwrap()
                            .strip_suffix('"')
                            .unwrap()
                            .to_owned(),
                    ),
                    Some(Node::Adverb(a)) => {
                        // We look up the current focus variable, whatever char it is we count in the
                        // adverb. Then push that as the value.
                        let focus_var_value = self.get_focus_var(variable_store);
                        let value = count_letters_in(focus_var_value, a) as i64;
                        Value::Integer(value)
                    }
                    Some(Node::ValueRef(value)) => {
                        // We treat the number inside a value ref as a standard integer
                        // For "above" refs we get a negative number, so we need to make
                        // sure it's positive.
                        Value::Integer((*value as i64).abs())
                    }
                    None => {
                        let msg = if lines_below < 0 {
                            format!("No value {} lines above", -lines_below)
                        } else {
                            format!("No value {} lines below", lines_below)
                        };
                        return Err(EvalError {
                            message: msg,
                            span: span.clone(),
                        });
                    }
                    _ => {
                        panic!("Unsupported Node being pushed onto stack");
                    }
                };

                // We print out a warning if this was chosen, because this behaviour is pretty surprising.
                // Especially when it gets chosen randomly (as I have found).
                let nodes_len = nodes.len();
                if nodes_len > 1 as usize {
                    let msg = match node {
                        Some(Node::Adverb(a)) => {
                            format!(
                                "Value deduced from Adverb (\"{}\") which was chosen at random",
                                a
                            )
                        }
                        _ => "Value deduced was chosen at random".to_owned(),
                    };
                    self.print_warning("Possible gotcha", &msg, "W102", span);
                }

                self.stack.push(value);
                Ok(())
            }
            None => {
                let msg = if lines_below < 0 {
                    format!("No value {} lines above", -lines_below)
                } else {
                    format!("No value {} lines below", lines_below)
                };
                return Err(EvalError {
                    message: msg,
                    span: span.clone(),
                });
            }
        }
    }

    fn eval_var_assign(
        &mut self,
        raw_ident: &str,
        variable_store: &mut VariableStore,
        span: &Span,
    ) -> Result<(), EvalError> {
        let val = self.stack.pop();
        match val {
            Some(v) => {
                variable_store.insert(&raw_ident, v);
                Ok(())
            }
            None => Err(EvalError {
                message: "Stack empty".to_string(),
                span: span.clone(),
            }),
        }
    }

    fn eval_var_read(
        &mut self,
        raw_ident: &str,
        variable_store: &VariableStore,
        span: &Span,
    ) -> Result<(), EvalError> {
        // Process the ident to remove punctuation.
        let ident = sort_ident(normalize_ident(raw_ident));

        let value = variable_store.get(&ident);
        match value {
            Some(v) => {
                self.stack.push(v.clone());
                Ok(())
            }
            None => Err(EvalError {
                message: format!("Unknown variable {}", raw_ident),
                span: span.clone(),
            }),
        }
    }

    fn eval_hack(
        &mut self,
        a: &String,
        maybe_b: &Option<String>,
        variable_store: &VariableStore,
        span: &Span,
    ) -> Result<(), EvalError> {
        let val = self.stack.pop();

        match val {
            Some(Value::Integer(v)) if v == 0 => {
                if let Some(b) = maybe_b {
                    self.eval_func_call(b, b.len(), variable_store, span)
                } else {
                    Ok(())
                }
            }
            Some(Value::Integer(v)) if v == 1 => {
                self.eval_func_call(a, a.len(), variable_store, span)
            }
            _ => Err(EvalError {
                message: format!(
                    "Bad value for hack statement, expected 1 or 0, got {:?}",
                    val
                ),
                span: span.clone(),
            }),
        }
    }

    fn eval_focus(
        &mut self,
        t: isize,
        variable_store: &mut VariableStore,
        _span: &Span,
    ) -> Result<(), EvalError> {
        let value = self.get_focus_var(variable_store);

        variable_store.insert("intently", Value::Integer((value - (t as u8)) as i64));
        Ok(())
    }

    fn get_focus_var(&self, variable_store: &VariableStore) -> u8 {
        match variable_store.get("intently") {
            Some(Value::Integer(r)) => *r as u8,
            _ => b'r',
        }
    }
}

// A LineIndex which ignores empty lines. Used for ValueRef evaluation.
#[derive(Debug, Clone)]
pub struct LineIndex<'a> {
    line_starts: Vec<usize>,
    filename: &'a str,
    src: &'a str,
}

impl<'a> LineIndex<'a> {
    pub fn new(src: &'a str, filename: &'a str) -> Self {
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

        Self {
            line_starts,
            filename,
            src,
        }
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
) -> HashMap<usize, Vec<Node>> {
    let mut line_to_literal: HashMap<usize, Vec<Node>> = HashMap::new();
    for node in nodes {
        // TODO: Reduce `line` duplication here.
        match node {
            (Node::FloatLiteral(f), span) => {
                let line = line_index
                    .as_ref()
                    .expect("Evaluator needs LineIndex")
                    .get_line(span.start);
                line_to_literal
                    .entry(line)
                    .or_default()
                    .push(Node::FloatLiteral(*f));
            }
            (Node::IntLiteral(i), span) => {
                let line = line_index
                    .as_ref()
                    .expect("Evaluator needs LineIndex")
                    .get_line(span.start);
                line_to_literal
                    .entry(line)
                    .or_default()
                    .push(Node::IntLiteral(*i));
            }
            (Node::StringLiteral(i), span) => {
                let line = line_index
                    .as_ref()
                    .expect("Evaluator needs LineIndex")
                    .get_line(span.start);
                line_to_literal
                    .entry(line)
                    .or_default()
                    .push(Node::StringLiteral(i.to_owned()));
            }
            (Node::Adverb(a), span) => {
                let line = line_index
                    .as_ref()
                    .expect("Evaluator needs LineIndex")
                    .get_line(span.start);
                line_to_literal
                    .entry(line)
                    .or_default()
                    .push(Node::Adverb(a.to_string()));
            }
            (Node::Story(ident, children), span) => {
                let line = line_index
                    .as_ref()
                    .expect("Evaluator needs LineIndex")
                    .get_line(span.start);
                line_to_literal
                    .entry(line)
                    .or_default()
                    .push(Node::Adverb(ident.to_owned()));
                line_to_literal.extend(collect_values(children, line_index));
            }
            (Node::Chapter(ident, children), span) => {
                let line = line_index
                    .as_ref()
                    .expect("Evaluator needs LineIndex")
                    .get_line(span.start);
                line_to_literal
                    .entry(line)
                    .or_default()
                    .push(Node::Adverb(ident.to_owned()));
                line_to_literal.extend(collect_values(children, line_index));
            }
            (Node::FuncCall(ident, words), span) => {
                let line = line_index
                    .as_ref()
                    .expect("Evaluator needs LineIndex")
                    .get_line(span.start);
                line_to_literal
                    .entry(line)
                    .or_default()
                    .push(Node::Adverb(ident.to_owned()));
                line_to_literal.extend(collect_values(words, line_index));
            }
            (Node::FuncReturn, _) => (),
            (Node::ValueRef(_n), span) => {
                let line = line_index
                    .as_ref()
                    .expect("Evaluator needs LineIndex")
                    .get_line(span.start);
                line_to_literal
                    .entry(line)
                    .or_default()
                    .push(node.unspanned().clone());
            }
            (Node::Word(_), _) => (),
            (Node::VariableAssign(ident), span) => {
                let line = line_index
                    .as_ref()
                    .expect("Evaluator needs LineIndex")
                    .get_line(span.start);
                line_to_literal
                    .entry(line)
                    .or_default()
                    .push(Node::Adverb(ident.to_owned()));
            }
            (Node::VariableRead(ident, child), span) => {
                let children = vec![(*child).as_ref().clone()];
                line_to_literal.extend(collect_values(&children, line_index));

                let line = line_index
                    .as_ref()
                    .expect("Evaluator needs LineIndex")
                    .get_line(span.start);
                line_to_literal
                    .entry(line)
                    .or_default()
                    .push(Node::Adverb(ident.to_owned()));
            }
            (Node::Hack(ident, other), span) => {
                let line = line_index
                    .as_ref()
                    .expect("Evaluator needs LineIndex")
                    .get_line(span.start);
                line_to_literal
                    .entry(line)
                    .or_default()
                    .push(Node::Adverb(ident.to_owned()));
                if let Some(o) = other {
                    line_to_literal
                        .entry(line)
                        .or_default()
                        .push(Node::Adverb(o.to_owned()));
                }
            }
            (Node::FocusChange(_), _) => (),
        }
    }

    return line_to_literal;
}

fn normalize_ident(ident: &str) -> String {
    ident
        .trim_matches(|c: char| c.is_ascii_punctuation())
        .to_ascii_lowercase()
}

fn sort_ident(ident: String) -> String {
    let mut chars: Vec<char> = ident.chars().collect();
    chars.sort_unstable();
    chars.into_iter().collect()
}

fn check_ident_clash(
    idents: &HashMap<String, Spanned<Node>>,
    name: &str,
    span: &Span,
) -> Result<(), EvalError> {
    if BUILT_INS.contains(name) {
        return Err(EvalError {
            message: format!("Duplicate identifier {}", &name),
            span: span.clone(),
        });
    }

    if idents.contains_key(name) {
        return Err(EvalError {
            message: format!("Duplicate identifier {}", &name),
            span: span.clone(),
        });
    }

    return Ok(());
}

fn collect_idents(
    idents: &mut HashMap<String, Spanned<Node>>,
    nodes: &Vec<Spanned<Node>>,
) -> Result<Option<usize>, EvalError> {
    let mut story: Option<usize> = None;
    for (i, n) in nodes.iter().enumerate() {
        match n {
            (Node::Story(name, children), span) => {
                story = Some(i);
                let ident = normalize_ident(name);
                check_ident_clash(idents, &ident, span)?;
                idents.insert(ident.to_string(), n.clone());
                collect_idents(idents, children)?;
            }
            (Node::Chapter(name, _children), span) => {
                let ident = normalize_ident(name);
                check_ident_clash(idents, &ident, span)?;
                idents.insert(ident.to_string(), n.clone());
            }
            (Node::IntLiteral(_), _span) => (),
            (Node::FloatLiteral(_), _span) => (),
            (Node::StringLiteral(_), _span) => (),
            (Node::Adverb(_), _span) => (),
            (Node::Word(_), _span) => (),
            (Node::FuncCall(_, _), _span) => (),
            (Node::ValueRef(_), _span) => (),
            (Node::Hack(_, _), _span) => (),
            (Node::VariableRead(_, _child), _span) => (),
            (Node::VariableAssign(_), _span) => (),
            (Node::FocusChange(_), _span) => (),
            _ => {
                unimplemented!("TODO {:?}", n);
            }
        }
    }
    Ok(story)
}

fn count_words(nodes: &Vec<Spanned<Node>>) -> usize {
    // We need to handle the counting of certain words in a custom way.
    // As an example, string literals are a single node but can contain
    // multiple words.
    let mut result = 0;
    for node in nodes {
        match node {
            (Node::StringLiteral(s), _) => result += s.split_whitespace().count(),
            (Node::Word(_), _) => result += 1,
            (Node::Adverb(_), _) => result += 1,
            (Node::FloatLiteral(_), _) => result += 1,
            (Node::IntLiteral(_), _) => result += 1,
            _ => panic!("Unexpected node type in count words"),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::lex_to_parsed_result;

    #[test]
    fn test_builtin_func_call() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::FuncCall("miltypul".to_string(), vec![]).spanned(0..0)],
            )
            .spanned(0..0),
        ];
        let index = LineIndex::new("", "test.ks");

        let mut evaluator = Evaluator::new(nodes.clone(), Some(index.clone()));
        evaluator.push(Value::Integer(5));
        evaluator.push(Value::Integer(5));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(25)]));
    }

    #[test]
    fn test_builtin_func_call_beyond() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::FuncCall("beyodn".to_string(), vec![]).spanned(0..0)],
            )
            .spanned(0..0),
        ];

        let index = LineIndex::new("", "test.ks");

        let mut evaluator = Evaluator::new(nodes.clone(), Some(index.clone()));
        evaluator.push(Value::Integer(8));
        evaluator.push(Value::Integer(0));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(1)]));
    }

    #[test]
    fn test_builtin_func_call_must_be_anagram() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::FuncCall("multiply".to_string(), vec![]).spanned(0..0)],
            )
            .spanned(0..0),
        ];

        let index = LineIndex::new("", "test.ks");

        let mut evaluator = Evaluator::new(nodes.clone(), Some(index.clone()));
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
                vec![Node::FuncCall("styleit".to_string(), vec![]).spanned(0..0)],
            )
            .spanned(0..0),
            Node::Chapter(
                "testily".to_string(),
                vec![Node::FuncCall("miltypul".to_string(), vec![]).spanned(0..0)],
            )
            .spanned(0..0),
        ];

        let index = LineIndex::new("", "test.ks");

        let mut evaluator = Evaluator::new(nodes.clone(), Some(index.clone()));
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
                vec![Node::FuncCall("testily".to_string(), vec![]).spanned(0..0)],
            )
            .spanned(0..0),
            Node::Chapter("testily".to_string(), vec![]).spanned(0..0),
        ];

        let index = LineIndex::new("", "test.ks");

        let mut evaluator = Evaluator::new(nodes.clone(), Some(index.clone()));
        let res = evaluator.eval_script();
        assert_eq!(
            res,
            Err(EvalError {
                message: "Invalid func call, need an anagram of testily".to_string(),
                span: 0..0
            })
        );
    }

    #[test]
    fn test_custom_func_call_flat_adverb_no_anagram_necessary() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::FuncCall("beyond".to_string(), vec![]).spanned(0..0)],
            )
            .spanned(0..0),
        ];

        let index = LineIndex::new("", "test.ks");

        let mut evaluator = Evaluator::new(nodes.clone(), Some(index.clone()));
        evaluator.push(Value::Integer(5));
        evaluator.push(Value::Integer(10));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(0)]));
    }

    // Verifies that whatever is on the stack at the end of function
    // evaluation is cleared, with only the top of the stock preserved.
    #[test]
    fn test_func_call_only_one_value_after_return() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::FuncCall("tesitly".to_string(), vec![]).spanned(0..0)],
            )
            .spanned(0..0),
            Node::Chapter("testily".to_string(), vec![]).spanned(0..0),
        ];

        let index = LineIndex::new("", "test.ks");

        let mut evaluator = Evaluator::new(nodes.clone(), Some(index.clone()));
        evaluator.push(Value::Integer(5));
        evaluator.push(Value::Integer(10));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(10)]));
    }

    #[test]
    fn test_value_push() {
        let src = "This story starts testly.\nThere is something with a value 2 lines below.\nLine one\nMy secret value is 42";
        let index = LineIndex::new(src, "test.ks");
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
    fn test_value_push_above() {
        let src = "This story starts testly.\nMy secret value is 42\nLine one\nThere is something with a value 2 lines above.\nLine one\nMy secret value is 1234\nThis story ends testly.";
        let nodes = lex_to_parsed_result(src).into_result().unwrap();
        let index = LineIndex::new(src, "test.ks");

        let mut evaluator = Evaluator::new(nodes, Some(index));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(42)]));
    }

    #[test]
    fn test_value_push_using_above() {
        let src = "This story starts testly.\nThere is a value 2 lines below.\nLine one\nThere is something with a value 2 lines above.\nLine one\nMy secret value is 1234\nThis story ends testly.";
        let nodes = lex_to_parsed_result(src).into_result().unwrap();
        let index = LineIndex::new(src, "test.ks");

        let mut evaluator = Evaluator::new(nodes, Some(index));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(2), Value::Integer(2)]));
    }

    #[test]
    fn test_value_push_float() {
        // Also testing repeated newlines here. These should be ignored.
        let src = "This story starts testly.\n\n\n\n\nThere is something with a value 2 lines below.\n\nLine one\n\nMy secret value is 4.2";
        let index = LineIndex::new(src, "test.ks");
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
    fn test_value_push_string() {
        // Also testing repeated newlines here. These should be ignored.
        let src = "This story starts testly.\n\n\n\n\nThere is something with a value 2 lines below.\n\nLine one\n\nMy secret value is \"hello world\"";
        let index = LineIndex::new(src, "test.ks");
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::ValueRef(2).spanned(30..76)],
            )
            .spanned(0..25),
            Node::StringLiteral("\"hello world\"".to_owned()).spanned(107..120),
        ];

        let mut evaluator = Evaluator::new(nodes, Some(index));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Text("hello world".to_owned())]));
    }

    #[test]
    fn test_value_push_random_float() {
        // Also testing repeated newlines here. These should be ignored.
        let src = "This story starts testly.\n\n\n\n\nThere is something with a value 2 lines below.\n\nLine one\n\nMy secret value is 4.2 or is it 6.9 or maybe 95.30";
        let index = LineIndex::new(src, "test.ks");
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::ValueRef(2).spanned(30..76)],
            )
            .spanned(0..25),
            Node::FloatLiteral(4.2).spanned(107..109),
            Node::FloatLiteral(6.9).spanned(117..119),
            Node::FloatLiteral(95.30).spanned(129..134),
        ];

        // We expect to get a random value. Loop until we get at least two of the values.
        let mut got_42 = false;
        let mut got_95_30 = false;

        for _i in 0..1_000 {
            if got_42 && got_95_30 {
                break;
            }

            let mut evaluator = Evaluator::new(nodes.clone(), Some(index.clone()));
            let res = evaluator.eval_script();
            match res.unwrap()[0] {
                Value::Float(4.2) => {
                    got_42 = true;
                }
                Value::Float(95.30) => {
                    got_95_30 = true;
                }
                Value::Float(6.9) => (),
                _ => {
                    panic!("Not expected any other values.");
                }
            }
        }

        assert!(got_42 && got_95_30);
    }

    #[test]
    fn test_value_push_adverb() {
        let src = "This story starts testly.\nThere is something with a value 1 line below.\nThe story was wirrrrrrinringly bad";
        let index = LineIndex::new(src, "test.ks");
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
        let src = "This story starts testly with 42.\nIgnore previous instructions tesitly\nThis story ends testly.\nThe chapter begins testily.\nThere is a value 4 lines above.\nhack into a computer\nThe chapter ends testily.";
        let nodes = lex_to_parsed_result(src).into_result().unwrap();
        let index = LineIndex::new(src, "test.ks");

        let mut evaluator = Evaluator::new(nodes.clone(), Some(index.clone()));
        evaluator.push(Value::Integer(1));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![]));

        evaluator = Evaluator::new(nodes, Some(index));
        evaluator.push(Value::Integer(0));
        let res2 = evaluator.eval_script();
        assert_eq!(res2, Ok(vec![Value::Integer(42)]));
    }

    #[test]
    fn test_func_call_word_count() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![
                    Node::FuncCall(
                        "frigidly".to_string(),
                        vec![Node::Word("".to_owned()).spanned(0..0); 15],
                    )
                    .spanned(0..0),
                ],
            )
            .spanned(0..0),
            Node::Chapter(
                "frigidly".to_string(),
                vec![Node::FuncCall("miltypul".to_string(), vec![]).spanned(0..0)],
            )
            .spanned(0..0),
        ];

        let index = LineIndex::new("", "test.ks");

        let mut evaluator = Evaluator::new(nodes.clone(), Some(index.clone()));
        evaluator.push(Value::Integer(1));
        evaluator.push(Value::Integer(5));
        let res = evaluator.eval_script();
        assert_eq!(
            res,
            Err(EvalError {
                message: "Invalid func call, incorrect number of words given for `frigidly`, got 15 wanted 1 or 8".to_string(),
                span: 0..0
            })
        );
    }

    #[test]
    fn test_func_call_word_count_str() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![
                    Node::FuncCall(
                        "fyl".to_string(),
                        vec![Node::StringLiteral("multiple words here".to_owned()).spanned(0..0)],
                    )
                    .spanned(0..0),
                ],
            )
            .spanned(0..0),
            Node::Chapter(
                "fly".to_string(),
                vec![Node::FuncCall("miltypul".to_string(), vec![]).spanned(0..0)],
            )
            .spanned(0..0),
        ];

        let index = LineIndex::new("", "test.ks");

        let mut evaluator = Evaluator::new(nodes.clone(), Some(index.clone()));
        evaluator.push(Value::Integer(6));
        evaluator.push(Value::Integer(5));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(30)]));
    }

    #[test]
    fn test_variables() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![
                    Node::VariableRead(
                        "oflly".to_string(),
                        Box::new(Node::IntLiteral(0).spanned(0..0)),
                    )
                    .spanned(0..0),
                    Node::VariableRead(
                        "oflly".to_string(),
                        Box::new(Node::IntLiteral(0).spanned(0..0)),
                    )
                    .spanned(0..0),
                    Node::VariableRead(
                        "oflly".to_string(),
                        Box::new(Node::IntLiteral(0).spanned(0..0)),
                    )
                    .spanned(0..0),
                    Node::VariableAssign("folly".to_string()).spanned(0..0),
                ],
            )
            .spanned(0..0),
        ];
        let index = LineIndex::new("" /* XXX */, "test.ks");

        let mut evaluator = Evaluator::new(nodes, Some(index));
        evaluator.push(Value::Integer(42));
        let res = evaluator.eval_script();
        assert_eq!(
            res,
            Ok(vec![
                Value::Integer(42),
                Value::Integer(42),
                Value::Integer(42)
            ])
        );
    }

    #[test]
    fn test_hack_stmt() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::Hack("firgidly".to_string(), Some("fllyo".to_string())).spanned(0..0)],
            )
            .spanned(0..0),
            Node::Chapter(
                "frigidly".to_string(),
                vec![Node::FuncCall("miltypul".to_string(), vec![]).spanned(0..0)],
            )
            .spanned(0..0),
            Node::Chapter(
                "folly".to_string(),
                vec![Node::FuncCall("additioanlly".to_string(), vec![]).spanned(0..0)],
            )
            .spanned(0..0),
        ];

        let index = LineIndex::new("", "test.ks");

        let mut evaluator = Evaluator::new(nodes.clone(), Some(index.clone()));
        evaluator.push(Value::Integer(5));
        evaluator.push(Value::Integer(5));
        evaluator.push(Value::Integer(0));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(10)]));
    }

    #[test]
    fn test_hack_stmt_one() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::Hack("firgidly".to_string(), None).spanned(0..0)],
            )
            .spanned(0..0),
            Node::Chapter(
                "frigidly".to_string(),
                vec![Node::FuncCall("miltypul".to_string(), vec![]).spanned(0..0)],
            )
            .spanned(0..0),
        ];

        let index = LineIndex::new("", "test.ks");

        let mut evaluator = Evaluator::new(nodes.clone(), Some(index.clone()));
        evaluator.push(Value::Integer(5));
        evaluator.push(Value::Integer(5));
        evaluator.push(Value::Integer(1));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(25)]));
    }

    #[test]
    fn test_arithmetic_sub() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::FuncCall("deductviely".to_string(), vec![]).spanned(0..0)],
            )
            .spanned(0..0),
        ];

        let index = LineIndex::new("", "test.ks");

        let mut evaluator = Evaluator::new(nodes.clone(), Some(index.clone()));
        evaluator.push(Value::Integer(5));
        evaluator.push(Value::Integer(1));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(4)]));
    }

    #[test]
    fn test_arithmetic_div() {
        let nodes = vec![
            Node::Story(
                "testly".to_string(),
                vec![Node::FuncCall("divisibyl".to_string(), vec![]).spanned(0..0)],
            )
            .spanned(0..0),
        ];

        let index = LineIndex::new("", "test.ks");

        let mut evaluator = Evaluator::new(nodes.clone(), Some(index.clone()));
        evaluator.push(Value::Integer(5));
        evaluator.push(Value::Integer(1));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Float(5.0)]));
    }

    #[test]
    fn test_focus_change() {
        let src = "This story starts testly with 42.\nintently = <stack>;```<content>\nThis story ends testly.";
        let nodes = lex_to_parsed_result(src).into_result().unwrap();
        let index = LineIndex::new(src, "test.ks");

        let mut evaluator = Evaluator::new(nodes.clone(), Some(index.clone()));
        let res = evaluator.eval_script();
        assert_eq!(res, Ok(vec![Value::Integer(111)]));
    }
}
