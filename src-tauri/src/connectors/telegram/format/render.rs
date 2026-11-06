//! Il markdown come tratti di testo visibile con i tag di Telegram (puro). Un renderer
//! proprio sopra gli eventi di `pulldown_cmark` (CommonMark piu' barrato, tabelle e
//! task list di GFM): Telegram non ha tag per titoli, elenchi, tabelle e righe, che
//! diventano righe di testo (grassetto, «• », «n. », un blocco `<pre>`, «———»).
//! L'HTML grezzo resta testo. Fra due blocchi va una riga vuota se c'era nel testo
//! d'origine, altrimenti un solo a capo: un testo semplice esce con le stesse righe.
//! Niente tag dentro codice, tabelle e link (Telegram non li annida).

mod spacing;

use super::attributes::{code_language, link_allowed};
use super::markup::{Run, Tag};
use pulldown_cmark::{CodeBlockKind, Event, LinkType, Options, Parser, Tag as Markdown};
use spacing::{blank_line_between, content_end};
use std::ops::Range;

const BULLET: &str = "• ";
const TASK_OPEN: &str = "☐ ";
const TASK_DONE: &str = "☑ ";
/// Il rientro di un sotto-elenco, per livello.
const NESTED_INDENT: &str = "  ";
const RULE: &str = "———";
const CELL_SEPARATOR: &str = " | ";
const BLANK_LINE: usize = 2;
const LINE_BREAK: usize = 1;

fn options() -> Options {
    Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS
}

/// I tratti di `markdown`, in ordine. Mai in panico, qualunque sia l'input.
pub fn runs(markdown: &str) -> Vec<Run> {
    let mut renderer = Renderer::new(markdown);
    for (event, range) in Parser::new_ext(markdown, options()).into_offset_iter() {
        renderer.event(event, range);
    }
    renderer.runs
}

/// Cosa chiudere alla fine di un elemento.
enum Frame {
    /// Niente.
    Plain,
    /// Il tag in cima ad `active`.
    Styled,
    Link,
    Item,
    List,
    CodeBlock(Option<String>),
    /// Una tabella; `styled` se ha aperto il suo `<pre>`.
    Table {
        styled: bool,
    },
}

/// Un link aperto: il suo testo si raccoglie per decidere cosa mostrare alla fine.
struct OpenLink {
    /// L'indirizzo come scritto nel testo.
    shown: String,
    allowed: bool,
    text: String,
}

/// Il segno di una voce d'elenco, scritto davanti al primo contenuto della voce.
struct ItemPrefix {
    depth: usize,
    marker: String,
    bullet: bool,
}

#[derive(Default)]
struct OpenTable {
    rows: usize,
    cells: usize,
}

struct Renderer<'s> {
    source: &'s str,
    runs: Vec<Run>,
    /// I tag che avvolgono il prossimo testo, dal piu' esterno.
    active: Vec<Tag>,
    frames: Vec<Frame>,
    /// Il prossimo numero di ogni elenco aperto (`None` per quelli puntati).
    lists: Vec<Option<u64>>,
    item: Option<ItemPrefix>,
    link: Option<OpenLink>,
    /// Il contenuto del blocco di codice aperto.
    code: Option<String>,
    table: Option<OpenTable>,
    /// Gli a capo in fondo al testo scritto finora.
    trailing_newlines: usize,
    /// Dove finisce, nel testo d'origine, l'ultimo contenuto letto.
    last_end: usize,
}

impl<'s> Renderer<'s> {
    fn new(source: &'s str) -> Self {
        Self {
            source,
            runs: Vec::new(),
            active: Vec::new(),
            frames: Vec::new(),
            lists: Vec::new(),
            item: None,
            link: None,
            code: None,
            table: None,
            trailing_newlines: 0,
            last_end: 0,
        }
    }

    fn event(&mut self, event: Event<'_>, range: Range<usize>) {
        match event {
            Event::Start(tag) => {
                let frame = self.start(tag, range.start);
                self.frames.push(frame);
            }
            Event::End(_) => {
                if let Some(frame) = self.frames.pop() {
                    self.end(frame);
                }
            }
            leaf => {
                self.leaf(leaf, range.start);
                self.last_end = content_end(self.source, range);
            }
        }
    }

