//! The data tables stay in step with the enums they are indexed by.

use crate::token::KIND_INFO;
use crate::*;

#[test]
fn kind_table_follows_the_enum() {
    for (i, kind) in TokenKind::ALL.into_iter().enumerate() {
        assert_eq!(kind as usize, i);
        assert_eq!(KIND_INFO[i].0, kind);
    }
    assert_eq!(kind_class(TokenKind::NameBuiltinPseudo), "bp");
    assert_eq!(kind_name(TokenKind::StringDouble), "Literal.String.Double");
    assert_eq!(kind_parent(TokenKind::String), Some(TokenKind::Literal));
    assert!(kind_is(TokenKind::StringDouble, TokenKind::Literal));
    assert!(!kind_is(TokenKind::TokenString, TokenKind::Literal));
}

#[test]
fn lexer_table_follows_the_enum() {
    for (i, lexer) in Lexer::ALL.into_iter().enumerate() {
        assert_eq!(lexer as usize, i);
        assert_eq!(find_lexer(lexer_alias(lexer)), Some(lexer));
    }
    assert_eq!(lexer_name(Lexer::Cpp), "C++");
    assert_eq!(find_lexer("a/setup.py"), Some(Lexer::Python));
    assert_eq!(find_lexer("Makefile"), Some(Lexer::Makefile));
    assert_eq!(find_lexer("text/x-rst"), Some(Lexer::Rst));
    assert_eq!(find_lexer_for_filename("dir\\.bash_profile"), Some(Lexer::Bash));
    assert_eq!(find_lexer(" "), None);
}

#[test]
fn style_table_follows_the_enum() {
    for (i, style) in Style::ALL.into_iter().enumerate() {
        assert_eq!(style as usize, i);
        assert_eq!(find_style(style_name(style)), Some(style));
    }
    let looks = resolve_style(Style::Default);
    assert_eq!(looks.look(TokenKind::Keyword), kind_look(Style::Default, TokenKind::Keyword));
    assert!(looks.look(TokenKind::Keyword).bold);
}
