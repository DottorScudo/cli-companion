//! Il testo formattato come lo intende Telegram (puro): tratti di testo visibile, ognuno
//! con la pila dei tag che lo avvolgono, e il loro HTML. Solo i tag ammessi dalla Bot
//! API; l'escape copre `&`, `<`, `>` nel testo e anche `"` negli attributi. L'HTML si
//! scrive da una sequenza di tratti qualsiasi (anche tagliati a meta'), chiudendo e
//! riaprendo i tag fra un tratto e l'altro: e' sempre ben formato.

/// Un tag HTML della Bot API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tag {
    Bold,
    Italic,
    Strike,
    Code,
    /// Un blocco di codice, col linguaggio gia' ripulito (`<pre><code class="language-…">`).
    Pre {
        language: Option<String>,
    },
    /// Un link con un indirizzo gia' controllato.
    Link {
        href: String,
    },
    Quote,
}

impl Tag {
    fn open(&self, out: &mut String) {
        match self {
            Self::Bold => out.push_str("<b>"),
            Self::Italic => out.push_str("<i>"),
            Self::Strike => out.push_str("<s>"),
            Self::Code => out.push_str("<code>"),
            Self::Pre { language: None } => out.push_str("<pre>"),
            Self::Pre { language: Some(language) } => {
                out.push_str("<pre><code class=\"language-");
                escape_attribute(language, out);
                out.push_str("\">");
            }
            Self::Link { href } => {
                out.push_str("<a href=\"");
                escape_attribute(href, out);
                out.push_str("\">");
            }
            Self::Quote => out.push_str("<blockquote>"),
        }
    }

    fn close(&self, out: &mut String) {
        out.push_str(match self {
            Self::Bold => "</b>",
            Self::Italic => "</i>",
            Self::Strike => "</s>",
            Self::Code => "</code>",
            Self::Pre { language: None } => "</pre>",
            Self::Pre { language: Some(_) } => "</code></pre>",
            Self::Link { .. } => "</a>",
            Self::Quote => "</blockquote>",
        });
    }
}

/// Un tratto di testo visibile e i tag che lo avvolgono, dal piu' esterno.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub tags: Vec<Tag>,
    pub text: String,
}

/// Il testo con `&`, `<` e `>` come entita'.
pub fn escape_text(text: &str, out: &mut String) {
    escape(text, false, out);
}

/// Il valore di un attributo fra virgolette: come il testo, piu' `"`.
fn escape_attribute(value: &str, out: &mut String) {
    escape(value, true, out);
}

fn escape(text: &str, attribute: bool, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attribute => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
}

/// L'HTML di una sequenza di tratti `(tag, testo)`: fra due tratti si chiudono i tag
/// che non continuano e si aprono i nuovi; alla fine si chiude tutto. I tratti vuoti
/// non aprono nulla (Telegram non vuole entita' vuote).
pub fn html<'a>(pieces: impl IntoIterator<Item = (&'a [Tag], &'a str)>) -> String {
    let mut out = String::new();
    let mut open: &[Tag] = &[];
    for (tags, text) in pieces.into_iter().filter(|(_, text)| !text.is_empty()) {
        let common = open.iter().zip(tags).take_while(|(a, b)| a == b).count();
        open[common..].iter().rev().for_each(|tag| tag.close(&mut out));
        tags[common..].iter().for_each(|tag| tag.open(&mut out));
        escape_text(text, &mut out);
        open = tags;
    }
    open.iter().rev().for_each(|tag| tag.close(&mut out));
    out
}

#[cfg(test)]
mod tests;
