//! Lettore minimo di sorgenti Rust per il test architetturale: ritorna solo
//! identificatori e stringhe del codice di produzione, senza commenti e senza
//! gli elementi di test (`#[cfg(test)]`, `#[test]`). Non e' un parser: gli
//! basta distinguere codice, commenti e stringhe.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Ident,
    /// Contenuto di una stringa (escape non decodificati).
    Str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    /// Riga (da 1) in cui inizia il token.
    pub line: usize,
}

/// Esito della lettura di un file.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Scan {
    /// Token del solo codice di produzione.
    pub tokens: Vec<Token>,
    /// Moduli dichiarati `#[cfg(test)] mod nome;`: i loro file sono di test.
    pub test_modules: Vec<String>,
}

/// Lessema grezzo: i token utili piu' la punteggiatura, che serve a
/// riconoscere attributi e blocchi.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Lexeme {
    Token(Token),
    Punct(char),
}

/// Raggruppa i caratteri di un sorgente con la riga corrente.
struct Cursor {
    chars: Vec<char>,
    pos: usize,
    line: usize,
}

impl Cursor {
    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.pos + ahead).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek(0)?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
        }
        Some(c)
    }

    fn skip_line_comment(&mut self) {
        while self.peek(0).is_some_and(|c| c != '\n') {
            self.bump();
        }
    }

    /// Commento a blocchi, anche annidato.
    fn skip_block_comment(&mut self) {
        let mut depth = 0usize;
        while let Some(c) = self.bump() {
            match (c, self.peek(0)) {
                ('/', Some('*')) => {
                    self.bump();
                    depth += 1;
                }
                ('*', Some('/')) => {
                    self.bump();
                    depth -= 1;
                    if depth == 0 {
                        return;
                    }
                }
                _ => {}
            }
        }
    }

    /// Stringa normale: il cursore e' sul primo carattere dopo l'apice iniziale.
    fn read_string(&mut self) -> String {
        let mut text = String::new();
        while let Some(c) = self.bump() {
            match c {
                '"' => break,
                '\\' => {
                    text.push(c);
                    text.extend(self.bump());
                }
                _ => text.push(c),
            }
        }
        text
    }

    /// Stringa raw con `hashes` cancelletti: il cursore e' dopo l'apice iniziale.
    fn read_raw_string(&mut self, hashes: usize) -> String {
        let mut text = String::new();
        while let Some(c) = self.bump() {
            if c == '"' && (0..hashes).all(|n| self.peek(n) == Some('#')) {
                self.pos += hashes;
                break;
            }
            text.push(c);
        }
        text
    }

    /// `r"..."`, `r#"..."#`, `br"..."`: se dopo il prefisso c'e' una stringa
    /// raw la legge (e ritorna il contenuto), altrimenti non consuma nulla.
    fn try_raw_string(&mut self) -> Option<String> {
        let hashes = (0..).take_while(|&n| self.peek(n) == Some('#')).count();
        if self.peek(hashes) != Some('"') {
            return None;
        }
        self.pos += hashes + 1;
        Some(self.read_raw_string(hashes))
    }

    /// Dopo un `'`: un carattere letterale viene saltato, una lifetime no
    /// (il suo nome si legge poi come identificatore, innocuo).
    fn skip_char_literal(&mut self) {
        let closes_after_one = self.peek(1) == Some('\'');
        if self.peek(0) == Some('\\') {
            self.bump();
            self.bump();
            while self.peek(0).is_some_and(|c| c != '\'') {
                self.bump();
            }
            self.bump();
        } else if closes_after_one {
            self.pos += 2;
        }
    }
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn lex(source: &str) -> Vec<Lexeme> {
    let mut cursor = Cursor { chars: source.chars().collect(), pos: 0, line: 1 };
    let mut out = Vec::new();
    while let Some(c) = cursor.peek(0) {
        let line = cursor.line;
        let token = |kind, text| Lexeme::Token(Token { kind, text, line });
        match c {
            _ if c.is_whitespace() => {
                cursor.bump();
            }
            '/' if cursor.peek(1) == Some('/') => cursor.skip_line_comment(),
            '/' if cursor.peek(1) == Some('*') => cursor.skip_block_comment(),
            '"' => {
                cursor.bump();
                let text = cursor.read_string();
                out.push(token(TokenKind::Str, text));
            }
            '\'' => {
                cursor.bump();
                cursor.skip_char_literal();
            }
            _ if c.is_ascii_digit() => {
                while cursor.peek(0).is_some_and(is_ident_char) {
                    cursor.bump();
                }
            }
            _ if is_ident_char(c) => {
                let mut name = String::new();
                while let Some(n) = cursor.peek(0).filter(|&n| is_ident_char(n)) {
                    name.push(n);
                    cursor.bump();
                }
                match matches!(name.as_str(), "r" | "br" | "cr").then(|| cursor.try_raw_string()).flatten() {
                    Some(text) => out.push(token(TokenKind::Str, text)),
                    None => out.push(token(TokenKind::Ident, name)),
                }
            }
            _ => {
                cursor.bump();
                out.push(Lexeme::Punct(c));
            }
        }
    }
    out
}

