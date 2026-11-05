//! Il tipo MIME di un allegato: serve alla UI per decidere l'anteprima, non
//! filtra nulla (si allega qualsiasi file). Prima le firme dei formati di
//! immagine e del PDF nei primi byte, poi l'estensione. Puro.

/// Il tipo di un file che non si riconosce.
pub const UNKNOWN: &str = "application/octet-stream";

/// Byte iniziali che bastano per riconoscere le firme di `SIGNATURES`.
pub const HEAD_LEN: usize = 16;

/// Firme nei primi byte: `(scostamento, byte, tipo)`.
const SIGNATURES: [(usize, &[u8], &str); 7] = [
    (0, b"\x89PNG\r\n\x1a\n", "image/png"),
    (0, b"\xFF\xD8\xFF", "image/jpeg"),
    (0, b"GIF87a", "image/gif"),
    (0, b"GIF89a", "image/gif"),
    (8, b"WEBP", "image/webp"),
    (0, b"%PDF-", "application/pdf"),
    (4, b"ftypavif", "image/avif"),
];

/// Estensioni note, in minuscolo.
const EXTENSIONS: [(&str, &str); 37] = [
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("bmp", "image/bmp"),
    ("avif", "image/avif"),
    ("svg", "image/svg+xml"),
    ("ico", "image/x-icon"),
    ("pdf", "application/pdf"),
    ("txt", "text/plain"),
    ("log", "text/plain"),
    ("md", "text/markdown"),
    ("csv", "text/csv"),
    ("html", "text/html"),
    ("htm", "text/html"),
    ("css", "text/css"),
    ("xml", "application/xml"),
    ("json", "application/json"),
    ("yaml", "application/yaml"),
    ("yml", "application/yaml"),
    ("toml", "application/toml"),
    ("js", "text/javascript"),
    ("ts", "text/plain"),
    ("rs", "text/plain"),
    ("py", "text/x-python"),
    ("sh", "application/x-sh"),
    ("zip", "application/zip"),
    ("gz", "application/gzip"),
    ("tar", "application/x-tar"),
    ("7z", "application/x-7z-compressed"),
    ("docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
    ("xlsx", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
    ("pptx", "application/vnd.openxmlformats-officedocument.presentationml.presentation"),
    ("mp3", "audio/mpeg"),
    ("mp4", "video/mp4"),
    ("wav", "audio/wav"),
];

/// Il tipo del file `name` che inizia con `head` (i primi `HEAD_LEN` byte, o
/// meno se il file e' piu' corto).
pub fn detect(name: &str, head: &[u8]) -> &'static str {
    by_signature(head).or_else(|| by_extension(name)).unwrap_or(UNKNOWN)
}

fn by_signature(head: &[u8]) -> Option<&'static str> {
    SIGNATURES.iter().find(|(offset, magic, _)| head.get(*offset..).is_some_and(|h| h.starts_with(magic))).map(|s| s.2)
}

fn by_extension(name: &str) -> Option<&'static str> {
    let (_, ext) = name.rsplit_once('.')?;
    EXTENSIONS.iter().find(|(known, _)| known.eq_ignore_ascii_case(ext)).map(|e| e.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures_win_over_the_extension() {
        assert_eq!(detect("incollata.bin", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR"), "image/png");
        assert_eq!(detect("foto.txt", b"\xFF\xD8\xFF\xE0"), "image/jpeg");
        assert_eq!(detect("x", b"RIFF\0\0\0\0WEBPVP8 "), "image/webp");
        assert_eq!(detect("x", b"\0\0\0\x1cftypavif"), "image/avif");
        assert_eq!(detect("doc", b"%PDF-1.7"), "application/pdf");
    }

    #[test]
    fn the_extension_decides_without_a_signature() {
        assert_eq!(detect("NOTE.MD", b"# titolo"), "text/markdown");
        assert_eq!(detect("disegno.svg", b"<svg"), "image/svg+xml");
        assert_eq!(detect("archivio.tar.gz", b""), "application/gzip");
    }

    #[test]
    fn unknown_files_are_octet_streams() {
        assert_eq!(detect("eseguibile", b"MZ\x90\0"), UNKNOWN);
        assert_eq!(detect("dati.xyz", b""), UNKNOWN);
        assert_eq!(detect(".env", b"A=1"), UNKNOWN);
    }
}
