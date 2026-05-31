/// <reference types="vite/client" />

interface Window {
  __XAC_EDITOR__?: {
    getValue: () => string;
    setValue: (value: string) => void;
  };
}
