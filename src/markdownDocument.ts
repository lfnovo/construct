export type MarkdownDocumentParts = {
  frontmatter: string;
  body: string;
  hasFrontmatter: boolean;
  error?: string;
};

/**
 * Separates a leading YAML frontmatter block without parsing or normalizing it.
 * The returned frontmatter includes both delimiters and its original line endings,
 * so joining the parts again is byte-for-byte lossless.
 */
export function splitMarkdownDocument(content: string): MarkdownDocumentParts {
  const opening = content.match(/^---[ \t]*\r?\n/);
  if (!opening) return { frontmatter: "", body: content, hasFrontmatter: false };

  const closing = /^---[ \t]*(?:\r?\n|$)/gm;
  closing.lastIndex = opening[0].length;
  const match = closing.exec(content);
  if (!match) {
    return {
      frontmatter: "",
      body: content,
      hasFrontmatter: true,
      error: "The YAML frontmatter block is not closed.",
    };
  }

  const bodyStart = match.index + match[0].length;
  return {
    frontmatter: content.slice(0, bodyStart),
    body: content.slice(bodyStart),
    hasFrontmatter: true,
  };
}

export function joinMarkdownDocument(frontmatter: string, body: string) {
  return `${frontmatter}${body}`;
}

export function serializeVisualMarkdown(
  frontmatter: string,
  body: string,
  baselineBody: string,
  baselineValue: string,
) {
  return body === baselineBody ? baselineValue : joinMarkdownDocument(frontmatter, body);
}
