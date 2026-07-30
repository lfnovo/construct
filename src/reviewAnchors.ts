export type ReviewAnchor = {
  start: number;
  end: number;
  prefix: string;
  suffix: string;
};

export type ResolvedReviewAnchor = {
  start: number;
  end: number;
};

const CONTEXT_LENGTH = 80;

export function normalizeReviewText(value: string) {
  return value.replace(/\s+/g, " ").trim();
}

function occurrences(text: string, quote: string) {
  const matches: number[] = [];
  let offset = 0;
  while (offset <= text.length - quote.length) {
    const index = text.indexOf(quote, offset);
    if (index < 0) break;
    matches.push(index);
    offset = index + Math.max(1, quote.length);
  }
  return matches;
}

export function createReviewAnchor(text: string, quote: string, approximateStart: number) {
  const normalizedText = normalizeReviewText(text);
  const normalizedQuote = normalizeReviewText(quote);
  if (!normalizedQuote) return null;
  const matches = occurrences(normalizedText, normalizedQuote);
  if (!matches.length) return null;
  const start = matches.reduce((closest, candidate) => (
    Math.abs(candidate - approximateStart) < Math.abs(closest - approximateStart)
      ? candidate
      : closest
  ));
  const end = start + normalizedQuote.length;
  return {
    start,
    end,
    prefix: normalizedText.slice(Math.max(0, start - CONTEXT_LENGTH), start),
    suffix: normalizedText.slice(end, end + CONTEXT_LENGTH),
  } satisfies ReviewAnchor;
}

function contextMatches(text: string, start: number, quoteLength: number, anchor: ReviewAnchor) {
  const before = text.slice(Math.max(0, start - anchor.prefix.length), start);
  const after = text.slice(start + quoteLength, start + quoteLength + anchor.suffix.length);
  return {
    prefix: !anchor.prefix || before === anchor.prefix,
    suffix: !anchor.suffix || after === anchor.suffix,
  };
}

export function resolveReviewAnchor(
  text: string,
  quote: string,
  anchor?: ReviewAnchor,
): ResolvedReviewAnchor | null {
  const normalizedText = normalizeReviewText(text);
  const normalizedQuote = normalizeReviewText(quote);
  if (!normalizedQuote) return null;

  if (
    anchor
    && anchor.start >= 0
    && normalizedText.slice(anchor.start, anchor.start + normalizedQuote.length) === normalizedQuote
  ) {
    return { start: anchor.start, end: anchor.start + normalizedQuote.length };
  }

  const matches = occurrences(normalizedText, normalizedQuote);
  if (matches.length === 1) {
    return { start: matches[0], end: matches[0] + normalizedQuote.length };
  }
  if (!anchor || !matches.length) return null;

  const contextual = matches.filter((start) => {
    const matched = contextMatches(normalizedText, start, normalizedQuote.length, anchor);
    return matched.prefix && matched.suffix;
  });
  if (contextual.length === 1) {
    return {
      start: contextual[0],
      end: contextual[0] + normalizedQuote.length,
    };
  }

  const prefixMatches = matches.filter((start) => (
    contextMatches(normalizedText, start, normalizedQuote.length, anchor).prefix
  ));
  if (anchor.prefix && prefixMatches.length === 1) {
    return {
      start: prefixMatches[0],
      end: prefixMatches[0] + normalizedQuote.length,
    };
  }
  const suffixMatches = matches.filter((start) => (
    contextMatches(normalizedText, start, normalizedQuote.length, anchor).suffix
  ));
  if (!anchor.suffix || suffixMatches.length !== 1) return null;
  return {
    start: suffixMatches[0],
    end: suffixMatches[0] + normalizedQuote.length,
  };
}
