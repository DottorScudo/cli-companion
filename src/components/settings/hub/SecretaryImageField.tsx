import { ImageOff, ImagePlus } from "lucide-react";
import type { BotImage } from "../../../lib/bots";
import { Button } from "../../ui/button";
import BotAvatar from "../../BotAvatar";
import BotImagePreview from "../BotImagePreview";
import { BOT_IMAGE_ACCEPT, useBotImagePicker } from "../useBotImagePicker";

const AVATAR_SIZE = 32;
const FALLBACK_NAME = "segretario";

interface Props {
  /** Nome in corso di scrittura: dà l'aspetto dell'avatar senza immagine. */
  name: string;
  /** Immagine che si vede adesso (salvata o scelta), `null` se non ce n'è. */
  image: BotImage | null;
  disabled: boolean;
  onPick: (image: BotImage) => void;
  onRemove: () => void;
  onError: (message: string | null) => void;
}

/** Immagine del segretario: stessa pipeline pixel-art delle immagini dei bot, con anteprima prima di usarla. */
export default function SecretaryImageField({ name, image, disabled, onPick, onRemove, onError }: Props) {
  const picker = useBotImagePicker(onError);
  const shownName = name.trim() === "" ? FALLBACK_NAME : name.trim();

  const confirm = () => {
    if (!picker.candidate) return;
    onPick(picker.candidate.image);
    picker.cancel();
  };

  return (
    <div className="flex flex-col gap-2">
      <p className="text-xs font-semibold">Immagine</p>
      <div className="flex items-center gap-3">
        <BotAvatar name={shownName} size={AVATAR_SIZE} image={image ?? undefined} />
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={disabled || picker.pixelating}
          onClick={() => picker.pick(shownName)}
        >
          <ImagePlus className="!size-3.5" aria-hidden="true" /> {image ? "Cambia immagine" : "Scegli un’immagine"}
        </Button>
        {image && (
          <Button type="button" size="sm" variant="outline" disabled={disabled} onClick={onRemove}>
            <ImageOff className="!size-3.5" aria-hidden="true" /> Rimuovi
          </Button>
        )}
      </div>
      <input
        ref={picker.inputRef}
        type="file"
        accept={BOT_IMAGE_ACCEPT}
        className="hidden"
        tabIndex={-1}
        aria-hidden="true"
        onChange={(e) => void picker.onFileChange(e)}
      />
      {picker.candidate && (
        <BotImagePreview
          name={shownName}
          image={picker.candidate.image}
          busy={disabled || picker.pixelating}
          onConfirm={confirm}
          onCancel={picker.cancel}
        />
      )}
    </div>
  );
}
