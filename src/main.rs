use logos::Logos;

#[derive(Logos, Debug, PartialEq)]
#[logos(skip r"[ \t\n\f]+")] // Ignore this regex pattern between tokens
enum Token {
    // Tokens can be literal strings, of any length.
    #[token("fast")]
    Fast,

    #[token(".")]
    Period,

    // Or regular expressions.
    #[regex("[a-zA-Z]+")]
    Text,
}

fn main() {
    let mut lex = Token::lexer("Create ridiculously fast Lexers.");

    assert_eq!(lex.next(), Some(Ok(Token::Text)));
    assert_eq!(lex.span(), 0..6);
    assert_eq!(lex.slice(), "Create");

    assert_eq!(lex.next(), Some(Ok(Token::Text)));
    assert_eq!(lex.span(), 7..19);
    assert_eq!(lex.slice(), "ridiculously");

    assert_eq!(lex.next(), Some(Ok(Token::Fast)));
    assert_eq!(lex.span(), 20..24);
    assert_eq!(lex.slice(), "fast");

    assert_eq!(lex.next(), Some(Ok(Token::Text)));
    assert_eq!(lex.slice(), "Lexers");
    assert_eq!(lex.span(), 25..31);

    assert_eq!(lex.next(), Some(Ok(Token::Period)));
    assert_eq!(lex.span(), 31..32);
    assert_eq!(lex.slice(), ".");

    assert_eq!(lex.next(), None);

    println!("All tests passed");
}
