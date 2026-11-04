import { useState } from "react";
import type { FormEvent } from "react";
import { createThread } from "../../app/actions/hub";
import { errorMessage } from "../../lib/errors";
import { THREAD_TITLE_MAX_CHARS, checkTitle } from "../../lib/hub";
import { Button } from "../ui/button";
import { Input } from "../ui/input";

interface Props {
  channel: string;
  onDone: () => void;
}

/** Titolo del nuovo thread del canale; si chiude da solo a creazione riuscita. */
export default function NewThreadForm({ channel, onDone }: Props) {
  const [title, setTitle] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    const checked = checkTitle(title);
    if (!checked.ok) {
      setError(checked.reason);
      return;
    }
    setError(null);
    setSaving(true);
    try {
      await createThread(channel, checked.body);
      onDone();
    } catch (err) {
      setError(errorMessage(err));
      setSaving(false);
    }
  };

  return (
    <form onSubmit={(e) => void submit(e)} className="flex flex-col gap-1.5 border-b p-2">
      <Input
        autoFocus
        value={title}
        maxLength={THREAD_TITLE_MAX_CHARS}
        disabled={saving}
        onChange={(e) => setTitle(e.target.value)}
        placeholder="Titolo del thread"
        aria-label="Titolo del thread"
        className="h-8"
      />
      {error && <p className="text-[11px] text-destructive">{error}</p>}
      <div className="flex gap-2">
        <Button type="submit" size="sm" disabled={saving || title.trim() === ""}>
          Crea
        </Button>
        <Button type="button" size="sm" variant="outline" disabled={saving} onClick={onDone}>
          Annulla
        </Button>
      </div>
    </form>
  );
}
