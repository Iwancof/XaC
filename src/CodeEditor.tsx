import * as monaco from "monaco-editor";
import { useEffect, useRef } from "react";

interface CodeEditorProps {
  value: string;
  onChange: (value: string) => void;
}

export function CodeEditor({ value, onChange }: CodeEditorProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const changeRef = useRef(onChange);

  useEffect(() => {
    changeRef.current = onChange;
  }, [onChange]);

  useEffect(() => {
    if (!hostRef.current) return;
    monaco.languages.register({ id: "xac-script" });
    monaco.languages.setMonarchTokensProvider("xac-script", {
      keywords: [
        "mine",
        "return",
        "stop",
        "noop",
        "push",
        "push_any",
        "set_recipe",
        "produce",
        "attack_nearest",
        "attack_best",
        "dispatch",
        "net",
        "net_set",
        "output_blocked",
        "output_available",
        "can_produce",
        "ammo_count",
        "fuel_remaining",
        "ore",
        "ammo",
        "plate",
        "cpu_part",
        "drone_part",
        "nearest",
        "lowest_hp",
        "weakest",
        "grunt",
        "runner",
        "armored",
        "wire_cutter",
        "north",
        "east",
        "south",
        "west",
        "module",
        "import",
        "func",
        "export",
        "param",
        "result",
        "local",
        "local.get",
        "local.set",
        "block",
        "loop",
        "if",
        "then",
        "drop",
        "call",
        "br",
        "br_if",
        "i32.const",
        "i32.add",
        "i32.eqz",
        "i32.eq",
        "i32.ge_s"
      ],
      tokenizer: {
        root: [
          [/[a-zA-Z_][\w.-]*/, { cases: { "@keywords": "keyword", "@default": "identifier" } }],
          [/".*?"/, "string"],
          [/[()[\]]/, "delimiter"],
          [/#.*$/, "comment"],
          [/\/\/.*$/, "comment"],
          [/;;.*$/, "comment"]
        ]
      }
    });

    const editor = monaco.editor.create(hostRef.current, {
      value,
      language: "xac-script",
      theme: "vs-dark",
      minimap: { enabled: false },
      fontSize: 13,
      lineNumbersMinChars: 3,
      scrollBeyondLastLine: false,
      automaticLayout: true,
      tabSize: 4,
      wordWrap: "on"
    });
    editorRef.current = editor;
    const disposable = editor.onDidChangeModelContent(() => {
      changeRef.current(editor.getValue());
    });
    if (import.meta.env.VITE_XAC_MOCK_IPC === "1") {
      window.__XAC_EDITOR__ = {
        getValue: () => editor.getValue(),
        setValue: (nextValue: string) => {
          editor.setValue(nextValue);
          changeRef.current(nextValue);
        }
      };
    }
    return () => {
      if (import.meta.env.VITE_XAC_MOCK_IPC === "1") {
        window.__XAC_EDITOR__ = undefined;
      }
      disposable.dispose();
      editor.dispose();
      editorRef.current = null;
    };
  }, []);

  useEffect(() => {
    const editor = editorRef.current;
    if (editor && editor.getValue() !== value) {
      editor.setValue(value);
    }
  }, [value]);

  return (
    <div
      className="code-editor"
      data-testid="code-editor"
      data-source={import.meta.env.VITE_XAC_MOCK_IPC === "1" ? value : undefined}
      ref={hostRef}
    />
  );
}
