import { Fragment, type ReactNode } from "react";

// The little Markdown a tutor reply uses: paragraphs, "- " lists, **bold**
// and `code`. Rendered as elements, never as HTML.
export function Markdown({ text }: { text: string }) {
  const blocks = text.trim().split(/\n{2,}/);
  return (
    <>
      {blocks.map((block, i) => {
        const lines = block.split("\n");
        if (lines.every((l) => /^\s*[-*] /.test(l))) {
          return (
            // biome-ignore lint/suspicious/noArrayIndexKey: blocks are positional
            <ul key={i} className="ml-4 list-disc space-y-1">
              {lines.map((l, j) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: lines are positional
                <li key={j}>{inline(l.replace(/^\s*[-*] /, ""))}</li>
              ))}
            </ul>
          );
        }
        return (
          // biome-ignore lint/suspicious/noArrayIndexKey: blocks are positional
          <p key={i}>
            {lines.map((l, j) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: lines are positional
              <Fragment key={j}>
                {j > 0 && <br />}
                {inline(l)}
              </Fragment>
            ))}
          </p>
        );
      })}
    </>
  );
}

function inline(text: string): ReactNode[] {
  return text.split(/(`[^`]+`|\*\*[^*]+\*\*)/g).map((part, i) => {
    if (part.startsWith("`") && part.endsWith("`") && part.length > 2) {
      return (
        // biome-ignore lint/suspicious/noArrayIndexKey: parts are positional
        <code key={i} className="rounded bg-raised px-1 py-px font-mono text-[0.85em] text-ink">
          {part.slice(1, -1)}
        </code>
      );
    }
    if (part.startsWith("**") && part.endsWith("**") && part.length > 4) {
      return (
        // biome-ignore lint/suspicious/noArrayIndexKey: parts are positional
        <strong key={i} className="font-semibold text-ink">
          {part.slice(2, -2)}
        </strong>
      );
    }
    return part;
  });
}
