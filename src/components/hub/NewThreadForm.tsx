import { useState } from "react";
import type { FormEvent } from "react";
import { createChat } from "../../app/actions/hubMessenger";
import { errorMessage } from "../../lib/errors";
import { THREAD_TITLE_MAX_CHARS, checkTitle } from "../../lib/hub";
import type { HubChannel } from "../../lib/hub";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import ChannelSelect from "./ChannelSelect";

interface Props {
  /** Canale del thread; con `channels` è quello proposto, cambiabile. */
  channel: string;
  /** Se c'è, il modulo lascia scegliere il workspace tra questi canali (vista messenger). */
  channels?: readonly HubChannel[];
  onDone: () => void;
}

/** Titolo del nuovo thread (e, nella vista messenger, il suo workspace); si chiude da solo a creazione riuscita. */
export default function NewThreadForm({ channel, channels, onDone }: Props) {
  const [title, setTitle] = useState("");
  const [target, setTarget] = useState(channel);
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
      await createChat(target, checked.body);
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
      {channels && (
        <ChannelSelect
          label="Workspace del thread"
          channels={channels}
          value={target}
          disabled={saving}
          onChange={setTarget}
          className="h-8 text-[13px]"
        />
      )}
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