fn is_ident(lexeme: &Lexeme, name: &str) -> bool {
    matches!(lexeme, Lexeme::Token(t) if t.kind == TokenKind::Ident && t.text == name)
}

fn is_punct(lexeme: &Lexeme, c: char) -> bool {
    matches!(lexeme, Lexeme::Punct(p) if *p == c)
}

/// Fine dell'attributo che inizia a `at` (`#`, `#!` opzionale, `[...]`), come
/// indice del primo lessema dopo la `]`; `None` se non e' un attributo.
fn attribute_end(lexemes: &[Lexeme], at: usize) -> Option<usize> {
    let open = at + 1 + usize::from(lexemes.get(at + 1).is_some_and(|l| is_punct(l, '!')));
    if !lexemes.get(open).is_some_and(|l| is_punct(l, '[')) {
        return None;
    }
    let mut depth = 0usize;
    for (offset, lexeme) in lexemes[open..].iter().enumerate() {
        if is_punct(lexeme, '[') {
            depth += 1;
        } else if is_punct(lexeme, ']') {
            depth -= 1;
            if depth == 0 {
                return Some(open + offset + 1);
            }
        }
    }
    None
}

/// `true` per `#[test]` e `#[cfg(test)]` (non per `cfg(not(test))`).
fn is_test_attribute(lexemes: &[Lexeme], start: usize, end: usize) -> bool {
    let Some(body) = lexemes.get(start..end) else { return false };
    let names: Vec<&str> = body
        .iter()
        .filter_map(|l| match l {
            Lexeme::Token(t) if t.kind == TokenKind::Ident => Some(t.text.as_str()),
            Lexeme::Punct(_) => None,
            Lexeme::Token(_) => Some(""),
        })
        .collect();
    matches!(names.as_slice(), ["test"] | ["cfg", "test"])
}

/// Indice del primo lessema dopo l'elemento che inizia a `from`: finisce al
/// `;` o alla `}` che chiudono il livello di partenza.
fn item_end(lexemes: &[Lexeme], from: usize) -> usize {
    let mut depth = 0isize;
    for (offset, lexeme) in lexemes[from..].iter().enumerate() {
        let Lexeme::Punct(c) = lexeme else { continue };
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' => depth -= 1,
            '}' => {
                depth -= 1;
                if depth < 0 {
                    return from + offset;
                }
                if depth == 0 {
                    return from + offset + 1;
                }
            }
            ';' if depth == 0 => return from + offset + 1,
            _ => {}
        }
        if depth < 0 {
            return from + offset;
        }
    }
    lexemes.len()
}

/// Nome del modulo se l'elemento e' `[pub[(..)]] mod nome;`.
fn declared_module(item: &[Lexeme]) -> Option<String> {
    let mut rest = item;
    if rest.first().is_some_and(|l| is_ident(l, "pub")) {
        rest = &rest[1..];
        if rest.first().is_some_and(|l| is_punct(l, '(')) {
            let close = rest.iter().position(|l| is_punct(l, ')'))?;
            rest = &rest[close + 1..];
        }
    }
    match rest {
        [m, Lexeme::Token(name), semi] if is_ident(m, "mod") && is_punct(semi, ';') => Some(name.text.clone()),
        _ => None,
    }
}