    /// Un evento senza figli: testo, codice, HTML grezzo, a capo, riga, segno di una task list.
    fn leaf(&mut self, event: Event<'_>, at: usize) {
        match event {
            Event::Text(text) => self.text(&text),
            Event::Code(code) => self.inline_code(&code),
            Event::Html(raw) | Event::InlineHtml(raw) | Event::InlineMath(raw) | Event::DisplayMath(raw) => {
                self.text(&raw);
            }
            Event::FootnoteReference(label) => self.text(&format!("[{label}]")),
            Event::SoftBreak | Event::HardBreak => self.text("\n"),
            Event::Rule => {
                self.separate(at);
                self.text(RULE);
            }
            Event::TaskListMarker(done) => self.task_marker(done),
            Event::Start(_) | Event::End(_) => {}
        }
    }

    fn start(&mut self, tag: Markdown<'_>, at: usize) -> Frame {
        match tag {
            Markdown::Paragraph | Markdown::HtmlBlock => {
                self.separate(at);
                Frame::Plain
            }
            Markdown::Heading { .. } => {
                self.separate(at);
                self.style(Tag::Bold)
            }
            Markdown::BlockQuote(_) => {
                self.separate(at);
                self.style(Tag::Quote)
            }
            Markdown::CodeBlock(kind) => {
                self.separate(at);
                self.code = Some(String::new());
                Frame::CodeBlock(match kind {
                    CodeBlockKind::Fenced(info) => code_language(&info),
                    CodeBlockKind::Indented => None,
                })
            }
            Markdown::List(first) => {
                // Un sotto-elenco va a capo dopo il segno della voce che lo contiene.
                self.flush_item_prefix();
                self.separate(at);
                self.lists.push(first);
                Frame::List
            }
            Markdown::Item => self.start_item(at),
            Markdown::Table(_) => self.start_table(at),
            Markdown::TableHead | Markdown::TableRow => {
                self.start_row();
                Frame::Plain
            }
            Markdown::TableCell => {
                self.start_cell();
                Frame::Plain
            }
            Markdown::Emphasis => self.style(Tag::Italic),
            Markdown::Strong => self.style(Tag::Bold),
            Markdown::Strikethrough => self.style(Tag::Strike),
            Markdown::Link { link_type, dest_url, .. } | Markdown::Image { link_type, dest_url, .. } => {
                self.start_link(link_type, &dest_url)
            }
            // Note, definizioni, apici e metadati: le estensioni non sono attive.
            _ => Frame::Plain,
        }
    }

    fn end(&mut self, frame: Frame) {
        match frame {
            Frame::Plain => {}
            Frame::Styled => {
                self.active.pop();
            }
            Frame::Link => self.end_link(),
            // Una voce vuota mostra comunque il suo segno.
            Frame::Item => self.flush_item_prefix(),
            Frame::List => {
                self.lists.pop();
            }
            Frame::CodeBlock(language) => self.end_code_block(language),
            Frame::Table { styled } => {
                self.table = None;
                if styled {
                    self.active.pop();
                }
            }
        }
    }

    /// Formattazione ammessa qui? Non dentro una tabella (un `<pre>`) ne' in un link.
    fn formatting_allowed(&self) -> bool {
        self.table.is_none() && self.link.is_none()
    }

    /// Apre `tag` se e' ammesso e non e' gia' aperto (niente `<b>` dentro `<b>`).
    fn style(&mut self, tag: Tag) -> Frame {
        if !self.formatting_allowed() || self.active.contains(&tag) {
            return Frame::Plain;
        }
        self.enter(tag);
        Frame::Styled
    }

    /// Apre un tag: il segno della voce d'elenco in attesa resta fuori.
    fn enter(&mut self, tag: Tag) {
        self.flush_item_prefix();
        self.active.push(tag);
    }

    /// Prima di un blocco: la riga vuota o l'a capo che c'erano nel testo d'origine.
    /// Il primo blocco di una voce d'elenco resta sulla riga del suo segno.
    fn separate(&mut self, at: usize) {
        if self.item.is_some() || self.runs.is_empty() {
            return;
        }
        let gap = if blank_line_between(self.source, self.last_end, at) { BLANK_LINE } else { LINE_BREAK };
        let missing = gap.saturating_sub(self.trailing_newlines);
        self.push(&"\n".repeat(missing));
    }

