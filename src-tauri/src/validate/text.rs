//! Testi in ingresso: lunghezza e ritaglio di stringhe, indipendenti dal dominio.

/// Testo obbligatorio: trim, 1..=`max` caratteri.
pub fn req_text(label: &str, s: &str, max: usize) -> Result<String, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err(format!("{label} is missing"));
    }
    if t.chars().count() > max {
        return Err(format!("{label} is too long (max {max} characters)"));
    }
    Ok(t.to_string())
}

/// Primi `max` caratteri di `s` (mai in mezzo a un carattere).
pub fn clip_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}
