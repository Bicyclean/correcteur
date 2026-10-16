// Accès au document Word par Office.js.

import { type ActionResult, type DocumentAccess } from "../../../web/commun/document";
import { type Target, escapeSearch, replacement } from "../../../web/commun/mapping";

export class WordDocument implements DocumentAccess {
  readonly kind = "word";

  async readParagraphs(): Promise<string[]> {
    return Word.run(async (ctx) => {
      const paragraphs = ctx.document.body.paragraphs;
      paragraphs.load("items/text");
      await ctx.sync();
      return paragraphs.items.map((p) => p.text);
    });
  }

  select(paragraph: number, expected: string, target: Target): Promise<ActionResult> {
    return this.withRange(paragraph, expected, target, (range) => range.select());
  }

  replace(paragraph: number, expected: string, target: Target, suggestion: string): Promise<ActionResult> {
    return this.withRange(paragraph, expected, target, (range) => {
      range.insertText(replacement(target, suggestion), "Replace");
    });
  }

  private withRange(
    paragraph: number,
    expected: string,
    target: Target,
    action: (range: Word.Range) => void,
  ): Promise<ActionResult> {
    return Word.run(async (ctx) => {
      const paragraphs = ctx.document.body.paragraphs;
      paragraphs.load("items/text");
      await ctx.sync();
      const p = paragraphs.items[paragraph];
      if (!p || p.text !== expected) return "changed";
      const results = p.search(escapeSearch(target.snippet), { matchCase: true, matchWildcards: false });
      results.load("items/text");
      await ctx.sync();
      // Word peut confondre certains caractères (guillemets droits et courbes) :
      // ne garder que les correspondances exactes, dans l'ordre du document.
      const exact = results.items.filter((r) => r.text === target.snippet);
      const range = exact[target.occurrence];
      if (!range) return "notfound";
      action(range);
      await ctx.sync();
      return "ok";
    });
  }
}
