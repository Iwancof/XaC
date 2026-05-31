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
    monaco.languages.register({ id: "xac-pseudo" });
    monaco.languages.setMonarchTokensProvider("xac-pseudo", {
      keywords: ["export", "tick", "self", "if", "else", "return", "prefer"],
      tokenizer: {
        root: [
          [/[a-zA-Z_][\w-]*/, { cases: { "@keywords": "keyword", "@default": "identifier" } }],
          [/".*?"/, "string"],
          [/[{}()[\],.:]/, "delimiter"],
          [/#.*$/, "comment"]
        ]
      }
    });

    const editor = monaco.editor.create(hostRef.current, {
      value,
      language: "xac-pseudo",
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
    return () => {
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

  return <div className="code-editor" ref={hostRef} />;
}
