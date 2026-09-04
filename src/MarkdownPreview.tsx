import { createContext, memo, useContext, useEffect, useId, useMemo, useState, type ComponentPropsWithoutRef } from "react";
import ReactMarkdown, { type Components, type ExtraProps } from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import rehypeRaw from "rehype-raw";
import rehypeSanitize from "rehype-sanitize";
import mermaid from "mermaid";
import { api } from "./api";
import { resolveOkfLink, withoutFrontmatter } from "./okf";
import type { ReviewComment } from "./review";
import { rehypeReviewHighlights } from "./reviewHighlights";
import { DocumentErrorBoundary } from "./DocumentErrorBoundary";

type Props = {
  content: string;
  sourcePath: string;
  bundleRoot?: string;
  onOpenInternal: (path: string) => void;
  reviewComments?: readonly ReviewComment[];
  activeReviewId?: string | null;
  onRequestSource?: () => void;
  onRetryView?: () => void;
};

const NO_COMMENTS: readonly ReviewComment[] = [];
const MarkdownContext = createContext<Pick<Props, "sourcePath" | "bundleRoot" | "onOpenInternal" | "activeReviewId"> | null>(null);

function MermaidDiagram({ code }: { code: string }) {
  const id = useId().replace(/:/g, "-");
  const [svg, setSvg] = useState<string>("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    mermaid.initialize({ startOnLoad: false, securityLevel: "strict", theme: "neutral" });
    mermaid.render(`mermaid-${id}`, code)
      .then((result) => { if (!cancelled) { setSvg(result.svg); setError(null); } })
      .catch(() => { if (!cancelled) { setSvg(""); setError("This Mermaid diagram could not be rendered."); } });
    return () => { cancelled = true; };
  }, [code, id]);

  if (error) return <pre className="mermaid-error">{error}{"\n\n"}{code}</pre>;
  return <div className="mermaid" dangerouslySetInnerHTML={{ __html: svg }} />;
}

function LocalImage({ src = "", alt = "", sourcePath, bundleRoot }: { src?: string; alt?: string; sourcePath: string; bundleRoot?: string }) {
  const directSource = src.startsWith("http://") || src.startsWith("https://") || src.startsWith("data:");
  const localPath = directSource ? null : resolveOkfLink(sourcePath, bundleRoot, src);
  const [loaded, setLoaded] = useState<{ path: string; url: string } | null>(null);
  useEffect(() => {
    if (!localPath) return;
    let cancelled = false;
    void api.readImageDataUrl(localPath)
      .then((url) => {
        if (!cancelled) setLoaded({ path: localPath, url });
      })
      .catch(() => {
        if (!cancelled) setLoaded(null);
      });
    return () => {
      cancelled = true;
    };
  }, [localPath]);
  const resolved = directSource ? src : loaded?.path === localPath ? loaded.url : null;
  return resolved ? <img src={resolved} alt={alt} /> : <span className="missing-image">Image unavailable: {alt || src}</span>;
}

function withoutAstNode<T extends object>({ node, ...props }: T & ExtraProps) {
  void node;
  return props;
}

function MarkdownCode(input: ComponentPropsWithoutRef<"code"> & ExtraProps) {
  const { className, children, ...props } = withoutAstNode(input);
  const language = /language-(\w+)/.exec(className || "")?.[1];
  if (language === "mermaid") return <MermaidDiagram code={String(children).replace(/\n$/, "")} />;
  return <code className={className} {...props}>{children}</code>;
}

function MarkdownLink(input: ComponentPropsWithoutRef<"a"> & ExtraProps) {
  const { href = "", children, ...props } = withoutAstNode(input);
  const context = useContext(MarkdownContext)!;
  return (
    <a {...props} href={href} onClick={(event) => {
      event.preventDefault();
      if ((event.target as Element).closest("mark[data-review-id]")) return;
      if (href.startsWith("http://") || href.startsWith("https://")) {
        void api.openExternalUrl(href);
      } else if (!href.startsWith("#")) {
        context.onOpenInternal(resolveOkfLink(context.sourcePath, context.bundleRoot, href));
      } else {
        document.getElementById(href.slice(1))?.scrollIntoView({ behavior: "smooth" });
      }
    }}>{children}</a>
  );
}

function MarkdownImage({ src, alt }: ComponentPropsWithoutRef<"img">) {
  const { sourcePath, bundleRoot } = useContext(MarkdownContext)!;
  return <LocalImage src={src} alt={alt || ""} sourcePath={sourcePath} bundleRoot={bundleRoot} />;
}

// Component identity must not depend on changing callbacks from the workspace.
function MarkdownMark(input: ComponentPropsWithoutRef<"mark"> & ExtraProps) {
  const { activeReviewId } = useContext(MarkdownContext)!;
  const reviewId = input.node?.properties.dataReviewId;
  const { className, ...props } = withoutAstNode(input);
  const classes = [className, reviewId && reviewId === activeReviewId ? "active" : ""].filter(Boolean).join(" ");
  return <mark {...props} className={classes || undefined} />;
}

const components: Components = { code: MarkdownCode, a: MarkdownLink, img: MarkdownImage, mark: MarkdownMark };

export const MarkdownPreview = memo(function MarkdownPreview({ content, sourcePath, bundleRoot, onOpenInternal, reviewComments = NO_COMMENTS, activeReviewId, onRequestSource, onRetryView }: Props) {
  const plugins = useMemo(() => [remarkGfm], []);
  const context = useMemo(() => ({ sourcePath, bundleRoot, onOpenInternal, activeReviewId }), [sourcePath, bundleRoot, onOpenInternal, activeReviewId]);
  const rehypePlugins = useMemo(() => [rehypeRaw, rehypeSanitize, rehypeHighlight,
    [rehypeReviewHighlights, { comments: reviewComments }] as [typeof rehypeReviewHighlights, { comments: readonly ReviewComment[] }],
  ], [reviewComments]);
  // Context-only changes (navigation and active comment) must not parse Markdown.
  const markdown = useMemo(() => (
    <ReactMarkdown remarkPlugins={plugins} rehypePlugins={rehypePlugins} components={components}>
      {withoutFrontmatter(content)}
    </ReactMarkdown>
  ), [content, plugins, rehypePlugins]);
  return (
    <article className="markdown-preview">
      <MarkdownContext.Provider value={context}>
        {onRequestSource ? (
          <DocumentErrorBoundary mode={reviewComments === NO_COMMENTS ? "preview" : "review"} onRequestSource={onRequestSource} onRetry={onRetryView}>
            {markdown}
          </DocumentErrorBoundary>
        ) : markdown}
      </MarkdownContext.Provider>
    </article>
  );
});
