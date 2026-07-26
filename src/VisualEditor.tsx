import { useEffect, useRef, useState } from "react";
import { CrepeBuilder } from "@milkdown/crepe/builder";
import { blockEdit } from "@milkdown/crepe/feature/block-edit";
import { codeMirror } from "@milkdown/crepe/feature/code-mirror";
import { cursor } from "@milkdown/crepe/feature/cursor";
import { linkTooltip } from "@milkdown/crepe/feature/link-tooltip";
import { listItem } from "@milkdown/crepe/feature/list-item";
import { placeholder } from "@milkdown/crepe/feature/placeholder";
import { table } from "@milkdown/crepe/feature/table";
import { toolbar } from "@milkdown/crepe/feature/toolbar";
import { uploadConfig } from "@milkdown/kit/plugin/upload";
import { replaceAll } from "@milkdown/kit/utils";
import "@milkdown/crepe/theme/common/style.css";
import { serializeVisualMarkdown } from "./markdownDocument";
import { splitReviewDocument } from "./review";

type Props = {
  tabId: string;
  value: string;
  readOnly: boolean;
  onChange: (value: string) => void;
  onRequestSource: () => void;
};

export function VisualEditor({ tabId, value, readOnly, onChange, onRequestSource }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const crepeRef = useRef<CrepeBuilder | null>(null);
  const onChangeRef = useRef(onChange);
  const prefixRef = useRef("");
  const lastBodyRef = useRef("");
  const lastValueRef = useRef(value);
  const baselineBodyRef = useRef("");
  const baselineValueRef = useRef(value);
  const suppressChangesRef = useRef(false);
  const readyRef = useRef(false);
  const [frontmatter, setFrontmatter] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);

  useEffect(() => {
    if (!host.current) return;
    const parts = splitReviewDocument(lastValueRef.current);
    setFrontmatter(Boolean(parts.frontmatter) && !parts.error);
    setError(parts.error || null);
    if (parts.error) return;

    prefixRef.current = `${parts.frontmatter}${parts.reviewBlock}`;
    lastBodyRef.current = parts.body;
    baselineValueRef.current = lastValueRef.current;
    readyRef.current = false;
    let created = false;
    let cancelled = false;

    const crepe = new CrepeBuilder({
      root: host.current,
      defaultValue: parts.body,
    })
      .addFeature(blockEdit, {
        advancedGroup: {
          image: null,
          math: null,
        },
      })
      .addFeature(codeMirror)
      .addFeature(cursor)
      .addFeature(linkTooltip)
      .addFeature(listItem)
      .addFeature(placeholder, {
        text: "Start writing, or type / for commands",
        mode: "doc",
      })
      .addFeature(table)
      .addFeature(toolbar);
    // Crepe's fallback image uploader creates temporary blob URLs. Refuse image
    // paste/drop until Construct can copy files into a stable local destination.
    crepe.editor.config((ctx) => {
      ctx.update(uploadConfig.key, (current) => ({
        ...current,
        uploader: async () => [],
      }));
    });
    crepeRef.current = crepe;
    crepe.setReadonly(readOnly);
    crepe.on((listener) => {
      listener.markdownUpdated((_ctx, markdown) => {
        if (!readyRef.current || suppressChangesRef.current || markdown === lastBodyRef.current) return;
        lastBodyRef.current = markdown;
        // Milkdown has a canonical Markdown representation. When undo returns to
        // that canonical baseline, restore the exact input bytes instead of
        // leaving the tab dirty because of harmless serialization differences.
        const nextValue = serializeVisualMarkdown(
          prefixRef.current,
          markdown,
          baselineBodyRef.current,
          baselineValueRef.current,
        );
        lastValueRef.current = nextValue;
        onChangeRef.current(nextValue);
      });
    });

    void crepe.create()
      .then(() => {
        created = true;
        if (cancelled) {
          void crepe.destroy();
          return;
        }
        // Milkdown may normalize its internal representation during creation.
        // Treat that as the baseline so merely entering Edit never dirties a file.
        const latest = splitReviewDocument(lastValueRef.current);
        if (!latest.error && latest.body !== parts.body) {
          prefixRef.current = `${latest.frontmatter}${latest.reviewBlock}`;
          suppressChangesRef.current = true;
          crepe.editor.action(replaceAll(latest.body, true));
          suppressChangesRef.current = false;
        }
        lastBodyRef.current = crepe.getMarkdown();
        baselineBodyRef.current = lastBodyRef.current;
        baselineValueRef.current = lastValueRef.current;
        readyRef.current = true;
      })
      .catch((cause: unknown) => {
        if (!cancelled) setError(cause instanceof Error ? cause.message : String(cause));
      });

    return () => {
      cancelled = true;
      readyRef.current = false;
      crepeRef.current = null;
      if (created) void crepe.destroy();
    };
    // The editor owns its undo stack for a tab. External value updates are handled
    // by the synchronization effect below without recreating the editor.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tabId]);

  useEffect(() => {
    crepeRef.current?.setReadonly(readOnly);
  }, [readOnly]);

  useEffect(() => {
    if (value === lastValueRef.current) return;
    lastValueRef.current = value;
    const parts = splitReviewDocument(value);
    setFrontmatter(Boolean(parts.frontmatter) && !parts.error);
    setError(parts.error || null);
    if (parts.error || !readyRef.current || !crepeRef.current) return;

    prefixRef.current = `${parts.frontmatter}${parts.reviewBlock}`;
    const current = crepeRef.current.getMarkdown();
    if (current === parts.body) {
      lastBodyRef.current = current;
      baselineBodyRef.current = current;
      baselineValueRef.current = value;
      return;
    }

    suppressChangesRef.current = true;
    crepeRef.current.editor.action(replaceAll(parts.body, true));
    lastBodyRef.current = crepeRef.current.getMarkdown();
    baselineBodyRef.current = lastBodyRef.current;
    baselineValueRef.current = value;
    suppressChangesRef.current = false;
  }, [value]);

  if (error) {
    return (
      <div className="visual-editor-error">
        <div>
          <strong>Visual editing is unavailable for this document.</strong>
          <p>{error}</p>
          <button className="toolbar-button" onClick={onRequestSource}>Open Source</button>
        </div>
      </div>
    );
  }

  return (
    <div className="visual-editor">
      {frontmatter && (
        <div className="visual-frontmatter-note">
          <span>YAML</span>
          <p>Frontmatter is preserved exactly. Edit metadata in Source.</p>
          <button onClick={onRequestSource}>Open Source</button>
        </div>
      )}
      <div className="visual-editor-host" ref={host} />
    </div>
  );
}
