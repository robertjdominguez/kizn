import { type ReactNode, useState } from "react";

// Every screen's layout: an optional header that stays put (back link,
// title, progress, Stop) and a body that scrolls in its own container, so the
// app's top bar and the screen's header stay in view. A hairline appears
// under the header once the body has scrolled.
export function Screen({
  header,
  children,
  width = "max-w-3xl",
}: {
  header?: ReactNode;
  children: ReactNode;
  /** The column's max width (a Tailwind class), shared by header and body. */
  width?: string;
}) {
  const [scrolled, setScrolled] = useState(false);
  const column = `mx-auto w-full px-6 transition-[max-width] duration-300 ${width}`;
  return (
    <div className="flex h-full flex-col">
      {header && (
        <div
          className={`shrink-0 border-b transition-colors duration-200 ${scrolled ? "border-hairline" : "border-transparent"}`}
        >
          <div className={`${column} flex flex-col gap-3 pt-6 pb-4`}>{header}</div>
        </div>
      )}
      <div className="min-h-0 flex-1 overflow-y-auto" onScroll={(e) => setScrolled(e.currentTarget.scrollTop > 0)}>
        <div className={`${column} flex flex-col gap-6 ${header ? "pt-2" : "pt-8"} pb-10`}>{children}</div>
      </div>
    </div>
  );
}
