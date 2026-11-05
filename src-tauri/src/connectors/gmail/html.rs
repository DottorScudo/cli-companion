//! Dall'HTML al testo (puro, a caratteri, mai in panico): per le email che hanno solo la
//! parte `text/html`. Non e' un parser HTML completo: toglie i tag, tiene i
//! a capo dei blocchi, scarta `script`, `style`, `head` e commenti, decodifica le entita'.

/// Caratteri di HTML elaborati al massimo: il resto si ignora.
pub const HTML_CHARS_MAX: usize = 500_000;
/// Tag il cui contenuto non e' testo da leggere.
const SKIPPED_TAGS: [&str; 3] = ["script", "style", "head"];
/// Tag che, in apertura e in chiusura, vanno a capo.
const BLOCK_TAGS: [&str; 15] =
    ["br", "p", "div", "tr", "table", "ul", "ol", "h1", "h2", "h3", "h4", "h5", "h6", "blockquote", "hr"];
/// Tag che separano le celle con uno spazio.
const CELL_TAGS: [&str; 2] = ["td", "th"];
/// Lunghezza massima del nome di un'entita' (`&nbsp;` = 4) o di un numero.
const ENTITY_MAX: usize = 10;
/// Lunghezza massima, in caratteri, di un tag: oltre non e' un tag (limita la scansione).
const TAG_MAX: usize = 4096;
/// Il segno che apre un elemento di elenco.
const LIST_BULLET: &str = "\n- ";

/// Un tag letto: nome in minuscolo, se chiude, e dove finisce (indice dopo `>`).
struct Tag {
    name: String,
    closing: bool,
    self_closing: bool,
    end: usize,
}

fn starts_with_at(chars: &[char], at: usize, pattern: &str) -> bool {
    pattern.chars().enumerate().all(|(n, p)| chars.get(at + n).is_some_and(|c| c.eq_ignore_ascii_case(&p)))
}

/// L'indice dopo il primo `needle` a partire da `from`, o la fine del testo.
fn after(chars: &[char], from: usize, needle: &str) -> usize {
    (from..chars.len()).find(|&i| starts_with_at(chars, i, needle)).map_or(chars.len(), |i| i + needle.chars().count())
}

/// Il tag che inizia a `at` (dove c'e' `<`), se e' davvero un tag.
fn read_tag(chars: &[char], at: usize) -> Option<Tag> {
    let first = *chars.get(at + 1)?;
    if !(first.is_ascii_alphabetic() || first == '/') {
        return None;
    }
    let closing = first == '/';
    let name_start = if closing { at + 2 } else { at + 1 };
    let name: String = chars
        .get(name_start..)?
        .iter()
        .take_while(|c| c.is_ascii_alphanumeric())
        .map(char::to_ascii_lowercase)
        .collect();
    let mut quote: Option<char> = None;
    let mut end = at + 1;
    while let Some(&c) = chars.get(end).filter(|_| end - at <= TAG_MAX) {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(c),
            (None, '>') => {
                let self_closing = end > 0 && chars.get(end - 1) == Some(&'/');
                return Some(Tag { name, closing, self_closing, end: end + 1 });
            }
            _ => {}
        }
        end += 1;
    }
    None
}

/// Cosa scrivere per un tag, e dove riprendere (saltando il contenuto di `script`, `style`, `head`).
fn handle_tag(chars: &[char], tag: &Tag, out: &mut String) -> usize {
    let name = tag.name.as_str();
    if !tag.closing && !tag.self_closing && SKIPPED_TAGS.contains(&name) {
        return after_closing(chars, tag.end, name);
    }
    if name == "li" {
        if !tag.closing {
            out.push_str(LIST_BULLET);
        }
    } else if BLOCK_TAGS.contains(&name) {
        out.push('\n');
    } else if CELL_TAGS.contains(&name) {
        out.push(' ');
    }
    tag.end
}

/// Dopo il tag di chiusura `</name ...>` che segue `from`, o la fine del testo.
fn after_closing(chars: &[char], from: usize, name: &str) -> usize {
    let closing = format!("</{name}");
    let at = (from..chars.len()).find(|&i| starts_with_at(chars, i, &closing));
    at.and_then(|i| read_tag(chars, i)).map_or(chars.len(), |tag| tag.end)
}

/// La `<` a `at`: un commento, un tag o un semplice carattere. Ritorna dove riprendere.
fn handle_markup(chars: &[char], at: usize, out: &mut String) -> usize {
    if starts_with_at(chars, at, "<!--") {
        return after(chars, at + 4, "-->");
    }
    if starts_with_at(chars, at, "<!") || starts_with_at(chars, at, "<?") {
        return after(chars, at + 2, ">");
    }
    match read_tag(chars, at) {
        Some(tag) => handle_tag(chars, &tag, out),
        None => {
            out.push('<');
            at + 1
        }
    }
}

/// Il carattere di un'entita' con nome o numerica (senza `&` e `;`).
fn entity_char(body: &str) -> Option<char> {
    match body {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        _ => {
            let digits = body.strip_prefix('#')?;
            let code = match digits.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => digits.parse::<u32>().ok()?,
            };
            char::from_u32(code).filter(|c| !c.is_control() || c.is_whitespace())
        }
    }
}

/// La `&` a `at`: un'entita' valida o un semplice carattere. Ritorna dove riprendere.
fn handle_entity(chars: &[char], at: usize, out: &mut String) -> usize {
    let window = chars.get(at + 1..).unwrap_or(&[]);
    let semicolon = window.iter().take(ENTITY_MAX + 1).position(|c| *c == ';');
    let decoded = semicolon.and_then(|n| {
        let body: String = window.get(..n)?.iter().collect();
        entity_char(&body).map(|c| (c, n))
    });
    match decoded {
        Some((c, n)) => {
            out.push(c);
            at + n + 2
        }
        None => {
            out.push('&');
            at + 1
        }
    }
}

/// Spazi ripetuti ridotti a uno, righe ripulite, al piu' una riga vuota di seguito.
fn tidy(raw: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in raw.split('\n') {
        let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
        let blank_after_blank = collapsed.is_empty() && lines.last().is_none_or(String::is_empty);
        if !blank_after_blank {
            lines.push(collapsed);
        }
    }
    lines.join("\n").trim().to_string()
}

/// Il testo leggibile di una pagina HTML.
pub fn html_to_text(html: &str) -> String {
    let chars: Vec<char> = html.chars().take(HTML_CHARS_MAX).collect();
    let mut out = String::with_capacity(chars.len() / 2);
    let mut at = 0usize;
    while let Some(&c) = chars.get(at) {
        at = match c {
            '<' => handle_markup(&chars, at, &mut out),
            '&' => handle_entity(&chars, at, &mut out),
            other => {
                out.push(other);
                at + 1
            }
        };
    }
    tidy(&out)
}

#[cfg(test)]
mod tests;
