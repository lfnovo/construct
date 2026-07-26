import { useEffect, useId, useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import rehypeRaw from "rehype-raw";
import rehypeSanitize from "rehype-sanitize";
import mermaid from "mermaid";
import { api } from "./api";
import { resolveOkfLink, withoutFrontmatter } from "./okf";

type Props = {
  content: string;
  sourcePath: string;
  bundleRoot?: string;
  onOpenInternal: (path: string) => void;
};

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

export function MarkdownPreview({ content, sourcePath, bundleRoot, onOpenInternal }: Props) {
  const plugins = useMemo(() => [remarkGfm], []);
  return (
    <article className="markdown-preview">
      <ReactMarkdown
        remarkPlugins={plugins}
        rehypePlugins={[rehypeRaw, rehypeSanitize, rehypeHighlight]}
        components={{
          code({ className, children, ...props }) {
            const language = /language-(\w+)/.exec(className || "")?.[1];
            const text = String(children).replace(/\n$/, "");
            if (language === "mermaid") return <MermaidDiagram code={text} />;
            return <code className={className} {...props}>{children}</code>;
          },
          a({ href = "", children, ...props }) {
            return (
              <a
                {...props}
                href={href}
                onClick={(event) => {
                  event.preventDefault();
                  if (href.startsWith("http://") || href.startsWith("https://")) {
                    void api.openExternalUrl(href);
                  } else if (!href.startsWith("#")) {
                    onOpenInternal(resolveOkfLink(sourcePath, bundleRoot, href));
                  } else {
                    const anchor = href.slice(1);
                    document.getElementById(anchor)?.scrollIntoView({ behavior: "smooth" });
                  }
                }}
              >{children}</a>
            );
          },
          img({ src, alt }) { return <LocalImage src={src} alt={alt || ""} sourcePath={sourcePath} bundleRoot={bundleRoot} />; },
        }}
      >
        {withoutFrontmatter(content)}
      </ReactMarkdown>
    </article>
  );
}
