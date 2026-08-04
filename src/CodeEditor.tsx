import { useEffect, useRef } from "react";
import { Compartment, EditorState } from "@codemirror/state";
import { EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";
import { defaultHighlightStyle, syntaxHighlighting } from "@codemirror/language";

type Props = {
  tabId: string;
  value: string;
  readOnly: boolean;
  onChange: (value: string) => void;
  onSave: () => void;
};

export function CodeEditor({ tabId, value, readOnly, onChange, onSave }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const editableCompartment = useRef(new Compartment());
  const onChangeRef = useRef(onChange);
  const onSaveRef = useRef(onSave);

  useEffect(() => {
    onChangeRef.current = onChange;
    onSaveRef.current = onSave;
  }, [onChange, onSave]);

  useEffect(() => {
    if (!host.current) return;
    const updateListener = EditorView.updateListener.of((update) => {
      if (update.docChanged) onChangeRef.current(update.state.doc.toString());
    });
    const view = new EditorView({
      state: EditorState.create({
        doc: value,
        extensions: [
          lineNumbers(),
          highlightActiveLine(),
          drawSelection(),
          history(),
          markdown(),
          syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
          keymap.of([
            ...defaultKeymap,
            ...historyKeymap,
            indentWithTab,
            { key: "Mod-s", preventDefault: true, run: () => { onSaveRef.current(); return true; } },
          ]),
          updateListener,
          editableCompartment.current.of(EditorView.editable.of(!readOnly)),
          EditorView.theme({
            "&": {
              height: "100%",
              backgroundColor: "var(--bg)",
              color: "var(--text)",
              fontSize: "13px",
            },
            ".cm-scroller": { overflow: "auto", fontFamily: "var(--font-mono)" },
            ".cm-content": {
              padding: "18px 0 80px",
              caretColor: "var(--accent)",
              color: "var(--text)",
            },
            ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--accent)" },
            ".cm-gutters": { backgroundColor: "var(--surface)", borderRight: "1px solid var(--border)", color: "var(--muted)" },
            ".cm-activeLineGutter": { backgroundColor: "var(--surface-hover)" },
            ".cm-activeLine": { backgroundColor: "var(--editor-active-line)" },
            "&.cm-focused .cm-selectionBackground, .cm-selectionBackground": {
              backgroundColor: "var(--editor-selection)",
            },
            ".cm-content ::selection": {
              backgroundColor: "var(--editor-selection)",
              color: "var(--text)",
            },
          }),
        ],
      }),
      parent: host.current,
    });
    viewRef.current = view;
    return () => view.destroy();
    // Recreate the editor only when the active tab changes. Later effects synchronize
    // value and read-only state without destroying selection or undo history.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tabId]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const current = view.state.doc.toString();
    if (current !== value) {
      view.dispatch({ changes: { from: 0, to: current.length, insert: value } });
    }
  }, [value]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    view.dispatch({ effects: editableCompartment.current.reconfigure(EditorView.editable.of(!readOnly)) });
  }, [readOnly]);

  return <div className="code-editor" ref={host} aria-label="Editor de Markdown" />;
}
