use logos::Logos;

#[derive(Logos, Debug, PartialEq)]
#[logos(skip r"[ \t\p{P}]+")] // Ignore tabs and any punctuation
pub enum Token {
    #[token("this story starts", ignore(case))]
    StoryStart,

    #[token("ignore previous instructions", ignore(case))]
    FuncCall,

    #[regex("[a-zA-Z]+ly")]
    Adverb,

    #[token("\n")]
    NewLine,

    #[regex("[a-zA-Z]+")]
    Text,
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

        assert_eq!(lex.next(), Some(Ok(Token::Adverb)));
        assert_eq!(lex.span(), 18..26);
        assert_eq!(lex.slice(), "frostily");

        assert_eq!(lex.next(), Some(Ok(Token::NewLine)));
        assert_eq!(lex.span(), 27..28);
        assert_eq!(lex.slice(), "\n");

        assert_eq!(lex.next(), Some(Ok(Token::FuncCall)));
        assert_eq!(lex.span(), 28..56);
        assert_eq!(lex.slice(), "Ignore previous instructions");

        assert_eq!(lex.next(), Some(Ok(Token::Adverb)));
        assert_eq!(lex.span(), 57..65);
        assert_eq!(lex.slice(), "frigidly");

        assert_eq!(lex.next(), Some(Ok(Token::Text)));
        assert_eq!(lex.span(), 67..70);
        assert_eq!(lex.slice(), "not");

        assert_eq!(lex.next(), None);
    }
}