/// Legge `source` togliendo commenti ed elementi di test.
pub fn scan(source: &str) -> Scan {
    let lexemes = lex(source);
    let mut scan = Scan::default();
    let mut i = 0;
    while i < lexemes.len() {
        if !is_punct(&lexemes[i], '#') {
            if let Lexeme::Token(token) = &lexemes[i] {
                scan.tokens.push(token.clone());
            }
            i += 1;
            continue;
        }
        let Some(end) = attribute_end(&lexemes, i) else {
            i += 1;
            continue;
        };
        let body_start = i + 2 + usize::from(lexemes.get(i + 1).is_some_and(|l| is_punct(l, '!')));
        if !is_test_attribute(&lexemes, body_start, end - 1) {
            i = end;
            continue;
        }
        // Un `#![cfg(test)]` interno rende di test l'intero file.
        if is_punct(&lexemes[i + 1], '!') {
            return Scan::default();
        }
        let mut item_start = end;
        while lexemes.get(item_start).is_some_and(|l| is_punct(l, '#')) {
            let Some(next) = attribute_end(&lexemes, item_start) else { break };
            item_start = next;
        }
        let item_stop = item_end(&lexemes, item_start);
        scan.test_modules.extend(declared_module(&lexemes[item_start..item_stop]));
        i = item_stop;
    }
    scan
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(scan: &Scan) -> Vec<(TokenKind, &str)> {
        scan.tokens.iter().map(|t| (t.kind, t.text.as_str())).collect()
    }

    #[test]
    fn comments_are_ignored_and_strings_are_kept() {
        let scan = scan("// \"a\"\n/* \"b\" /* \"c\" */ */\n/// \"d\"\nlet x = \"e\"; // \"f\"\n");
        assert_eq!(texts(&scan), vec![(TokenKind::Ident, "let"), (TokenKind::Ident, "x"), (TokenKind::Str, "e")]);
    }

    #[test]
    fn strings_chars_and_lifetimes_do_not_confuse_the_reader() {
        let scan = scan(r##"fn f<'a>(x: &'a str) { let c = '"'; let d = '\''; let s = "a\"//b"; let r = r#"x"y"#; }"##);
        let strings: Vec<_> = scan.tokens.iter().filter(|t| t.kind == TokenKind::Str).map(|t| &t.text[..]).collect();
        assert_eq!(strings, vec![r#"a\"//b"#, r#"x"y"#]);
    }

    #[test]
    fn test_items_are_removed_and_test_modules_recorded() {
        let source = r#"
            use a::b;
            #[cfg(test)]
            mod tests { fn x() { let s = "{"; let _ = "claude-code"; } }
            #[cfg(test)]
            pub(crate) mod testing;
            #[cfg(test)]
            const ALL: [u8; 2] = [1, 2];
            #[test]
            fn t() { let _ = "dentro"; }
            #[cfg_attr(not(test), expect(dead_code))]
            fn kept() { let _ = "fuori"; }
        "#;
        let scan = scan(source);
        assert_eq!(scan.test_modules, vec!["testing".to_string()]);
        let strings: Vec<_> = scan.tokens.iter().filter(|t| t.kind == TokenKind::Str).map(|t| &t.text[..]).collect();
        assert_eq!(strings, vec!["fuori"]);
        assert!(scan.tokens.iter().any(|t| t.text == "kept"));
        assert!(!scan.tokens.iter().any(|t| t.text == "tests" || t.text == "ALL"));
    }

    #[test]
    fn an_inner_cfg_test_attribute_skips_the_whole_file() {
        assert_eq!(scan("#![cfg(test)]\nfn x() { let _ = \"claude\"; }"), Scan::default());
    }

    #[test]
    fn lines_are_one_based_and_survive_comments() {
        let scan = scan("a\n/* x\ny */\nb \"c\"");
        let lines: Vec<_> = scan.tokens.iter().map(|t| (t.text.as_str(), t.line)).collect();
        assert_eq!(lines, vec![("a", 1), ("b", 4), ("c", 4)]);
    }
}
