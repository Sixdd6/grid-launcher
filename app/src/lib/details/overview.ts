// The Overview tab's description, as paragraphs. Pure, like `header.ts`.

/** The server's description split into paragraphs at its line breaks, with
 *  blank and whitespace-only lines dropped. A description with no breaks is
 *  one paragraph: the text is the server's, so it is never split by guess. */
export function descriptionParagraphs(description: string): string[] {
  return description
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line !== '');
}
