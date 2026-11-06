//! I valori che finiscono negli attributi dei tag (puri): l'indirizzo di un link, solo
//! con gli schemi che Telegram apre senza sorprese, e il linguaggio di un blocco di
//! codice, ridotto ai caratteri di un nome. Tutto il resto resta testo.

/// Gli schemi di un link cliccabile (confrontati in minuscolo).
const LINK_SCHEMES: [&str; 4] = ["http", "https", "mailto", "tg"];
/// Gli schemi che vogliono un `//host` dopo i due punti.
const SCHEMES_WITH_HOST: [&str; 3] = ["http", "https", "tg"];
/// Caratteri al massimo del nome di un linguaggio.
const LANGUAGE_MAX: usize = 32;

/// `target` puo' diventare un `<a href>`? Solo uno schema ammesso, con qualcosa dopo
/// (un host per quelli web), e mai spazi o caratteri di controllo.
pub fn link_allowed(target: &str) -> bool {
    if target.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    let Some((scheme, rest)) = target.split_once(':') else { return false };
    let scheme = scheme.to_ascii_lowercase();
    if !LINK_SCHEMES.contains(&scheme.as_str()) {
        return false;
    }
    if SCHEMES_WITH_HOST.contains(&scheme.as_str()) {
        rest.strip_prefix("//").is_some_and(|host| !host.is_empty() && !host.starts_with('/'))
    } else {
        !rest.is_empty()
    }
}

/// Il linguaggio di un blocco recintato: la prima parola dell'info string ridotta a
/// `[A-Za-z0-9_+-]` (al piu' `LANGUAGE_MAX` caratteri). `None` se non resta nulla.
pub fn code_language(info: &str) -> Option<String> {
    let word = info.split_whitespace().next()?;
    let language: String =
        word.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-')).take(LANGUAGE_MAX).collect();
    (!language.is_empty()).then_some(language)
}

#[cfg(test)]
mod tests;
