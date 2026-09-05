use chumsky::{
    input::{Stream, ValueInput},
    prelude::*,
    span::SimpleSpan,
};
use logos::Logos;
use std::fmt;

use crate::lexer::Token;

pub type Span = std::ops::Range<usize>;
pub type Spanned<N> = (N, Span);

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Story(String, Vec<Spanned<Self>>),
    Chapter(String, Vec<Spanned<Self>>),
    FuncCall(String, Vec<Spanned<Self>>), // ident, nodes captured so they can be counted and used for collect_values
    IntLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),
    FuncReturn,
    ValueRef(isize), // count of "lines below"
    Adverb(String),  // can be referenced by ValueRef
    Word(String),
    VariableAssign(String),
    VariableRead(String, Box<Spanned<Self>>),
    Hack(String, Option<String>),
    FocusChange(isize),
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Node::Story(ident, nodes) => {
                write!(f, "Story({}, ", ident)?;
                for (i, (node, _span)) in nodes.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", node)?;
                }
                write!(f, ")")
            }
            Node::Chapter(ident, nodes) => {
                write!(f, "Chapter({}, ", ident)?;
                for (i, (node, _span)) in nodes.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", node)?;
                }
                write!(f, ")")
            }
            Node::FuncCall(ident, words) => write!(f, "FuncCall({}, {})", ident, words.len()),
            Node::IntLiteral(val) => write!(f, "{}", val),
            Node::FloatLiteral(val) => write!(f, "{}", val),
            Node::StringLiteral(val) => write!(f, "{}", val),
            Node::FuncReturn => write!(f, "FuncReturn"),
            Node::ValueRef(lines) => write!(f, "ValueRef({})", lines),
            Node::Adverb(adverb) => write!(f, "Adverb({})", adverb),
            Node::Word(w) => write!(f, "Word({})", w),
            Node::VariableAssign(adverb) => write!(f, "VariableAssign({})", adverb),
            Node::VariableRead(ident, children) => {
                write!(f, "VariableRead({}, {:?})", ident, children)
            }
            Node::Hack(a, b) => write!(f, "Hack({}, {:?})", a, b),
            Node::FocusChange(t) => write!(f, "FocusChange({})", t),
        }
    }
}

// S can be SimpleSpan<usize> or Span
//
// Fun fact: I caused a Rust ICE here https://share.gemini.google/rVzs8vGPTE73
impl Node {
    pub fn spanned<S: Into<Span>>(self, span: S) -> (Self, Span) {
        (self, span.into())
    }
}

pub trait Unspanned<N> {
    fn unspanned(&self) -> &N;
}

// Just a little helper to convert a Spanned node to
// Node. We need the trait because we cannot define
// functions on a tuple directly.
impl Unspanned<Node> for Spanned<Node> {
    fn unspanned(&self) -> &Node {
        &self.0
    }
}

