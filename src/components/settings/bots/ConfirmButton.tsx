import { useState } from "react";
import { Button } from "../../ui/button";

interface Props {
  label: string;
  /** Domanda mostrata mentre si attende la conferma. */
  question: string;
  confirmLabel: string;
  disabled: boolean;
  destructive?: boolean;
  onConfirm: () => void;
}

/** Bottone che chiede conferma sul posto prima di eseguire l'azione. */
export default function ConfirmButton({ label, question, confirmLabel, disabled, destructive, onConfirm }: Props) {
  const [confirming, setConfirming] = useState(false);

  if (!confirming) {
    return (
      <Button type="button" size="sm" variant="outline" disabled={disabled} onClick={() => setConfirming(true)}>
        {label}
      </Button>
    );
  }
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="text-[11px] text-muted-foreground">{question}</span>
      <Button
        type="button"
        size="sm"
        variant={destructive ? "destructive" : "default"}
        disabled={disabled}
        onClick={() => {
          setConfirming(false);
          onConfirm();
        }}
      >
        {confirmLabel}
      </Button>
      <Button type="button" size="sm" variant="outline" onClick={() => setConfirming(false)}>
        Annulla
      </Button>
    </div>
  );
}
