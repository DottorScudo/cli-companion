//! Il controllo dei valori che finiscono in un URL, in un'intestazione o in un corpo.

/// `value` e' fatto solo di caratteri ASCII visibili (niente spazi ne' controlli) ed e' lungo 1..=`max`.
pub fn is_visible_ascii(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && value.bytes().all(|b| b.is_ascii_graphic())
}

#[cfg(test)]
mod tests;
