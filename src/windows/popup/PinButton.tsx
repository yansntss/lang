interface PinButtonProps {
  pinned: boolean;
  onToggle: () => void;
}

/** Botão discreto no canto: fixado, o popup não some ao clicar em outro app. */
export default function PinButton({ pinned, onToggle }: PinButtonProps) {
  return (
    <button
      type="button"
      className="popup__pin"
      aria-label="Fixar na tela"
      aria-pressed={pinned}
      title={pinned ? "Desafixar" : "Fixar na tela"}
      // O foco fica no campo de texto: Enter e Esc continuam funcionando.
      onMouseDown={(event) => event.preventDefault()}
      onClick={onToggle}
    >
      <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true" focusable="false">
        <path
          fill="currentColor"
          d="M9.8 1.5 14.5 6.2l-1.4 1.4-.7-.7-2.6 2.6.3 3-1.4 1.4-2.5-2.5-3.2 3.2-.7-.7 3.2-3.2-2.5-2.5 1.4-1.4 3 .3 2.6-2.6-.7-.7z"
        />
      </svg>
    </button>
  );
}