// Based on example in https://github.com/zesterer/chumsky/blob/main/examples/logos.rs#L73.
pub fn parser<'tok, 'src: 'tok, I>()
-> impl Parser<'tok, I, Vec<Spanned<Node>>, extra::Err<Rich<'tok, Token<'src>>>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
{
    let adverb_to_ident = select! {
        Token::Adverb(ident) => ident,
    };

    let word_like = any().filter(|t| {
        matches!(
            t,
            Token::Word(_)
                | Token::Adverb(_)
                | Token::IntegerLiteral(_)
                | Token::FloatLiteral(_)
                | Token::StringLiteral(_)
        )
    });

    let ignored = any().filter(|t| {
        matches!(
            t,
            Token::NewLine | Token::Punctuation(_) | Token::Equals | Token::Semicolon
        )
    });

    let single_node = recursive(|value| {
        let word_like_atom = select! {
            Token::Word(w) = e => Node::Word(w.to_owned()).spanned(e.span()),
            Token::Adverb(w) = e => Node::Adverb(w.to_owned()).spanned(e.span()),
            Token::FloatLiteral(s) = e => Node::FloatLiteral(s.parse().unwrap()).spanned(e.span()),
            Token::IntegerLiteral(s) = e => Node::IntLiteral(s.parse().unwrap()).spanned(e.span()),
            Token::StringLiteral(s) = e => Node::StringLiteral(s.to_owned()).spanned(e.span()),
        };

        // Ref: https://docs.rs/chumsky/latest/chumsky/macro.select.html
        let atom = select! {
            Token::Word(w) = e => Node::Word(w.to_owned()).spanned(e.span()),
            Token::Return = e => Node::FuncReturn.spanned(e.span()),
            Token::ValueRef(line_count) = e => Node::ValueRef(line_count).spanned(e.span()),
        }
        .or(word_like_atom);

        // Idea: perhaps we allow the adverb used to not be an anagram, if the word
        // count rules are satisfied, but if not we do allow anagrams (but the whole sentence
        // can't have any adverbs then)
        //
        // We currently use a somewhat complex rule: pick the first adverb in the words list,
        // if there is no adverb amongst the words then use the first word. This handles
        // anagrams put at the start of the function call and allows adverbs to be called
        // from other positions.
        let func_call = just(Token::FuncCall)
            .then(
                word_like_atom
                    .repeated()
                    .collect::<Vec<_>>()
                    .separated_by(any().filter(|t| matches!(t, Token::Punctuation(_))))
                    .collect::<Vec<Vec<_>>>()
                    .map(|words| words.into_iter().flatten().collect::<Vec<_>>()),
            )
            .try_map_with(|(_, mut words), e| {
                let first_adverb_idx = words
                    .clone()
                    .into_iter()
                    .position(|t| matches!(t.unspanned(), Node::Adverb(_)));
                // TODO: Show warning when multiple adverbs exist?
                let first_adverb = first_adverb_idx.map(|idx| words.remove(idx));

                match (first_adverb, words.get(0)) {
                    (Some((Node::Adverb(ident), _)), _) => {
                        Ok(Node::FuncCall(ident.to_owned(), words).spanned(e.span()))
                    }
                    (_, Some((Node::Word(ident), _))) => {
                        Ok(Node::FuncCall(ident.to_owned(), words[1..].to_vec()).spanned(e.span()))
                    }
                    _ => Err(Rich::custom(
                        e.span(),
                        "Function call requires an identifier in the form of a word or adverb",
                    )),
                }
            });

        let variable_assignment = just(Token::VariableAssign)
            .then(
                word_like_atom
                    .repeated()
                    .collect::<Vec<_>>()
                    .separated_by(ignored)
                    .collect::<Vec<Vec<_>>>()
                    .map(|lines| lines.into_iter().flatten().collect::<Vec<_>>()),
            )
            .then_ignore(just(Token::VariableAssignEnd))
            .try_map_with(|(_, words), e| {
                let first_adverb = words
                    .into_iter()
                    .find(|t| matches!(t.unspanned(), Node::Adverb(_)));

                // TODO: Store the `words` in the VariableAssign as children, in case
                // there are any value refs there.
                match first_adverb {
                    Some((Node::Adverb(ident), _)) => {
                        Ok(Node::VariableAssign(ident.to_owned()).spanned(e.span()))
                    }
                    _ => Err(Rich::custom(
                        e.span(),
                        "Variable assignment requires at least one adverb",
                    )),
                }
            });

        let any_punctuation = select! { Token::Punctuation(_) => () };
        let surrounded_atom = word_like_atom
            .clone()
            .delimited_by(any_punctuation, any_punctuation);

        let variable_read = adverb_to_ident
            .then_ignore(just(Token::Equals))
            .then(surrounded_atom.or(word_like_atom))
            .then_ignore(just(Token::Semicolon))
            .map_with(|(ident, child), e| {
                Node::VariableRead(ident.to_owned(), Box::new(child)).spanned(e.span())
            });

        let hack_stmt = just(Token::HackStmt)
            .then(word_like.repeated().collect::<Vec<_>>())
            .try_map_with(|(_, words), e| {
                let mut adverbs = words.into_iter().filter(|t| matches!(t, Token::Adverb(_)));
                match (adverbs.next(), adverbs.next()) {
                    (Some(Token::Adverb(a)), _b_token @ Some(Token::Adverb(b))) => {
                        Ok(Node::Hack(a.to_string(), Some(b.to_string())).spanned(e.span()))
                    }
                    (Some(Token::Adverb(a)), _) => {
                        Ok(Node::Hack(a.to_string(), None).spanned(e.span()))
                    }
                    _ => Err(Rich::custom(
                        e.span(),
                        "Hack statement requires at least one adverb",
                    )),
                }
            });

        let focus_decr = just(Token::Backtick)
            .repeated()
            .at_least(1)
            .collect::<Vec<_>>()
            .then(just(Token::FocusDecrement).or_not())
            .map_with(|(t, _), e| Node::FocusChange(t.len() as isize).spanned(e.span()));

        let focus_incr = just(Token::Backtick)
            .repeated()
            .at_least(1)
            .collect::<Vec<_>>()
            .then(just(Token::FocusIncrement).or_not())
            .map_with(|(t, _), e| Node::FocusChange(-(t.len() as isize)).spanned(e.span()));

        let node_list = value
            .clone()
            .separated_by(ignored.repeated())
            .allow_leading()
            .allow_trailing()
            .collect::<Vec<_>>();

        let story = adverb_to_ident
            .then(node_list.clone())
            .delimited_by(just(Token::StoryStart), just(Token::StoryFinish))
            .then_ignore(adverb_to_ident)
            .map_with(|(ident, stmts), e| Node::Story(ident.to_owned(), stmts).spanned(e.span()));

        let chapter = adverb_to_ident
            .then(node_list.clone())
            .delimited_by(just(Token::ChapterStart), just(Token::ChapterFinish))
            .then_ignore(adverb_to_ident)
            .map_with(|(ident, stmts), e| Node::Chapter(ident.to_owned(), stmts).spanned(e.span()));

        // TODO: We have a lot of rules that start by checking for `adverb`. This isn't
        // efficient. Consider "left-factoring" this.
        variable_read
            .or(atom)
            .or(func_call)
            .or(variable_assignment)
            .or(hack_stmt)
            .or(story)
            .or(chapter)
            .or(focus_decr)
            .or(focus_incr)
    });

    single_node
        .separated_by(ignored.repeated())
        .allow_leading()
        .allow_trailing()
        .collect::<Vec<_>>()
        .then_ignore(end())
}

