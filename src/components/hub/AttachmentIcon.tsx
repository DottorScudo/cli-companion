import { File, FileImage, FileText } from "lucide-react";

const TEXT_MIMES: ReadonlySet<string> = new Set(["application/pdf", "application/json", "application/xml"]);

/** Icona di un allegato secondo il tipo: immagine, documento di testo, file generico. */
export default function AttachmentIcon({ mime, className }: { mime: string | null; className?: string }) {
  if (mime?.startsWith("image/")) return <FileImage aria-hidden className={className} />;
  if (mime && (mime.startsWith("text/") || TEXT_MIMES.has(mime))) {
    return <FileText aria-hidden className={className} />;
  }
  return <File aria-hidden className={className} />;
}
