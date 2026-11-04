//! Le menzioni `@nome` nel testo di un messaggio (puro). Una menzione inizia
//! con `@` a inizio testo o dopo un carattere che non puo' stare in un nome (cosi'
//! `posta@mario.it` non e' una menzione) e prosegue con lettere, cifre e `-`
//! fino al primo altro carattere; il `-` finale non conta. Il nome si confronta
//! in minuscolo, come lo salva `settings::secretary::normalize_name`.

use crate::hub::settings::normalize_name;

/// I nomi menzionati, senza ripetizioni, nell'ordine della prima comparsa.
/// Un token che non puo' essere il nome di un segretario (troppo lungo) si salta.
pub fn mentions(body: &str) -> Vec<String> {
    let chars: Vec<char> = body.chars().collect();
    let mut found: Vec<String> = Vec::new();
    for (at, c) in chars.iter().enumerate() {
        if *c != '@' || at.checked_sub(1).is_some_and(|prev| continues_token(chars[prev])) {
            continue;
        }
        let token: String = chars[at + 1..].iter().take_while(|c| is_name_char(**c)).collect();
        // `@màrio`, `@mario_1`: il token prosegue con caratteri che un nome non ha.
        if chars.get(at + 1 + token.chars().count()).is_some_and(|next| next.is_alphanumeric() || *next == '_') {
            continue;
        }
        let Ok(name) = normalize_name(token.trim_end_matches('-')) else {
            continue;
        };
        if !found.contains(&name) {
            found.push(name);
        }
    }
    found
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-'
}

/// Un `@` dopo uno di questi caratteri fa parte di un'altra parola (un
/// indirizzo email, `@@mario`).
fn continues_token(prev: char) -> bool {
    prev.is_alphanumeric() || matches!(prev, '-' | '_' | '.' | '@')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mentions_are_found_in_order_without_repeats() {
        assert_eq!(mentions("@mario a che punto siamo?"), ["mario"]);
        assert_eq!(mentions("ciao @Giorgio e @mario, poi @giorgio"), ["giorgio", "mario"]);
        assert_eq!(mentions("(@mario) @bot-2: ok"), ["mario", "bot-2"]);
        assert_eq!(mentions("@mario-, @7"), ["mario", "7"]);
        assert_eq!(mentions("riga uno\n@mario\tok"), ["mario"]);
    }

    #[test]
    fn non_mentions_are_ignored() {
        assert!(mentions("nessuna menzione").is_empty());
        assert!(mentions("scrivi a posta@mario.it").is_empty());
        assert!(mentions("@ mario, @, @-x, @@mario, x_@mario").is_empty());
        assert!(mentions(&format!("@{}", "a".repeat(25))).is_empty());
        assert!(mentions("@màrio @mario_1").is_empty());
    }

    #[test]
    fn unicode_text_around_mentions_is_fine() {
        assert_eq!(mentions("è pronto? «@mario» 🙂"), ["mario"]);
    }
}