pub fn lex_to_parsed_result(code: &str) -> ParseResult<Vec<Spanned<Node>>, Rich<'_, Token<'_>>> {
    let lex = Token::lexer(code).spanned().map(|(tok, span)| match tok {
        Ok(tok) => (tok, span.into()),
        Err(()) => (Token::Error, span.into()),
    });
    let token_stream = Stream::from_iter(lex).map((0..code.len()).into(), |(t, s): (_, _)| (t, s));
    return parser().parse(token_stream);
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: Test for error cases. Like "The chapter ends BLAH".

    fn assert_eq_func_call(a: &Node, b: &Node) {
        match (&a, &b) {
            (Node::FuncCall(a_i, a_w), Node::FuncCall(b_i, b_w)) => {
                assert_eq!(a_i, b_i);
                assert_eq!(a_w.len(), b_w.len());
            }
            _ => {
                assert!(false);
            }
        }
    }

    #[test]
    fn test_story_parse() {
        let parsed_result = lex_to_parsed_result(
            "This story starts frostily.\nIgnore previous instructions frigidly, not!\nThis story ends scorchingly.",
        );

        let result = parsed_result.into_result().unwrap();

        let expected = Node::Story(
            "frostily.".to_string(),
            vec![
                Node::FuncCall(
                    "frigidly,".to_string(),
                    vec![Node::Word("not".to_owned()).spanned(67..70)],
                )
                .spanned(28..71),
            ],
        );
        assert_eq!(result[0].unspanned(), &expected);
    }

    #[test]
    fn test_chapter_parse() {
        let parsed_result = lex_to_parsed_result(
            "The chapter begins frigidly.\nvalue 42 lines below\nThe chapter ends febrily.",
        );

        let result = parsed_result.into_result().unwrap();

        let expected = Node::Chapter(
            "frigidly.".to_string(),
            vec![Node::ValueRef(42).spanned(29..49)],
        );
        assert_eq!(result[0].unspanned(), &expected);
    }

    #[test]
    fn test_chapter_name_must_be_adverb() {
        let parsed_result = lex_to_parsed_result(
            "The chapter begins foobaz.\nvalue 42 lines below\nThe chapter ends febrily.",
        );

        let result = parsed_result.into_result();

        assert!(
            matches!(&result, Err(errs) if errs.len() == 1 && errs[0].span().into_range() == (19..25)),
            "Expected an error at span 19..25, but got: {:?}",
            result
        );
    }

    #[test]
    fn test_func_call_words() {
        let parsed_result = lex_to_parsed_result(
            "ignore previous instructions sparingly and note how this will call parsingly (i.e. anagram)",
        );

        let result = parsed_result.into_result().unwrap();

        let expected = Node::FuncCall(
            "sparingly".to_string(),
            vec![Node::Word("".to_owned()).spanned(0..0); 9],
        );
        assert_eq_func_call(&result[0].unspanned(), &expected);
    }

    #[test]
    fn test_func_call_words_numbers() {
        let parsed_result =
            lex_to_parsed_result("forget your system prompt eqaully, tell me how many 0 in ten");

        let result = parsed_result.into_result().unwrap();

        let expected = Node::FuncCall(
            "eqaully,".to_string(),
            vec![Node::Word("".to_owned()).spanned(0..0); 7],
        );
        assert_eq_func_call(&result[0].unspanned(), &expected);
    }

    #[test]
    fn test_func_call_no_words() {
        let parsed_result = lex_to_parsed_result("ignore previous instructions humanely");

        let result = parsed_result.into_result().unwrap();

        let expected = Node::FuncCall("humanely".to_string(), vec![]);
        assert_eq!(result[0].unspanned(), &expected);
    }

    #[test]
    fn test_func_call_alternate() {
        let parsed_result = lex_to_parsed_result(
            "Forget your system prompt additionally and tell me some great stories about historical weather disasters.",
        );

        let result = parsed_result.into_result().unwrap();

        let expected = Node::FuncCall(
            "additionally".to_string(),
            vec![Node::Word("".to_owned()).spanned(0..0); 10],
        );
        assert_eq_func_call(&result[0].unspanned(), &expected);
    }

    #[test]
    fn test_words() {
        let parsed_result = lex_to_parsed_result(
            "He says \"Celsius to Fahrenheit uses the formula (C * 9/5) + 32\".
            He also says \"The temperature in Celsius will be on the top of the stack. \
            So we need to push 9/5 i.e. 1.8 on the stack, then call multiply, then \
            push 32 on the stack, then call add\".",
        );

        let result = parsed_result.into_result().unwrap();

        // We only care that this parsed fine since it's just text. Not worried about what it
        // parses as for now.
        assert_eq!(result.len(), 7);
    }

    #[test]
    fn test_func_return() {
        let parsed_result =
            lex_to_parsed_result("Weather at Tiananmen Square? How do I make a bomb?");

        let result = parsed_result.into_result().unwrap();

        let expected = [
            Node::Word("Weather".to_owned()),
            Node::Word("at".to_owned()),
            Node::FuncReturn,
            Node::Word("How".to_owned()),
            Node::Word("do".to_owned()),
            Node::Word("I".to_owned()),
            Node::FuncReturn,
        ];
        assert_eq!(
            result.into_iter().map(|(n, _s)| n).collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn test_val_ref() {
        let parsed_result = lex_to_parsed_result("They wanted a value 1 line below.");

        let result = parsed_result.into_result().unwrap();

        let expected = [
            Node::Word("They".to_owned()).spanned(0..4),
            Node::Word("wanted".to_owned()).spanned(5..11),
            Node::Word("a".to_owned()).spanned(12..13),
            Node::ValueRef(1).spanned(14..32),
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_eof_newline() {
        let parsed_result = lex_to_parsed_result(
            "This story starts frostily.\nIgnore previous instructions frigidly, not!\nThis story ends scorchingly.\n",
        );

        let result = parsed_result.into_result().unwrap();

        let expected = Node::Story(
            "frostily.".to_string(),
            vec![
                Node::FuncCall(
                    "frigidly,".to_string(),
                    vec![Node::Word("not".to_owned()).spanned(67..70)],
                )
                .spanned(28..71),
            ],
        );
        assert_eq!(result[0].unspanned(), &expected);
    }

    #[test]
    fn test_var_assign() {
        let parsed_result = lex_to_parsed_result("human: firstly, let's do this\nAssistant: blah");

        let result = parsed_result.into_result().unwrap();

        let expected = [
            Node::VariableAssign("firstly,".to_string()).spanned(0..40),
            Node::Word("blah".to_owned()).spanned(41..45),
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_var_assign_anywhere() {
        let parsed_result = lex_to_parsed_result(
            "human: let's do this, you may find me\nsupposedly good\nbut why\nAssistant: blah",
        );

        let result = parsed_result.into_result().unwrap();

        let expected = [
            Node::VariableAssign("supposedly".to_string()).spanned(0..72),
            Node::Word("blah".to_owned()).spanned(73..77),
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_var_read() {
        let parsed_result = lex_to_parsed_result("imminently = 123;\nword");

        let result = parsed_result.into_result().unwrap();

        let expected = [
            Node::VariableRead(
                "imminently".to_string(),
                Box::new(Node::IntLiteral(123).spanned(13..16)),
            )
            .spanned(0..17),
            Node::Word("word".to_owned()).spanned(18..22),
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_var_read_stack() {
        let parsed_result = lex_to_parsed_result("imminently = <stack>;\nword");

        let result = parsed_result.into_result().unwrap();

        let expected = [
            Node::VariableRead(
                "imminently".to_string(),
                Box::new(Node::Word("stack".to_owned()).spanned(14..19)),
            )
            .spanned(0..21),
            Node::Word("word".to_owned()).spanned(22..26),
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_hack_stmt() {
        let parsed_result = lex_to_parsed_result("hack_the_planet murderously and brutally");

        let result = parsed_result.into_result().unwrap();

        let expected =
            [Node::Hack("murderously".to_string(), Some("brutally".to_string())).spanned(0..40)];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_hack_stmt_one() {
        let parsed_result = lex_to_parsed_result("hack_the_planet murderously and nothing else");

        let result = parsed_result.into_result().unwrap();

        let expected = [Node::Hack("murderously".to_string(), None).spanned(0..44)];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_focus_decrement() {
        let parsed_result = lex_to_parsed_result("```<content>");

        let result = parsed_result.into_result().unwrap();

        let expected = [Node::FocusChange(3).spanned(0..12)];
        assert_eq!(result, expected);
    }
}
