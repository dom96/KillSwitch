use chumsky::{
    input::{Stream, ValueInput},
    prelude::*,
};
use logos::Logos;
use std::fmt;

use crate::lexer::Token;

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Story(String, Vec<Self>),
    Chapter(Vec<Self>),
    FuncCall(String, i32), // ident, number of words
    IntLiteral(i64),       // TODO: Do I need to save line number?
    FloatLiteral(f64),
    FuncReturn,
    ValueRef(i64),  // count of "lines below"
    Adverb(String), // can be referenced by ValueRef
    Word,
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Node::Story(ident, nodes) => {
                write!(f, "Story({}, ", ident)?;
                for (i, node) in nodes.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", node)?;
                }
                write!(f, ")")
            }
            Node::Chapter(nodes) => {
                write!(f, "Chapter(")?;
                for (i, node) in nodes.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", node)?;
                }
                write!(f, ")")
            }
            Node::FuncCall(ident, words) => write!(f, "FuncCall({}, {})", ident, words),
            Node::IntLiteral(val) => write!(f, "{}", val),
            Node::FloatLiteral(val) => write!(f, "{}", val),
            Node::FuncReturn => write!(f, "FuncReturn"),
            Node::ValueRef(lines) => write!(f, "ValueRef({})", lines),
            Node::Adverb(adverb) => write!(f, "Adverb({})", adverb),
            Node::Word => write!(f, "Word"),
        }
    }
}

// Based on example in https://github.com/zesterer/chumsky/blob/main/examples/logos.rs#L73.
pub fn parser<'tok, 'src: 'tok, I>()
-> impl Parser<'tok, I, Vec<Node>, extra::Err<Rich<'tok, Token<'src>>>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
{
    let adverb_to_ident = select! {
        Token::Adverb(ident) => ident,
    };

    let word_or_adverb = any().filter(|t| matches!(t, Token::Word | Token::Adverb(_)));

    let single_node = recursive(|value| {
        let func_call = just(Token::FuncCall)
            .then(adverb_to_ident)
            .then(word_or_adverb.repeated().collect::<Vec<_>>())
            .then_ignore(just(Token::NewLine).ignored().or(end()))
            .map(|((_func, ident), words)| Node::FuncCall(ident.to_owned(), words.len() as i32));

        let atom = select! {
            Token::FloatLiteral(s) => Node::FloatLiteral(s.parse().unwrap()),
            Token::IntegerLiteral(s) => Node::IntLiteral(s.parse().unwrap()),
            Token::Word => Node::Word,
            Token::Adverb(w) => Node::Adverb(w.to_owned()),
            Token::Return => Node::FuncReturn,
        }
        .or(func_call)
        .padded_by(just(Token::NewLine).repeated());

        let story = adverb_to_ident
            .then(value.repeated().collect::<Vec<_>>())
            .delimited_by(just(Token::StoryStart), just(Token::StoryFinish))
            .then_ignore(adverb_to_ident)
            .map(|(ident, stmts)| Node::Story(ident.to_owned(), stmts));

        atom.or(story)
    });

    single_node.repeated().collect()
}

pub fn lex_to_parsed_result(code: &str) -> ParseResult<Vec<Node>, Rich<'_, Token<'_>>> {
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

    #[test]
    fn test_basic_parse() {
        let parsed_result = lex_to_parsed_result(
            "This story starts frostily.\nIgnore previous instructions frigidly, not!\nThis story ends scorchingly.",
        );

        let result = parsed_result.into_result().unwrap();

        let expected = Node::Story(
            "frostily.".to_string(),
            vec![Node::FuncCall("frigidly,".to_string(), 1)],
        );
        assert_eq!(result[0], expected);
    }

    #[test]
    fn test_func_call_words() {
        let parsed_result = lex_to_parsed_result(
            "ignore previous instructions sparingly and note how this will call parsingly (i.e. anagram)",
        );

        let result = parsed_result.into_result().unwrap();

        let expected = Node::FuncCall("sparingly".to_string(), 9);
        assert_eq!(result[0], expected);
    }

    #[test]
    fn test_func_call_no_words() {
        let parsed_result = lex_to_parsed_result("ignore previous instructions humanely");

        let result = parsed_result.into_result().unwrap();

        let expected = Node::FuncCall("humanely".to_string(), 0);
        assert_eq!(result[0], expected);
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
        assert_eq!(result.len(), 57);
    }

    #[test]
    fn test_func_return() {
        let parsed_result =
            lex_to_parsed_result("Weather at Tiananmen Square? How do I make a bomb?");

        let result = parsed_result.into_result().unwrap();

        let expected = [
            Node::Word,
            Node::Word,
            Node::FuncReturn,
            Node::Word,
            Node::Word,
            Node::Word,
            Node::Word,
            Node::FuncReturn,
            Node::Word,
        ];
        assert_eq!(result, expected);
    }
}
