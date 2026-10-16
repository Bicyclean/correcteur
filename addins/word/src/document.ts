// Accès au document : Word (Office.js) ou zone de texte de démonstration.

import { type Target, escapeSearch, replacement } from "./mapping";

/**
 * Résultat d'une action sur le document :
 * - `ok` : fait ;
 * - `changed` : le paragraphe a été modifié depuis l'analyse ;
 * - `notfound` : le texte n'a pas été retrouvé.
 */
export type ActionResult = "ok" | "changed" | "notfound";

export interface DocumentAccess {
  readonly kind: "word" | "demo";
  readParagraphs(): Promise<string[]>;
  /** Sélectionne l'erreur dans le document. */
  select(paragraph: number, expected: string, target: Target): Promise<ActionResult>;
  /** Remplace l'erreur par `suggestion`. */
  replace(paragraph: number, expected: string, target: Target, suggestion: string): Promise<ActionResult>;
}

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

/** Document de démonstration : une zone de texte, un paragraphe par ligne. */
export class DemoDocument implements DocumentAccess {
  readonly kind = "demo";

  constructor(private readonly area: HTMLTextAreaElement) {}

  async readParagraphs(): Promise<string[]> {
    return this.area.value.split("\n");
  }

  async select(paragraph: number, expected: string, target: Target): Promise<ActionResult> {
    const at = this.offset(paragraph, expected);
    if (at === null) return "changed";
    const start = at + target.start + target.prefix.length;
    const end = at + target.start + target.snippet.length - target.suffix.length;
    this.area.focus();
    this.area.setSelectionRange(start, end);
    return "ok";
  }

  async replace(paragraph: number, expected: string, target: Target, suggestion: string): Promise<ActionResult> {
    const at = this.offset(paragraph, expected);
    if (at === null) return "changed";
    const start = at + target.start;
    const end = start + target.snippet.length;
    const v = this.area.value;
    this.area.value = v.slice(0, start) + replacement(target, suggestion) + v.slice(end);
    return "ok";
  }

  /** Position du début du paragraphe, ou `null` s'il a changé. */
  private offset(paragraph: number, expected: string): number | null {
    const lines = this.area.value.split("\n");
    if (lines[paragraph] !== expected) return null;
    return lines.slice(0, paragraph).reduce((n, l) => n + l.length + 1, 0);
  }
}