    fn start_item(&mut self, at: usize) -> Frame {
        self.separate(at);
        let depth = self.lists.len().saturating_sub(1);
        let (marker, bullet) = match self.lists.last_mut() {
            Some(Some(next)) => {
                let marker = format!("{next}. ");
                *next = next.saturating_add(1);
                (marker, false)
            }
            _ => (BULLET.to_string(), true),
        };
        self.item = Some(ItemPrefix { depth, marker, bullet });
        Frame::Item
    }

    /// `☐`/`☑` al posto del pallino (o dopo il numero) della voce.
    fn task_marker(&mut self, done: bool) {
        let mark = if done { TASK_DONE } else { TASK_OPEN };
        match &mut self.item {
            Some(prefix) if prefix.bullet => prefix.marker = mark.to_string(),
            Some(prefix) => prefix.marker.push_str(mark),
            None => self.text(mark),
        }
    }

    fn flush_item_prefix(&mut self) {
        if let Some(prefix) = self.item.take() {
            self.push(&format!("{}{}", NESTED_INDENT.repeat(prefix.depth), prefix.marker));
        }
    }

    fn start_table(&mut self, at: usize) -> Frame {
        self.separate(at);
        let styled = matches!(self.style(Tag::Pre { language: None }), Frame::Styled);
        self.table = Some(OpenTable::default());
        Frame::Table { styled }
    }

    fn start_row(&mut self) {
        let Some(table) = &mut self.table else { return };
        let first = table.rows == 0;
        table.rows += 1;
        table.cells = 0;
        if !first {
            self.text("\n");
        }
    }

    fn start_cell(&mut self) {
        let Some(table) = &mut self.table else { return };
        let first = table.cells == 0;
        table.cells += 1;
        if !first {
            self.text(CELL_SEPARATOR);
        }
    }

    fn inline_code(&mut self, code: &str) {
        if !self.formatting_allowed() || self.active.contains(&Tag::Code) {
            self.text(code);
        } else {
            self.enter(Tag::Code);
            self.text(code);
            self.active.pop();
        }
    }

    /// Il blocco intero, letterale, senza gli a capo finali; un blocco vuoto non scrive nulla.
    fn end_code_block(&mut self, language: Option<String>) {
        let code = self.code.take().unwrap_or_default();
        let code = code.trim_end_matches('\n');
        if code.is_empty() {
            return;
        }
        self.enter(Tag::Pre { language });
        self.text(code);
        self.active.pop();
    }

    /// Un link (o un'immagine) dentro un altro, o in una tabella, resta testo.
    fn start_link(&mut self, link_type: LinkType, dest_url: &str) -> Frame {
        if !self.formatting_allowed() {
            return Frame::Plain;
        }
        let target = match link_type {
            LinkType::Email => format!("mailto:{dest_url}"),
            _ => dest_url.to_string(),
        };
        let allowed = link_allowed(&target);
        if allowed {
            self.enter(Tag::Link { href: target });
        } else {
            self.flush_item_prefix();
        }
        self.link = Some(OpenLink { shown: dest_url.to_string(), allowed, text: String::new() });
        Frame::Link
    }

    /// Senza testo il link mostra l'indirizzo; se non e' cliccabile, l'indirizzo segue il
    /// testo fra parentesi (quando e' diverso).
    fn end_link(&mut self) {
        let Some(link) = self.link.take() else { return };
        let has_text = !link.text.trim().is_empty();
        if !has_text {
            self.push(&link.shown);
        }
        if link.allowed {
            self.active.pop();
        } else if has_text && link.text != link.shown && !link.shown.is_empty() {
            self.push(&format!(" ({})", link.shown));
        }
    }

    fn text(&mut self, text: &str) {
        if let Some(code) = &mut self.code {
            code.push_str(text);
            return;
        }
        self.flush_item_prefix();
        self.push(text);
    }

    /// Aggiunge `text` con i tag attivi, unendolo al tratto precedente se ha gli stessi.
    fn push(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(link) = &mut self.link {
            link.text.push_str(text);
        }
        let body = text.trim_end_matches('\n');
        let newlines = text.len() - body.len();
        self.trailing_newlines = if body.is_empty() { self.trailing_newlines + newlines } else { newlines };
        match self.runs.last_mut() {
            Some(last) if last.tags == self.active => last.text.push_str(text),
            _ => self.runs.push(Run { tags: self.active.clone(), text: text.to_string() }),
        }
    }
}

#[cfg(test)]
mod tests;
